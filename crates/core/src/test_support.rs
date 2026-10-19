//! In-memory fakes of every port, builders, and the port contract suite.
//! Enabled by the `test-support` feature.

pub mod contract;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use uuid::Uuid;

use crate::history::{
    ChangeContext, ChangeKind, EntityType, HistoryEntry, HistoryEntryId, snapshot,
};
use crate::person::{Person, PersonId, PersonView};
use crate::ports::{
    HistoryQuery, Page, PageRequest, PeopleQuery, PeopleSort, PersonRepository, ProjectRepository,
    ProjectSort, ProjectsQuery, ReferenceCounter, ReferenceKind, RepoError, Transaction,
    UnitOfWork,
};
use crate::project::{
    Description, PersonRef, Project, ProjectCode, ProjectId, ProjectName, ProjectView,
};

#[derive(Debug, Clone, Default)]
struct State {
    people: BTreeMap<PersonId, Person>,
    projects: BTreeMap<ProjectId, Project>,
    history: Vec<HistoryEntry>,
    references: HashMap<(Uuid, ReferenceKind), u32>,
}

/// An in-memory store implementing every port. Transactions work on a copy
/// of the state and replace it on commit, so dropping one rolls back.
#[derive(Debug, Clone, Default)]
pub struct InMemoryStore {
    state: Arc<Mutex<State>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> State {
        self.state.lock().expect("store lock").clone()
    }
}

pub struct InMemoryTx {
    store: Arc<Mutex<State>>,
    work: State,
}

impl UnitOfWork for InMemoryStore {
    type Tx = InMemoryTx;

    async fn begin(&self) -> Result<InMemoryTx, RepoError> {
        Ok(InMemoryTx {
            store: Arc::clone(&self.state),
            work: self.snapshot(),
        })
    }
}

impl Transaction for InMemoryTx {
    fn people(&mut self) -> impl PersonRepository + Send + '_ {
        InMemoryPeople(&mut self.work)
    }

    fn projects(&mut self) -> impl ProjectRepository + Send + '_ {
        InMemoryProjects(&mut self.work)
    }

    fn references(&mut self) -> impl ReferenceCounter + Send + '_ {
        InMemoryReferences(&mut self.work)
    }

    async fn commit(self) -> Result<(), RepoError> {
        *self.store.lock().expect("store lock") = self.work;
        Ok(())
    }
}

struct InMemoryPeople<'a>(&'a mut State);

impl PersonRepository for InMemoryPeople<'_> {
    async fn load(&mut self, id: PersonId) -> Result<Person, RepoError> {
        self.0.people.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Person>,
        after: &Person,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.people.get(&after.id()).map(Person::version);
        if stored_version != before.map(Person::version) {
            return Err(RepoError::Conflict);
        }
        self.0.people.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Person,
            entity_id: after.id().as_uuid(),
            project_id: None,
            version: after.version(),
            change_kind: if before.is_some() {
                ChangeKind::Updated
            } else {
                ChangeKind::Created
            },
            acting_person: ctx.acting_person,
            recorded_at: Utc::now(),
            before: before.map(snapshot),
            after: snapshot(after),
            caused_by: ctx.caused_by,
        });
        Ok(id)
    }
}

struct InMemoryProjects<'a>(&'a mut State);

impl ProjectRepository for InMemoryProjects<'_> {
    async fn load(&mut self, id: ProjectId) -> Result<Project, RepoError> {
        self.0.projects.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Project>,
        after: &Project,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.projects.get(&after.id()).map(Project::version);
        if stored_version != before.map(Project::version) {
            return Err(RepoError::Conflict);
        }
        let code_clash = self
            .0
            .projects
            .values()
            .any(|p| p.id() != after.id() && p.project_code() == after.project_code());
        if code_clash {
            return Err(RepoError::UniqueViolation {
                constraint: "projects.project_code".to_owned(),
            });
        }
        self.0.projects.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Project,
            entity_id: after.id().as_uuid(),
            project_id: Some(after.id().as_uuid()),
            version: after.version(),
            change_kind: if before.is_some() {
                ChangeKind::Updated
            } else {
                ChangeKind::Created
            },
            acting_person: ctx.acting_person,
            recorded_at: Utc::now(),
            before: before.map(snapshot),
            after: snapshot(after),
            caused_by: ctx.caused_by,
        });
        Ok(id)
    }
}

struct InMemoryReferences<'a>(&'a mut State);

impl ReferenceCounter for InMemoryReferences<'_> {
    async fn next(&mut self, project_id: Uuid, kind: ReferenceKind) -> Result<u32, RepoError> {
        let next = self.0.references.entry((project_id, kind)).or_insert(0);
        *next += 1;
        Ok(*next)
    }
}

fn page_of<T>(all: Vec<T>, page: PageRequest) -> Page<T> {
    let total = all.len() as u64;
    let items = all
        .into_iter()
        .skip(page.offset as usize)
        .take(page.limit as usize)
        .collect();
    Page { items, total }
}

fn view(person: &Person) -> PersonView {
    PersonView {
        id: person.id(),
        display_name: person.display_name().as_str().to_owned(),
        version: person.version(),
    }
}

impl PeopleQuery for InMemoryStore {
    async fn list_people(
        &self,
        page: PageRequest,
        sort: PeopleSort,
    ) -> Result<Page<PersonView>, RepoError> {
        let mut all: Vec<_> = self.snapshot().people.values().map(view).collect();
        match sort {
            PeopleSort::Id => all.sort_by_key(|p| p.id),
            PeopleSort::DisplayName => {
                all.sort_by(|a, b| (&a.display_name, a.id).cmp(&(&b.display_name, b.id)))
            }
        }
        Ok(page_of(all, page))
    }

    async fn get_person(&self, id: PersonId) -> Result<PersonView, RepoError> {
        self.snapshot()
            .people
            .get(&id)
            .map(view)
            .ok_or(RepoError::NotFound)
    }
}

fn project_view(state: &State, project: &Project) -> ProjectView {
    ProjectView {
        id: project.id(),
        name: project.name().as_str().to_owned(),
        description: project.description().map(|d| d.as_str().to_owned()),
        project_code: project.project_code().as_str().to_owned(),
        archived: project.archived(),
        project_leader: project.project_leader().map(|id| PersonRef {
            id,
            display_name: state
                .people
                .get(&id)
                .map(|p| p.display_name().as_str().to_owned())
                .unwrap_or_default(),
        }),
        version: project.version(),
    }
}

impl ProjectsQuery for InMemoryStore {
    async fn list_projects(
        &self,
        page: PageRequest,
        include_archived: bool,
        sort: ProjectSort,
    ) -> Result<Page<ProjectView>, RepoError> {
        let state = self.snapshot();
        let mut all: Vec<_> = state
            .projects
            .values()
            .filter(|p| include_archived || !p.archived())
            .map(|p| project_view(&state, p))
            .collect();
        match sort {
            ProjectSort::Id => all.sort_by_key(|p| p.id),
            ProjectSort::Name => all.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id))),
        }
        Ok(page_of(all, page))
    }

    async fn get_project(&self, id: ProjectId) -> Result<ProjectView, RepoError> {
        let state = self.snapshot();
        state
            .projects
            .get(&id)
            .map(|p| project_view(&state, p))
            .ok_or(RepoError::NotFound)
    }

    async fn project_codes_starting_with(
        &self,
        prefix: &str,
    ) -> Result<HashSet<String>, RepoError> {
        Ok(self
            .snapshot()
            .projects
            .values()
            .map(|p| p.project_code().as_str())
            .filter(|c| c.starts_with(prefix))
            .map(str::to_owned)
            .collect())
    }
}

impl HistoryQuery for InMemoryStore {
    async fn entity_history(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
        page: PageRequest,
    ) -> Result<Page<HistoryEntry>, RepoError> {
        let all = self
            .snapshot()
            .history
            .into_iter()
            .filter(|e| e.entity_type == entity_type && e.entity_id == entity_id)
            .collect();
        Ok(page_of(all, page))
    }
}

/// Builds a valid [`Person`], defaulting every field.
pub fn a_person() -> PersonBuilder {
    PersonBuilder {
        display_name: "Test Person".to_owned(),
    }
}

pub struct PersonBuilder {
    display_name: String,
}

impl PersonBuilder {
    pub fn named(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    pub fn build(self) -> Person {
        Person::create(&self.display_name).expect("builder produces a valid person")
    }
}

/// Builds a valid [`Project`] led by `creator`, defaulting every other field.
pub fn a_project(creator: PersonId) -> ProjectBuilder {
    ProjectBuilder {
        creator,
        name: "Test Project".to_owned(),
        description: None,
        code: "TEST".to_owned(),
    }
}

pub struct ProjectBuilder {
    creator: PersonId,
    name: String,
    description: Option<String>,
    code: String,
}

impl ProjectBuilder {
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn described(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn coded(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn build(self) -> Project {
        Project::create(
            ProjectName::new(&self.name).expect("valid name"),
            self.description
                .map(|d| Description::new(d).expect("valid description")),
            ProjectCode::new(&self.code).expect("valid code"),
            self.creator,
        )
    }
}
