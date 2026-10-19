//! Port contract suite (#12). Every adapter, fake or real, must pass each case.
//! Each case takes a fresh, empty store.

use uuid::Uuid;

use crate::action::{Action, ActionId};
use crate::blocker::{Blocker, BlockerId};

use crate::history::{ChangeContext, ChangeKind, EntityType, snapshot};
use crate::person::PersonId;
use crate::ports::{
    ActionFilter, ActionRepository, ActionSort, ActionsQuery, BlockerFilter, BlockerRepository,
    BlockerSort, BlockersQuery, ClearedFilter, ClosedFilter, HistoryQuery, PageRequest,
    PeopleQuery, PeopleSort, PersonRepository, ProjectRepository, ProjectSort, ProjectsQuery,
    ReferenceCounter, ReferenceKind, RepoError, RiskFilter, RiskRepository, RiskSort, RisksQuery,
    Transaction, UnitOfWork,
};
use crate::project::{Description, Project, ProjectId, ProjectName};
use crate::risk::{Rating, RiskId};
use crate::test_support::{a_person, a_project, a_risk};

/// Everything a full store adapter provides.
pub trait Store:
    UnitOfWork + PeopleQuery + ProjectsQuery + RisksQuery + ActionsQuery + BlockersQuery + HistoryQuery
{
}
impl<
    T: UnitOfWork
        + PeopleQuery
        + ProjectsQuery
        + RisksQuery
        + ActionsQuery
        + BlockersQuery
        + HistoryQuery,
> Store for T
{
}

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
        .edited(ProjectName::new("Website").unwrap(), None, None, None)
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

pub async fn project_history_holds_the_project_and_its_items_in_time_order(store: impl Store) {
    let project = a_saved_project(&store, "WEBR").await;
    let other = a_saved_project(&store, "DOCS").await;
    let risk = a_risk(project.id(), 1).build();
    let elsewhere = a_risk(other.id(), 1).build();
    let mut tx = store.begin().await.unwrap();
    tx.risks().save(None, &risk, &ctx()).await.unwrap();
    tx.risks().save(None, &elsewhere, &ctx()).await.unwrap();
    tx.risks()
        .save(Some(&risk), &risk.with_closed(true), &ctx())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let history = store.project_history(project.id()).await.unwrap();
    let seen: Vec<(EntityType, Uuid, u32)> = history
        .iter()
        .map(|e| (e.entity_type, e.entity_id, e.version))
        .collect();
    assert_eq!(
        seen,
        [
            (EntityType::Project, project.id().as_uuid(), 1),
            (EntityType::Risk, risk.id().as_uuid(), 1),
            (EntityType::Risk, risk.id().as_uuid(), 2),
        ]
    );
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

/// Saves a Project, with a Person to lead it.
async fn a_saved_project(store: &impl Store, code: &str) -> Project {
    let ada = a_saved_person(store, "Ada").await;
    let project = a_project(ada).coded(code).build();
    let mut tx = store.begin().await.unwrap();
    tx.projects().save(None, &project, &ctx()).await.unwrap();
    tx.commit().await.unwrap();
    project
}

pub async fn saved_risk_can_be_loaded_and_updated(store: impl Store) {
    let project = a_saved_project(&store, "WEBR").await;
    let v1 = a_risk(project.id(), 1).described("Vendor late").build();
    let v2 = v1.edited(
        Description::new("Vendor very late").unwrap(),
        Rating::new(5).unwrap(),
        Rating::new(4).unwrap(),
    );
    let v3 = v2.with_closed(true);
    let mut tx = store.begin().await.unwrap();
    tx.risks().save(None, &v1, &ctx()).await.unwrap();
    assert_eq!(tx.risks().load(v1.id()).await.unwrap(), v1);
    tx.risks().save(Some(&v1), &v2, &ctx()).await.unwrap();
    tx.risks().save(Some(&v2), &v3, &ctx()).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = store.begin().await.unwrap();
    assert_eq!(tx.risks().load(v1.id()).await.unwrap(), v3);
    assert_eq!(
        tx.risks().load(RiskId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
    assert_eq!(
        tx.risks().save(Some(&v1), &v2, &ctx()).await.unwrap_err(),
        RepoError::Conflict
    );
    assert_eq!(
        tx.risks().save(None, &v1, &ctx()).await.unwrap_err(),
        RepoError::Conflict
    );
}

pub async fn each_risk_save_writes_a_history_entry(store: impl Store) {
    let project = a_saved_project(&store, "WEBR").await;
    let actor = PersonId::new();
    let v1 = a_risk(project.id(), 1).build();
    let v2 = v1.with_closed(true);
    let mut tx = store.begin().await.unwrap();
    tx.risks()
        .save(None, &v1, &ChangeContext::by(actor))
        .await
        .unwrap();
    tx.risks()
        .save(Some(&v1), &v2, &ChangeContext::by(actor))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let history = store
        .entity_history(EntityType::Risk, v1.id().as_uuid(), PageRequest::first(10))
        .await
        .unwrap();
    let [first, second] = history.items.as_slice() else {
        panic!("expected two entries, got {:?}", history.items);
    };
    assert_eq!(first.change_kind, ChangeKind::Created);
    assert_eq!(first.project_id, Some(project.id().as_uuid()));
    assert_eq!(first.acting_person, actor);
    assert_eq!(first.after, snapshot(&v1));
    assert_eq!(second.change_kind, ChangeKind::Updated);
    assert_eq!(second.before, Some(snapshot(&v1)));
    assert_eq!(second.after, snapshot(&v2));
}

pub async fn risks_are_listed_by_project_state_and_score(store: impl Store) {
    let web = a_saved_project(&store, "WEBR").await;
    let other = a_saved_project(&store, "OTHR").await;
    // Built out of Reference order, so ID order and Reference order differ.
    let mid = a_risk(web.id(), 3).described("Mid").rated(3, 3).build();
    let low = a_risk(web.id(), 1).described("Low").rated(1, 2).build();
    let high = a_risk(web.id(), 2).described("High").rated(5, 4).build();
    let shut = a_risk(web.id(), 4).described("Shut").rated(5, 5).build();
    let elsewhere = a_risk(other.id(), 1).described("Elsewhere").build();
    let mut tx = store.begin().await.unwrap();
    for r in [&mid, &low, &high, &shut, &elsewhere] {
        tx.risks().save(None, r, &ctx()).await.unwrap();
    }
    tx.risks()
        .save(Some(&shut), &shut.with_closed(true), &ctx())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let in_web = |include_closed| RiskFilter {
        project_id: Some(web.id()),
        state: if include_closed {
            ClosedFilter::All
        } else {
            ClosedFilter::Open
        },
    };
    let all = PageRequest::first(10);
    let names = |page: crate::ports::Page<crate::risk::RiskView>| -> (u64, Vec<String>) {
        (
            page.total,
            page.items.into_iter().map(|r| r.description).collect(),
        )
    };
    let by_id = store.list_risks(all, in_web(false), RiskSort::Id).await;
    assert_eq!(
        names(by_id.unwrap()),
        (3, vec!["Mid".into(), "Low".into(), "High".into()])
    );
    let by_score = store.list_risks(all, in_web(false), RiskSort::Score).await;
    assert_eq!(
        names(by_score.unwrap()),
        (3, vec!["High".into(), "Mid".into(), "Low".into()])
    );
    let paged = PageRequest {
        offset: 1,
        limit: 2,
    };
    let by_reference = store
        .list_risks(paged, in_web(true), RiskSort::Reference)
        .await;
    assert_eq!(
        names(by_reference.unwrap()),
        (4, vec!["High".into(), "Mid".into()])
    );
    let everywhere = RiskFilter {
        project_id: None,
        state: ClosedFilter::All,
    };
    let everything = store.list_risks(all, everywhere, RiskSort::Id).await;
    assert_eq!(everything.unwrap().total, 5);

    let view = store.get_risk(high.id()).await.unwrap();
    assert_eq!(view.project_id, web.id());
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.reference, "R-002");
    assert_eq!(view.description, "High");
    assert_eq!((view.likelihood, view.impact, view.score), (5, 4, 20));
    assert!(!view.closed);
    assert_eq!(view.version, 1);
    assert!(store.get_risk(shut.id()).await.unwrap().closed);
    assert_eq!(
        store.get_risk(RiskId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
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

/// Saves two People and a Project led by the first with both as Members.
async fn a_saved_project_with(store: &impl Store, code: &str) -> (ProjectId, PersonId, PersonId) {
    let ada = a_saved_person(store, "Ada").await;
    let bob = a_saved_person(store, "Bob").await;
    let project = a_project(ada).coded(code).build().with_member(bob);
    let mut tx = store.begin().await.unwrap();
    tx.projects().save(None, &project, &ctx()).await.unwrap();
    tx.commit().await.unwrap();
    (project.id(), ada, bob)
}

fn an_action(project: ProjectId, reference: u32, assignee: Option<PersonId>) -> Action {
    Action::create(
        project,
        reference,
        Description::new(format!("Action {reference}\nsecond line")).unwrap(),
        assignee,
        Some("2030-01-31".parse().unwrap()),
    )
}

pub async fn saved_action_can_be_loaded_updated_and_viewed(store: impl Store) {
    let (project, _ada, bob) = a_saved_project_with(&store, "WEBR").await;
    let v1 = an_action(project, 7, Some(bob));
    let v2 = v1.closed_now();
    let mut tx = store.begin().await.unwrap();
    let created = tx
        .actions()
        .save(None, &v1, &ChangeContext::by(bob))
        .await
        .unwrap();
    assert_eq!(tx.actions().load(v1.id()).await.unwrap(), v1);
    tx.actions()
        .save(Some(&v1), &v2, &ChangeContext::by(bob).caused_by(created))
        .await
        .unwrap();
    assert_eq!(
        tx.actions().save(Some(&v1), &v2, &ctx()).await.unwrap_err(),
        RepoError::Conflict
    );
    assert_eq!(
        tx.actions().load(ActionId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
    tx.commit().await.unwrap();

    let view = store.get_action(v1.id()).await.unwrap();
    assert_eq!(view.reference, "A-007");
    assert_eq!(view.project_id, project);
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.description, "Action 7\nsecond line");
    let assignee = view.assignee.unwrap();
    assert_eq!((assignee.id, assignee.display_name.as_str()), (bob, "Bob"));
    assert_eq!(view.due_date, Some("2030-01-31".parse().unwrap()));
    assert!(view.closed);
    assert_eq!(view.version, 2);
    assert_eq!(
        store.get_action(ActionId::new()).await.unwrap_err(),
        RepoError::NotFound
    );

    let history = store
        .entity_history(
            EntityType::Action,
            v1.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let [first, second] = history.items.as_slice() else {
        panic!("expected two entries, got {:?}", history.items);
    };
    assert_eq!(first.project_id, Some(project.as_uuid()));
    assert_eq!(first.after, snapshot(&v1));
    assert_eq!(second.before, Some(snapshot(&v1)));
    assert_eq!(second.caused_by, Some(created));
}

pub async fn open_actions_assigned_to_a_person_can_be_found(store: impl Store) {
    let (project, ada, bob) = a_saved_project_with(&store, "WEBR").await;
    let (other, _, _) = a_saved_project_with(&store, "OTHR").await;
    let open = an_action(project, 1, Some(bob));
    let closed = an_action(project, 2, Some(bob)).closed_now();
    let adas = an_action(project, 3, Some(ada));
    let elsewhere = an_action(other, 1, Some(bob));
    let mut tx = store.begin().await.unwrap();
    for a in [&open, &closed, &adas, &elsewhere] {
        tx.actions().save(None, a, &ctx()).await.unwrap();
    }
    let found: Vec<_> = tx
        .actions()
        .open_assigned_to(project, bob)
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.id())
        .collect();
    assert_eq!(found, [open.id()]);
}

pub async fn actions_are_listed_by_project_code_and_reference(store: impl Store) {
    let (web, _, _) = a_saved_project_with(&store, "WEBR").await;
    let (alpha, _, _) = a_saved_project_with(&store, "ALPH").await;
    let mut tx = store.begin().await.unwrap();
    for a in [
        an_action(web, 10, None),
        an_action(web, 2, None),
        an_action(web, 3, None).closed_now(),
        an_action(alpha, 1, None),
    ] {
        tx.actions().save(None, &a, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let labels = |page: crate::ports::Page<crate::action::ActionView>| -> Vec<String> {
        page.items
            .into_iter()
            .map(|a| format!("{}/{}", a.project_code, a.reference))
            .collect()
    };
    let open = store
        .list_actions(
            ActionFilter::default(),
            PageRequest::first(10),
            ActionSort::Reference,
        )
        .await
        .unwrap();
    assert_eq!(open.total, 3);
    assert_eq!(labels(open), ["ALPH/A-001", "WEBR/A-002", "WEBR/A-010"]);
    let web_all = store
        .list_actions(
            ActionFilter {
                project_id: Some(web),
                state: ClosedFilter::All,
                ..ActionFilter::default()
            },
            PageRequest {
                offset: 1,
                limit: 5,
            },
            ActionSort::Reference,
        )
        .await
        .unwrap();
    assert_eq!(web_all.total, 3);
    assert_eq!(labels(web_all), ["WEBR/A-003", "WEBR/A-010"]);
}

fn a_blocker(project: ProjectId, reference: u32, accountable: Option<PersonId>) -> Blocker {
    Blocker::create(
        project,
        reference,
        Description::new(format!("Blocker {reference}\nsecond line")).unwrap(),
        accountable,
    )
}

pub async fn saved_blocker_with_links_can_be_loaded_updated_and_viewed(store: impl Store) {
    let (project, _ada, bob) = a_saved_project_with(&store, "WEBR").await;
    let a2 = an_action(project, 2, None);
    let a1 = an_action(project, 1, None);
    let v1 = a_blocker(project, 4, Some(bob));
    let v2 = v1.linked_to(a2.id()).linked_to(a1.id());
    let v3 = v2.unlinked_from(a2.id());
    let mut tx = store.begin().await.unwrap();
    tx.actions().save(None, &a1, &ctx()).await.unwrap();
    tx.actions().save(None, &a2, &ctx()).await.unwrap();
    let created = tx
        .blockers()
        .save(None, &v1, &ChangeContext::by(bob))
        .await
        .unwrap();
    assert_eq!(tx.blockers().load(v1.id()).await.unwrap(), v1);
    tx.blockers()
        .save(Some(&v1), &v2, &ChangeContext::by(bob).caused_by(created))
        .await
        .unwrap();
    assert_eq!(tx.blockers().load(v1.id()).await.unwrap(), v2);
    assert_eq!(
        tx.blockers()
            .save(Some(&v1), &v2, &ctx())
            .await
            .unwrap_err(),
        RepoError::Conflict
    );
    assert_eq!(
        tx.blockers().load(BlockerId::new()).await.unwrap_err(),
        RepoError::NotFound
    );
    tx.commit().await.unwrap();

    let view = store.get_blocker(v1.id()).await.unwrap();
    assert_eq!(view.reference, "B-004");
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.description, "Blocker 4\nsecond line");
    assert_eq!(view.accountable.unwrap().display_name, "Bob");
    assert!(!view.cleared);
    assert_eq!(view.version, 3);
    let refs: Vec<_> = view
        .blocked_actions
        .iter()
        .map(|a| (a.id, a.reference.as_str()))
        .collect();
    assert_eq!(
        refs,
        [(a1.id().as_uuid(), "A-001"), (a2.id().as_uuid(), "A-002")]
    );
    assert_eq!(
        store.get_blocker(BlockerId::new()).await.unwrap_err(),
        RepoError::NotFound
    );

    let mut tx = store.begin().await.unwrap();
    tx.blockers().save(Some(&v2), &v3, &ctx()).await.unwrap();
    assert_eq!(tx.blockers().load(v1.id()).await.unwrap(), v3);
    tx.commit().await.unwrap();

    let history = store
        .entity_history(
            EntityType::Blocker,
            v1.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let [first, second, _] = history.items.as_slice() else {
        panic!("expected three entries, got {:?}", history.items);
    };
    assert_eq!(first.project_id, Some(project.as_uuid()));
    assert_eq!(second.after, snapshot(&v2));
    assert_eq!(second.caused_by, Some(created));
}

pub async fn actions_show_the_open_blockers_blocking_them(store: impl Store) {
    let (project, _, _) = a_saved_project_with(&store, "WEBR").await;
    let action = an_action(project, 1, None);
    let open2 = a_blocker(project, 2, None).linked_to(action.id());
    let open1 = a_blocker(project, 1, None).linked_to(action.id());
    let cleared = a_blocker(project, 3, None)
        .linked_to(action.id())
        .cleared_now();
    let mut tx = store.begin().await.unwrap();
    tx.actions().save(None, &action, &ctx()).await.unwrap();
    for b in [&open2, &open1, &cleared] {
        tx.blockers().save(None, b, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let refs = |v: crate::action::ActionView| -> Vec<String> {
        v.blocked_by.into_iter().map(|b| b.reference).collect()
    };
    assert_eq!(
        refs(store.get_action(action.id()).await.unwrap()),
        ["B-001", "B-002"]
    );
    let listed = store
        .list_actions(
            ActionFilter::default(),
            PageRequest::first(10),
            ActionSort::Reference,
        )
        .await
        .unwrap();
    assert_eq!(refs(listed.items[0].clone()), ["B-001", "B-002"]);
}

pub async fn open_blockers_a_person_is_accountable_for_can_be_found(store: impl Store) {
    let (project, ada, bob) = a_saved_project_with(&store, "WEBR").await;
    let (other, _, _) = a_saved_project_with(&store, "OTHR").await;
    let open = a_blocker(project, 1, Some(bob));
    let cleared = a_blocker(project, 2, Some(bob)).cleared_now();
    let adas = a_blocker(project, 3, Some(ada));
    let elsewhere = a_blocker(other, 1, Some(bob));
    let mut tx = store.begin().await.unwrap();
    for b in [&open, &cleared, &adas, &elsewhere] {
        tx.blockers().save(None, b, &ctx()).await.unwrap();
    }
    let found: Vec<_> = tx
        .blockers()
        .open_accountable(project, bob)
        .await
        .unwrap()
        .into_iter()
        .map(|b| b.id())
        .collect();
    assert_eq!(found, [open.id()]);
}

pub async fn blockers_are_listed_by_project_code_and_reference(store: impl Store) {
    let (web, _, _) = a_saved_project_with(&store, "WEBR").await;
    let (alpha, _, _) = a_saved_project_with(&store, "ALPH").await;
    let mut tx = store.begin().await.unwrap();
    for b in [
        a_blocker(web, 10, None),
        a_blocker(web, 2, None),
        a_blocker(web, 3, None).cleared_now(),
        a_blocker(alpha, 1, None),
    ] {
        tx.blockers().save(None, &b, &ctx()).await.unwrap();
    }
    tx.commit().await.unwrap();

    let labels = |page: crate::ports::Page<crate::blocker::BlockerView>| -> Vec<String> {
        page.items
            .into_iter()
            .map(|b| format!("{}/{}", b.project_code, b.reference))
            .collect()
    };
    let open = store
        .list_blockers(
            BlockerFilter::default(),
            PageRequest::first(10),
            BlockerSort::Reference,
        )
        .await
        .unwrap();
    assert_eq!(open.total, 3);
    assert_eq!(labels(open), ["ALPH/B-001", "WEBR/B-002", "WEBR/B-010"]);
    let web_all = store
        .list_blockers(
            BlockerFilter {
                project_id: Some(web),
                state: ClearedFilter::All,
                ..BlockerFilter::default()
            },
            PageRequest {
                offset: 1,
                limit: 5,
            },
            BlockerSort::Reference,
        )
        .await
        .unwrap();
    assert_eq!(web_all.total, 3);
    assert_eq!(labels(web_all), ["WEBR/B-003", "WEBR/B-010"]);
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
            saved_risk_can_be_loaded_and_updated,
            each_risk_save_writes_a_history_entry,
            risks_are_listed_by_project_state_and_score,
            project_members_are_saved_and_listed_by_name,
            saved_action_can_be_loaded_updated_and_viewed,
            open_actions_assigned_to_a_person_can_be_found,
            actions_are_listed_by_project_code_and_reference,
            saved_blocker_with_links_can_be_loaded_updated_and_viewed,
            actions_show_the_open_blockers_blocking_them,
            open_blockers_a_person_is_accountable_for_can_be_found,
            blockers_are_listed_by_project_code_and_reference,
            project_history_holds_the_project_and_its_items_in_time_order
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
