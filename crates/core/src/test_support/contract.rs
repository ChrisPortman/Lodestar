//! Port contract suite (#12). Every adapter, fake or real, must pass each case.
//! Each case takes a fresh, empty store.

use uuid::Uuid;

use crate::history::{ChangeContext, ChangeKind, EntityType, snapshot};
use crate::person::PersonId;
use crate::ports::{
    HistoryQuery, PageRequest, PeopleQuery, PeopleSort, PersonRepository, ProjectRepository,
    ProjectSort, ProjectsQuery, ReferenceCounter, ReferenceKind, RepoError, Transaction,
    UnitOfWork,
};
use crate::project::{ProjectId, ProjectName};
use crate::test_support::{a_person, a_project};

/// Everything a full store adapter provides.
pub trait Store: UnitOfWork + PeopleQuery + ProjectsQuery + HistoryQuery {}
impl<T: UnitOfWork + PeopleQuery + ProjectsQuery + HistoryQuery> Store for T {}

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

/// Saves a Person to lead Projects, returning their ID.
async fn a_saved_person(store: &impl Store, name: &str) -> PersonId {
    let person = a_person().named(name).build();
    let mut tx = store.begin().await.unwrap();
    tx.people().save(None, &person, &ctx()).await.unwrap();
    tx.commit().await.unwrap();
    person.id()
}

pub async fn saved_project_can_be_loaded_and_updated(store: impl Store) {
    let ada = a_saved_person(&store, "Ada").await;
    let v1 = a_project(ada)
        .named("Web")
        .described("Site")
        .coded("WEBR")
        .build();
    let v2 = v1
        .edited(ProjectName::new("Website").unwrap(), None, None)
        .unwrap();
    let mut tx = store.begin().await.unwrap();
    tx.projects().save(None, &v1, &ctx()).await.unwrap();
    assert_eq!(tx.projects().load(v1.id()).await.unwrap(), v1);
    tx.projects().save(Some(&v1), &v2, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(tx.projects().load(v1.id()).await.unwrap(), v2);
    assert_eq!(
        tx.projects().load(ProjectId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
    assert_eq!(
        tx.projects()
            .save(Some(&v1), &v2, &ctx())
            .await
            .unwrap_err(),
        RepoError::Conflict
    );
}

pub async fn a_project_code_can_be_used_only_once(store: impl Store) {
    let ada = a_saved_person(&store, "Ada").await;
    let first = a_project(ada).coded("WEBR").build();
    let second = a_project(ada).coded("WEBR").build();
    let mut tx = store.begin().await.unwrap();
    tx.projects().save(None, &first, &ctx()).await.unwrap();
    assert!(matches!(
        tx.projects().save(None, &second, &ctx()).await.unwrap_err(),
        RepoError::UniqueViolation { .. }
    ));
}

pub async fn each_project_save_writes_a_history_entry(store: impl Store) {
    let ada = a_saved_person(&store, "Ada").await;
    let v1 = a_project(ada).build();
    let v2 = v1.with_archived(true);
    let mut tx = store.begin().await.unwrap();
    tx.projects()
        .save(None, &v1, &ChangeContext::by(ada))
        .await
        .unwrap();
    tx.projects()
        .save(Some(&v1), &v2, &ChangeContext::by(ada))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let history = store
        .entity_history(
            EntityType::Project,
            v1.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let [first, second] = history.items.as_slice() else {
        panic!("expected two entries, got {:?}", history.items);
    };
    assert_eq!(first.change_kind, ChangeKind::Created);
    assert_eq!(first.project_id, Some(v1.id().as_uuid()));
    assert_eq!(first.acting_person, ada);
    assert_eq!(first.after, snapshot(&v1));
    assert_eq!(second.change_kind, ChangeKind::Updated);
    assert_eq!(second.before, Some(snapshot(&v1)));
    assert_eq!(second.after, snapshot(&v2));
}

pub async fn projects_are_listed_without_archived_ones_by_default(store: impl Store) {
    let ada = a_saved_person(&store, "Ada").await;
    let web = a_project(ada).named("Web").coded("WEBR").build();
    let alpha = a_project(ada)
        .named("Alpha")
        .described("First")
        .coded("ALPH")
        .build();
    let old = a_project(ada).named("Old").coded("OLD").build();
    let mut tx = store.begin().await.unwrap();
    for p in [&web, &alpha, &old] {
        tx.projects().save(None, p, &ctx()).await.unwrap();
    }
    tx.projects()
        .save(Some(&old), &old.with_archived(true), &ctx())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let names = |page: crate::ports::Page<crate::project::ProjectView>| -> Vec<String> {
        page.items.into_iter().map(|p| p.name).collect()
    };
    let default = store
        .list_projects(PageRequest::first(10), false, ProjectSort::Id)
        .await
        .unwrap();
    assert_eq!(default.total, 2);
    assert_eq!(names(default), ["Web", "Alpha"]);
    let all = store
        .list_projects(
            PageRequest {
                offset: 1,
                limit: 2,
            },
            true,
            ProjectSort::Name,
        )
        .await
        .unwrap();
    assert_eq!(all.total, 3);
    assert_eq!(names(all), ["Old", "Web"]);

    let view = store.get_project(alpha.id()).await.unwrap();
    assert_eq!(view.name, "Alpha");
    assert_eq!(view.description.as_deref(), Some("First"));
    assert_eq!(view.project_code, "ALPH");
    assert!(!view.archived);
    let leader = view.project_leader.unwrap();
    assert_eq!((leader.id, leader.display_name.as_str()), (ada, "Ada"));
    assert_eq!(view.version, 1);
    assert!(store.get_project(old.id()).await.unwrap().archived);
    assert_eq!(
        store.get_project(ProjectId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
}

pub async fn project_codes_can_be_found_by_prefix(store: impl Store) {
    let ada = a_saved_person(&store, "Ada").await;
    let mut tx = store.begin().await.unwrap();
    for code in ["WEBS", "WEBS1", "WEB", "AWEBS"] {
        let p = a_project(ada).coded(code).build();
        tx.projects().save(None, &p, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let mut found: Vec<_> = store
        .project_codes_starting_with("WEBS")
        .await
        .unwrap()
        .into_iter()
        .collect();
    found.sort();
    assert_eq!(found, ["WEBS", "WEBS1"]);
}

pub async fn project_members_are_saved_and_listed_by_name(store: impl Store) {
    let zed = a_saved_person(&store, "Zed").await;
    let ada = a_saved_person(&store, "Ada").await;
    let bob = a_saved_person(&store, "Bob").await;
    let v1 = a_project(zed).build();
    let v2 = v1.with_member(ada).with_member(bob);
    let v3 = v2.without_member(zed);
    let mut tx = store.begin().await.unwrap();
    tx.projects().save(None, &v1, &ctx()).await.unwrap();
    tx.projects().save(Some(&v1), &v2, &ctx()).await.unwrap();
    tx.projects().save(Some(&v2), &v3, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(tx.projects().load(v1.id()).await.unwrap(), v3);
    drop(tx);
    let members: Vec<_> = store
        .project_members(v1.id())
        .await
        .unwrap()
        .into_iter()
        .map(|m| (m.id, m.display_name))
        .collect();
    assert_eq!(members, [(ada, "Ada".to_owned()), (bob, "Bob".to_owned())]);
    assert_eq!(
        store.get_project(v1.id()).await.unwrap().project_leader,
        None
    );
    assert_eq!(
        store.project_members(ProjectId::new()).await.unwrap_err(),
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
            a_person_can_be_fetched_by_id,
            saved_project_can_be_loaded_and_updated,
            a_project_code_can_be_used_only_once,
            each_project_save_writes_a_history_entry,
            projects_are_listed_without_archived_ones_by_default,
            project_codes_can_be_found_by_prefix,
            project_members_are_saved_and_listed_by_name
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
