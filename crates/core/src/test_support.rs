//! In-memory fakes of every port, builders, and the port contract suite.
//! Enabled by the `test-support` feature.

pub mod contract;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use uuid::Uuid;

use crate::history::{
    ChangeContext, ChangeKind, EntityType, HistoryEntry, HistoryEntryId, snapshot,
};
use crate::person::{Person, PersonId, PersonView};
use crate::ports::{
    HistoryQuery, Page, PageRequest, PeopleQuery, PeopleSort, PersonRepository, ReferenceCounter,
    ReferenceKind, RepoError, Transaction, UnitOfWork,
};

#[derive(Debug, Clone, Default)]
struct State {
    people: BTreeMap<PersonId, Person>,
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
