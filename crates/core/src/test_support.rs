//! In-memory fakes of every port, builders, and the port contract suite.
//! Enabled by the `test-support` feature.

pub mod contract;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::action::{Action, ActionId, ActionView};
use crate::blocker::{Blocker, BlockerId, BlockerView};
use crate::comment::{Comment, CommentBody, CommentId, CommentView, Target, TargetType};
use crate::history::{
    ChangeContext, ChangeKind, EntityType, HistoryEntry, HistoryEntryId, snapshot,
};
use crate::person::{Person, PersonId, PersonView};
use crate::ports::{
    ActionFilter, ActionRepository, ActionSort, ActionsQuery, BlockerFilter, BlockerRepository,
    BlockerSort, BlockersQuery, CommentFilter, CommentRepository, CommentsQuery, HistoryQuery,
    Page, PageRequest, PeopleQuery, PeopleSort, PersonRepository, ProjectRepository, ProjectSort,
    ProjectsQuery, ReferenceCounter, ReferenceKind, RepoError, RiskFilter, RiskRepository,
    RiskSort, RisksQuery, Transaction, UnitOfWork,
};
use crate::project::{
    Description, ItemRef, PersonRef, Project, ProjectCode, ProjectId, ProjectName, ProjectView,
};
use crate::risk::{Rating, Risk, RiskId, RiskView};

#[derive(Debug, Clone, Default)]
struct State {
    people: BTreeMap<PersonId, Person>,
    projects: BTreeMap<ProjectId, Project>,
    risks: BTreeMap<RiskId, Risk>,
    history: Vec<HistoryEntry>,
    references: HashMap<(Uuid, ReferenceKind), u32>,
    actions: BTreeMap<ActionId, Action>,
    blockers: BTreeMap<BlockerId, Blocker>,
    comments: BTreeMap<CommentId, Comment>,
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

    fn risks(&mut self) -> impl RiskRepository + Send + '_ {
        InMemoryRisks(&mut self.work)
    }

    fn actions(&mut self) -> impl ActionRepository + Send + '_ {
        InMemoryActions(&mut self.work)
    }

    fn blockers(&mut self) -> impl BlockerRepository + Send + '_ {
        InMemoryBlockers(&mut self.work)
    }

    fn comments(&mut self) -> impl CommentRepository + Send + '_ {
        InMemoryComments(&mut self.work)
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

struct InMemoryActions<'a>(&'a mut State);

impl ActionRepository for InMemoryActions<'_> {
    async fn load(&mut self, id: ActionId) -> Result<Action, RepoError> {
        self.0.actions.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Action>,
        after: &Action,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.actions.get(&after.id()).map(Action::version);
        if stored_version != before.map(Action::version) {
            return Err(RepoError::Conflict);
        }
        self.0.actions.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Action,
            entity_id: after.id().as_uuid(),
            project_id: Some(after.project_id().as_uuid()),
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

    async fn open_assigned_to(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> Result<Vec<Action>, RepoError> {
        Ok(self
            .0
            .actions
            .values()
            .filter(|a| a.project_id() == project && !a.closed() && a.assignee() == Some(person))
            .cloned()
            .collect())
    }
}

struct InMemoryBlockers<'a>(&'a mut State);

impl BlockerRepository for InMemoryBlockers<'_> {
    async fn load(&mut self, id: BlockerId) -> Result<Blocker, RepoError> {
        self.0.blockers.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Blocker>,
        after: &Blocker,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.blockers.get(&after.id()).map(Blocker::version);
        if stored_version != before.map(Blocker::version) {
            return Err(RepoError::Conflict);
        }
        if after
            .blocked_actions()
            .iter()
            .any(|a| !self.0.actions.contains_key(a))
        {
            return Err(RepoError::Unexpected(
                "blocker linked to an unknown action".to_owned(),
            ));
        }
        self.0.blockers.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Blocker,
            entity_id: after.id().as_uuid(),
            project_id: Some(after.project_id().as_uuid()),
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

    async fn open_accountable(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> Result<Vec<Blocker>, RepoError> {
        Ok(self
            .0
            .blockers
            .values()
            .filter(|b| {
                b.project_id() == project && !b.cleared() && b.accountable() == Some(person)
            })
            .cloned()
            .collect())
    }
}

struct InMemoryRisks<'a>(&'a mut State);

impl RiskRepository for InMemoryRisks<'_> {
    async fn load(&mut self, id: RiskId) -> Result<Risk, RepoError> {
        self.0.risks.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Risk>,
        after: &Risk,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.risks.get(&after.id()).map(Risk::version);
        if stored_version != before.map(Risk::version) {
            return Err(RepoError::Conflict);
        }
        if !self.0.projects.contains_key(&after.project_id()) {
            return Err(RepoError::Unexpected(
                "risk in an unknown project".to_owned(),
            ));
        }
        self.0.risks.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Risk,
            entity_id: after.id().as_uuid(),
            project_id: Some(after.project_id().as_uuid()),
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

struct InMemoryComments<'a>(&'a mut State);

impl CommentRepository for InMemoryComments<'_> {
    async fn load(&mut self, id: CommentId) -> Result<Comment, RepoError> {
        self.0.comments.get(&id).cloned().ok_or(RepoError::NotFound)
    }

    async fn save(
        &mut self,
        before: Option<&Comment>,
        after: &Comment,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let stored_version = self.0.comments.get(&after.id()).map(Comment::version);
        if stored_version != before.map(Comment::version) {
            return Err(RepoError::Conflict);
        }
        if !self.0.projects.contains_key(&after.project_id())
            || !self.0.people.contains_key(&after.author())
        {
            return Err(RepoError::Unexpected(
                "comment in an unknown project or by an unknown person".to_owned(),
            ));
        }
        self.0.comments.insert(after.id(), after.clone());
        let id = HistoryEntryId::new();
        self.0.history.push(HistoryEntry {
            id,
            entity_type: EntityType::Comment,
            entity_id: after.id().as_uuid(),
            project_id: Some(after.project_id().as_uuid()),
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
        health_rating: project.health_rating().cloned(),
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

    async fn project_members(&self, id: ProjectId) -> Result<Vec<PersonRef>, RepoError> {
        let state = self.snapshot();
        let project = state.projects.get(&id).ok_or(RepoError::NotFound)?;
        let mut members: Vec<_> = project
            .members()
            .iter()
            .map(|&id| PersonRef {
                id,
                display_name: state
                    .people
                    .get(&id)
                    .map(|p| p.display_name().as_str().to_owned())
                    .unwrap_or_default(),
            })
            .collect();
        members.sort_by(|a, b| (&a.display_name, a.id).cmp(&(&b.display_name, b.id)));
        Ok(members)
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

fn risk_view(state: &State, risk: &Risk) -> RiskView {
    RiskView {
        id: risk.id(),
        project_id: risk.project_id(),
        project_code: state
            .projects
            .get(&risk.project_id())
            .map(|p| p.project_code().as_str().to_owned())
            .unwrap_or_default(),
        reference: ReferenceKind::Risk.reference(risk.number()),
        description: risk.description().as_str().to_owned(),
        likelihood: risk.likelihood().get(),
        impact: risk.impact().get(),
        score: risk.score(),
        closed: risk.closed(),
        version: risk.version(),
    }
}

impl RisksQuery for InMemoryStore {
    async fn list_risks(
        &self,
        page: PageRequest,
        filter: RiskFilter,
        sort: RiskSort,
    ) -> Result<Page<RiskView>, RepoError> {
        let state = self.snapshot();
        let mut all: Vec<_> = state
            .risks
            .values()
            .filter(|r| filter.project_id.is_none_or(|p| r.project_id() == p))
            .filter(|r| filter.state.matches(r.closed()))
            .collect();
        match sort {
            RiskSort::Id => all.sort_by_key(|r| r.id()),
            RiskSort::Reference => all.sort_by_key(|r| (r.project_id(), r.number(), r.id())),
            RiskSort::Score => all.sort_by_key(|r| (std::cmp::Reverse(r.score()), r.id())),
        }
        let all = all.into_iter().map(|r| risk_view(&state, r)).collect();
        Ok(page_of(all, page))
    }

    async fn get_risk(&self, id: RiskId) -> Result<RiskView, RepoError> {
        let state = self.snapshot();
        state
            .risks
            .get(&id)
            .map(|r| risk_view(&state, r))
            .ok_or(RepoError::NotFound)
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

    async fn project_history(&self, project_id: ProjectId) -> Result<Vec<HistoryEntry>, RepoError> {
        Ok(self
            .snapshot()
            .history
            .into_iter()
            .filter(|e| e.project_id == Some(project_id.as_uuid()))
            .collect())
    }

    async fn project_history_between(
        &self,
        project_id: ProjectId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<HistoryEntry>, RepoError> {
        Ok(self
            .project_history(project_id)
            .await?
            .into_iter()
            .filter(|e| from <= e.recorded_at && e.recorded_at < to)
            .collect())
    }
}

fn person_ref(state: &State, id: PersonId) -> PersonRef {
    PersonRef {
        id,
        display_name: state
            .people
            .get(&id)
            .map(|p| p.display_name().as_str().to_owned())
            .unwrap_or_default(),
    }
}

fn action_view(state: &State, action: &Action) -> ActionView {
    ActionView {
        id: action.id(),
        project_id: action.project_id(),
        project_code: state
            .projects
            .get(&action.project_id())
            .map(|p| p.project_code().as_str().to_owned())
            .unwrap_or_default(),
        reference: action.reference_label(),
        description: action.description().as_str().to_owned(),
        assignee: action.assignee().map(|id| person_ref(state, id)),
        due_date: action.due_date(),
        closed: action.closed(),
        blocked_by: {
            let mut open: Vec<_> = state
                .blockers
                .values()
                .filter(|b| !b.cleared() && b.blocked_actions().contains(&action.id()))
                .collect();
            open.sort_by_key(|b| b.reference());
            open.into_iter()
                .map(|b| ItemRef {
                    id: b.id().as_uuid(),
                    reference: b.reference_label(),
                })
                .collect()
        },
        version: action.version(),
    }
}

/// By display name, with no one last.
fn by_person(a: Option<&PersonRef>, b: Option<&PersonRef>) -> std::cmp::Ordering {
    let key = |p: Option<&PersonRef>| (p.is_none(), p.map(|p| p.display_name.clone()));
    key(a).cmp(&key(b))
}

impl ActionsQuery for InMemoryStore {
    async fn list_actions(
        &self,
        filter: ActionFilter,
        page: PageRequest,
        sort: ActionSort,
    ) -> Result<Page<ActionView>, RepoError> {
        let state = self.snapshot();
        let mut all: Vec<_> = state
            .actions
            .values()
            .filter(|a| filter.project_id.is_none_or(|p| a.project_id() == p))
            .filter(|a| filter.state.matches(a.closed()))
            .filter(|a| filter.assignee.is_none_or(|f| f.matches(a.assignee())))
            .filter(|a| {
                filter
                    .due_by
                    .is_none_or(|d| a.due_date().is_some_and(|due| due <= d))
            })
            .map(|a| (a.reference(), action_view(&state, a)))
            .collect();
        match sort {
            ActionSort::Id => all.sort_by_key(|(_, a)| a.id),
            ActionSort::Reference => all.sort_by(|(ra, a), (rb, b)| {
                (&a.project_code, ra, a.id).cmp(&(&b.project_code, rb, b.id))
            }),
            ActionSort::DueDate => {
                all.sort_by_key(|(_, a)| (a.due_date.is_none(), a.due_date, a.id));
            }
            ActionSort::Assignee => all.sort_by(|(_, a), (_, b)| {
                by_person(a.assignee.as_ref(), b.assignee.as_ref()).then(a.id.cmp(&b.id))
            }),
        }
        Ok(page_of(all.into_iter().map(|(_, v)| v).collect(), page))
    }

    async fn get_action(&self, id: ActionId) -> Result<ActionView, RepoError> {
        let state = self.snapshot();
        state
            .actions
            .get(&id)
            .map(|a| action_view(&state, a))
            .ok_or(RepoError::NotFound)
    }
}

fn blocker_view(state: &State, blocker: &Blocker) -> BlockerView {
    let mut blocked: Vec<_> = blocker
        .blocked_actions()
        .iter()
        .filter_map(|id| state.actions.get(id))
        .collect();
    blocked.sort_by_key(|a| a.reference());
    BlockerView {
        id: blocker.id(),
        project_id: blocker.project_id(),
        project_code: state
            .projects
            .get(&blocker.project_id())
            .map(|p| p.project_code().as_str().to_owned())
            .unwrap_or_default(),
        reference: blocker.reference_label(),
        description: blocker.description().as_str().to_owned(),
        accountable: blocker.accountable().map(|id| person_ref(state, id)),
        cleared: blocker.cleared(),
        blocked_actions: blocked
            .into_iter()
            .map(|a| ItemRef {
                id: a.id().as_uuid(),
                reference: a.reference_label(),
            })
            .collect(),
        version: blocker.version(),
    }
}

impl BlockersQuery for InMemoryStore {
    async fn list_blockers(
        &self,
        filter: BlockerFilter,
        page: PageRequest,
        sort: BlockerSort,
    ) -> Result<Page<BlockerView>, RepoError> {
        let state = self.snapshot();
        let mut all: Vec<_> = state
            .blockers
            .values()
            .filter(|b| filter.project_id.is_none_or(|p| b.project_id() == p))
            .filter(|b| filter.state.matches(b.cleared()))
            .filter(|b| {
                filter
                    .accountable
                    .is_none_or(|f| f.matches(b.accountable()))
            })
            .map(|b| (b.reference(), blocker_view(&state, b)))
            .collect();
        match sort {
            BlockerSort::Id => all.sort_by_key(|(_, b)| b.id),
            BlockerSort::Reference => all.sort_by(|(ra, a), (rb, b)| {
                (&a.project_code, ra, a.id).cmp(&(&b.project_code, rb, b.id))
            }),
            BlockerSort::Accountable => all.sort_by(|(_, a), (_, b)| {
                by_person(a.accountable.as_ref(), b.accountable.as_ref()).then(a.id.cmp(&b.id))
            }),
        }
        Ok(page_of(all.into_iter().map(|(_, v)| v).collect(), page))
    }

    async fn get_blocker(&self, id: BlockerId) -> Result<BlockerView, RepoError> {
        let state = self.snapshot();
        state
            .blockers
            .get(&id)
            .map(|b| blocker_view(&state, b))
            .ok_or(RepoError::NotFound)
    }
}

fn comment_view(state: &State, comment: &Comment) -> CommentView {
    CommentView {
        id: comment.id(),
        project_id: comment.project_id(),
        target: comment.target(),
        author: person_ref(state, comment.author()),
        body: comment.body().as_str().to_owned(),
        created_at: comment.created_at(),
        edited_at: comment.edited_at(),
        deleted: comment.is_deleted(),
        version: comment.version(),
    }
}

impl CommentsQuery for InMemoryStore {
    async fn list_comments(
        &self,
        filter: CommentFilter,
        page: PageRequest,
    ) -> Result<Page<CommentView>, RepoError> {
        let state = self.snapshot();
        let mut all: Vec<_> = state
            .comments
            .values()
            .filter(|c| filter.project_id.is_none_or(|p| c.project_id() == p))
            .filter(|c| filter.target.is_none_or(|t| c.target() == t))
            .collect();
        all.sort_by_key(|c| (c.created_at(), c.id()));
        let all = all.into_iter().map(|c| comment_view(&state, c)).collect();
        Ok(page_of(all, page))
    }

    async fn get_comment(&self, id: CommentId) -> Result<CommentView, RepoError> {
        let state = self.snapshot();
        state
            .comments
            .get(&id)
            .map(|c| comment_view(&state, c))
            .ok_or(RepoError::NotFound)
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

/// Builds a valid open [`Risk`] in `project`, defaulting every other field.
pub fn a_risk(project: ProjectId, number: u32) -> RiskBuilder {
    RiskBuilder {
        project,
        number,
        description: "Test Risk".to_owned(),
        likelihood: 3,
        impact: 3,
    }
}

pub struct RiskBuilder {
    project: ProjectId,
    number: u32,
    description: String,
    likelihood: u8,
    impact: u8,
}

impl RiskBuilder {
    pub fn described(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn rated(mut self, likelihood: u8, impact: u8) -> Self {
        self.likelihood = likelihood;
        self.impact = impact;
        self
    }

    pub fn build(self) -> Risk {
        Risk::create(
            self.project,
            self.number,
            Description::new(&self.description).expect("valid description"),
            Rating::new(self.likelihood).expect("valid likelihood"),
            Rating::new(self.impact).expect("valid impact"),
        )
    }
}

/// Builds a valid [`Comment`] by `author` on `project` itself, defaulting
/// every other field.
pub fn a_comment(project: ProjectId, author: PersonId) -> CommentBuilder {
    CommentBuilder {
        project,
        target: Target {
            kind: TargetType::Project,
            id: project.as_uuid(),
        },
        author,
        body: "Test Comment".to_owned(),
    }
}

pub struct CommentBuilder {
    project: ProjectId,
    target: Target,
    author: PersonId,
    body: String,
}

impl CommentBuilder {
    pub fn on(mut self, kind: TargetType, id: Uuid) -> Self {
        self.target = Target { kind, id };
        self
    }

    pub fn saying(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn build(self) -> Comment {
        Comment::create(
            self.project,
            self.target,
            self.author,
            CommentBody::new(self.body).expect("valid body"),
        )
    }
}
