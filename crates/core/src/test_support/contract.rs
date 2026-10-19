//! Port contract suite (#12). Every adapter, fake or real, must pass each case.
//! Each case takes a fresh, empty store.

use uuid::Uuid;

use crate::history::{ChangeContext, ChangeKind, EntityType, snapshot};
use crate::person::PersonId;
use crate::ports::{
    HistoryQuery, PageRequest, PeopleQuery, PeopleSort, PersonRepository, ReferenceCounter,
    ReferenceKind, RepoError, Transaction, UnitOfWork,
};
use crate::test_support::a_person;

/// Everything a full store adapter provides.
pub trait Store: UnitOfWork + PeopleQuery + HistoryQuery {}
impl<T: UnitOfWork + PeopleQuery + HistoryQuery> Store for T {}

fn ctx() -> ChangeContext {
    ChangeContext::by(PersonId::new())
}

pub async fn saved_person_can_be_loaded(store: impl Store) {
    let person = a_person().named("Ada").build();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &person, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(tx.people().load(person.id()).await.unwrap(), person);
}

pub async fn loading_an_unknown_person_is_not_found(store: impl Store) {
    let mut tx = store.begin().await.unwrap();
    assert_eq!(
        tx.people().load(PersonId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
}

pub async fn saving_an_update_replaces_the_state(store: impl Store) {
    let v1 = a_person().named("Ada").build();
    let v2 = v1.renamed("Ada King").unwrap();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &v1, &ctx()).await.unwrap();
    tx.people().save(Some(&v1), &v2, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(tx.people().load(v1.id()).await.unwrap(), v2);
}

pub async fn saving_from_a_stale_version_is_a_conflict(store: impl Store) {
    let v1 = a_person().build();
    let v2 = v1.renamed("First").unwrap();
    let stale = v1.renamed("Second").unwrap();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &v1, &ctx()).await.unwrap();
    tx.people().save(Some(&v1), &v2, &ctx()).await.unwrap();
    assert_eq!(
        tx.people()
            .save(Some(&v1), &stale, &ctx())
            .await
            .unwrap_err(),
        RepoError::Conflict
    );
}

pub async fn creating_an_existing_person_again_is_a_conflict(store: impl Store) {
    let person = a_person().build();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &person, &ctx()).await.unwrap();
    assert_eq!(
        tx.people().save(None, &person, &ctx()).await.unwrap_err(),
        RepoError::Conflict
    );
}

pub async fn dropping_a_transaction_rolls_it_back(store: impl Store) {
    let person = a_person().build();
    {
        let mut tx = store.begin().await.unwrap();
        tx.people().save(None, &person, &ctx()).await.unwrap();
    }
    let mut tx = store.begin().await.unwrap();
    assert_eq!(
        tx.people().load(person.id()).await.unwrap_err(),
        RepoError::NotFound
    );
    let history = store
        .entity_history(
            EntityType::Person,
            person.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    assert_eq!(history.total, 0);
}

pub async fn each_save_writes_a_history_entry(store: impl Store) {
    let actor = PersonId::new();
    let v1 = a_person().named("Ada").build();
    let v2 = v1.renamed("Ada King").unwrap();
    let mut tx = store.begin().await.unwrap();
    let created = tx
        .people()
        .save(None, &v1, &ChangeContext::by(actor))
        .await
        .unwrap();
    let updated = tx
        .people()
        .save(Some(&v1), &v2, &ChangeContext::by(actor).caused_by(created))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let history = store
        .entity_history(
            EntityType::Person,
            v1.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    assert_eq!(history.total, 2);
    let [first, second] = history.items.as_slice() else {
        panic!("expected two entries, got {:?}", history.items);
    };

    assert_eq!(first.id, created);
    assert_eq!(first.entity_type, EntityType::Person);
    assert_eq!(first.entity_id, v1.id().as_uuid());
    assert_eq!(first.project_id, None);
    assert_eq!(first.version, 1);
    assert_eq!(first.change_kind, ChangeKind::Created);
    assert_eq!(first.acting_person, actor);
    assert_eq!(first.before, None);
    assert_eq!(first.after, snapshot(&v1));
    assert_eq!(first.caused_by, None);

    assert_eq!(second.id, updated);
    assert_eq!(second.version, 2);
    assert_eq!(second.change_kind, ChangeKind::Updated);
    assert_eq!(second.before, Some(snapshot(&v1)));
    assert_eq!(second.after, snapshot(&v2));
    assert_eq!(second.caused_by, Some(created));
    assert!(second.recorded_at >= first.recorded_at);
}

pub async fn a_conflicting_save_writes_no_history(store: impl Store) {
    let v1 = a_person().build();
    let v2 = v1.renamed("Two").unwrap();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &v1, &ctx()).await.unwrap();
    tx.people().save(Some(&v1), &v2, &ctx()).await.unwrap();
    let _ = tx.people().save(Some(&v1), &v2, &ctx()).await;
    tx.commit().await.unwrap();

    let history = store
        .entity_history(
            EntityType::Person,
            v1.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    assert_eq!(history.total, 2);
}

pub async fn references_count_up_per_project_and_kind(store: impl Store) {
    let (p1, p2) = (Uuid::now_v7(), Uuid::now_v7());
    let mut tx = store.begin().await.unwrap();
    let mut refs = tx.references();
    assert_eq!(refs.next(p1, ReferenceKind::Action).await.unwrap(), 1);
    assert_eq!(refs.next(p1, ReferenceKind::Action).await.unwrap(), 2);
    assert_eq!(refs.next(p1, ReferenceKind::Risk).await.unwrap(), 1);
    assert_eq!(refs.next(p2, ReferenceKind::Action).await.unwrap(), 1);
    drop(refs);
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(
        tx.references()
            .next(p1, ReferenceKind::Action)
            .await
            .unwrap(),
        3
    );
}

pub async fn rolled_back_references_are_reused(store: impl Store) {
    let project = Uuid::now_v7();
    {
        let mut tx = store.begin().await.unwrap();
        tx.references()
            .next(project, ReferenceKind::Blocker)
            .await
            .unwrap();
    }
    let mut tx = store.begin().await.unwrap();
    assert_eq!(
        tx.references()
            .next(project, ReferenceKind::Blocker)
            .await
            .unwrap(),
        1
    );
}

pub async fn people_are_listed_by_name_in_pages(store: impl Store) {
    let mut tx = store.begin().await.unwrap();
    for name in ["Charlie", "Ada", "Bea"] {
        let person = a_person().named(name).build();
        tx.people().save(None, &person, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let page = store
        .list_people(
            PageRequest {
                offset: 1,
                limit: 1,
            },
            PeopleSort::DisplayName,
        )
        .await
        .unwrap();
    assert_eq!(page.total, 3);
    let names: Vec<_> = page.items.iter().map(|p| p.display_name.as_str()).collect();
    assert_eq!(names, ["Bea"]);
}

pub async fn people_are_listed_by_id_by_default(store: impl Store) {
    let mut tx = store.begin().await.unwrap();
    for name in ["Charlie", "Ada", "Bea"] {
        let person = a_person().named(name).build();
        tx.people().save(None, &person, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let page = store
        .list_people(PageRequest::first(10), PeopleSort::Id)
        .await
        .unwrap();
    let names: Vec<_> = page.items.iter().map(|p| p.display_name.as_str()).collect();
    assert_eq!(names, ["Charlie", "Ada", "Bea"]);
}

pub async fn a_person_can_be_fetched_by_id(store: impl Store) {
    let person = a_person().named("Ada").build();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &person, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let view = store.get_person(person.id()).await.unwrap();
    assert_eq!(view.id, person.id());
    assert_eq!(view.display_name, "Ada");
    assert_eq!(view.version, 1);
    assert_eq!(
        store.get_person(PersonId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
}

/// Generates one test per contract case. `$make` turns the test's arguments
/// into a fresh store; `$attr` is the test attribute to use.
///
/// ```ignore
/// lodestar_core::port_contract_tests!(#[tokio::test], () => InMemoryStore::new());
/// ```
#[macro_export]
macro_rules! port_contract_tests {
    (#[$attr:meta], $args:tt => $make:expr) => {
        $crate::port_contract_tests!(@cases #[$attr], $args => $make;
            saved_person_can_be_loaded,
            loading_an_unknown_person_is_not_found,
            saving_an_update_replaces_the_state,
            saving_from_a_stale_version_is_a_conflict,
            creating_an_existing_person_again_is_a_conflict,
            dropping_a_transaction_rolls_it_back,
            each_save_writes_a_history_entry,
            a_conflicting_save_writes_no_history,
            references_count_up_per_project_and_kind,
            rolled_back_references_are_reused,
            people_are_listed_by_name_in_pages,
            people_are_listed_by_id_by_default,
            a_person_can_be_fetched_by_id
        );
    };
    (@cases #[$attr:meta], $args:tt => $make:expr; $($case:ident),*) => {
        $(
            #[$attr]
            async fn $case $args {
                $crate::test_support::contract::$case($make).await
            }
        )*
    };
}
