use lodestar_core::history::EntityType;
use lodestar_core::person::{Person, PersonId, use_cases::create_person};
use lodestar_core::ports::{HistoryQuery, PageRequest, ProjectsQuery, RepoError};
use lodestar_core::project::Project;
use lodestar_core::project::members::{add_member, project_members, remove_member};
use lodestar_core::project::use_cases::{
    NewProject, ProjectEdit, ProjectError, create_project, set_archived, update_project,
};
use lodestar_core::test_support::InMemoryStore;

struct World {
    store: InMemoryStore,
    ada: Person,
    bob: Person,
    project: Project,
}

/// Ada creates a Project (and so leads it); Bob exists but is not a Member.
async fn world() -> World {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let bob = create_person(&store, None, "Bob").await.unwrap();
    let project = create_project(
        &store,
        ada.id(),
        NewProject {
            name: "Web",
            description: None,
            project_code: "WEBR",
        },
    )
    .await
    .unwrap();
    World {
        store,
        ada,
        bob,
        project,
    }
}

fn lead(name: &str, leader: PersonId) -> ProjectEdit<'_> {
    ProjectEdit {
        name,
        description: None,
        health_rating: None,
        project_leader: Some(leader),
    }
}

async fn member_names(w: &World) -> Vec<String> {
    project_members(&w.store, w.project.id())
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.display_name)
        .collect()
}

#[tokio::test]
async fn any_person_can_be_added_as_a_member() {
    let w = world().await;
    let after = add_member(&w.store, w.ada.id(), w.project.id(), 1, w.bob.id())
        .await
        .unwrap();
    assert_eq!(after.version(), 2);
    assert!(after.members().contains(&w.bob.id()));
    assert_eq!(member_names(&w).await, ["Ada", "Bob"]);
}

#[tokio::test]
async fn adding_an_existing_member_changes_nothing() {
    let w = world().await;
    let after = add_member(&w.store, w.ada.id(), w.project.id(), 1, w.ada.id())
        .await
        .unwrap();
    assert_eq!(after.version(), 1);
}

#[tokio::test]
async fn adding_an_unknown_person_fails() {
    let w = world().await;
    assert_eq!(
        add_member(&w.store, w.ada.id(), w.project.id(), 1, PersonId::new())
            .await
            .unwrap_err(),
        ProjectError::UnknownPerson
    );
}

#[tokio::test]
async fn member_changes_need_the_current_version() {
    let w = world().await;
    assert_eq!(
        add_member(&w.store, w.ada.id(), w.project.id(), 7, w.bob.id())
            .await
            .unwrap_err(),
        ProjectError::Repo(RepoError::Conflict)
    );
}

#[tokio::test]
async fn a_member_can_be_removed_and_added_back() {
    let w = world().await;
    add_member(&w.store, w.ada.id(), w.project.id(), 1, w.bob.id())
        .await
        .unwrap();
    let removed = remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
        .await
        .unwrap();
    assert_eq!(removed.version(), 3);
    assert_eq!(member_names(&w).await, ["Ada"]);
    add_member(&w.store, w.ada.id(), w.project.id(), 3, w.bob.id())
        .await
        .unwrap();
    assert_eq!(member_names(&w).await, ["Ada", "Bob"]);
}

#[tokio::test]
async fn removing_a_non_member_fails() {
    let w = world().await;
    assert_eq!(
        remove_member(&w.store, w.ada.id(), w.project.id(), 1, w.bob.id())
            .await
            .unwrap_err(),
        ProjectError::NotAMember
    );
}

#[tokio::test]
async fn removing_the_leader_leaves_no_leader() {
    let w = world().await;
    let after = remove_member(&w.store, w.ada.id(), w.project.id(), 1, w.ada.id())
        .await
        .unwrap();
    assert_eq!(after.project_leader(), None);
    let view = w.store.get_project(w.project.id()).await.unwrap();
    assert_eq!(view.project_leader, None);
}

#[tokio::test]
async fn the_leader_can_be_changed_to_another_member() {
    let w = world().await;
    add_member(&w.store, w.ada.id(), w.project.id(), 1, w.bob.id())
        .await
        .unwrap();
    let after = update_project(
        &w.store,
        w.ada.id(),
        w.project.id(),
        2,
        lead("Web", w.bob.id()),
    )
    .await
    .unwrap();
    assert_eq!(after.project_leader(), Some(w.bob.id()));
    assert_eq!(after.version(), 3);
}

#[tokio::test]
async fn only_a_member_can_lead() {
    let w = world().await;
    assert_eq!(
        update_project(
            &w.store,
            w.ada.id(),
            w.project.id(),
            1,
            lead("Web", w.bob.id()),
        )
        .await
        .unwrap_err(),
        ProjectError::LeaderNotAMember
    );
}

#[tokio::test]
async fn an_archived_project_rejects_member_changes() {
    let w = world().await;
    set_archived(&w.store, w.ada.id(), w.project.id(), 1, true)
        .await
        .unwrap();
    assert_eq!(
        add_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
            .await
            .unwrap_err(),
        ProjectError::Archived
    );
    assert_eq!(
        remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.ada.id())
            .await
            .unwrap_err(),
        ProjectError::Archived
    );
}

#[tokio::test]
async fn member_changes_are_in_the_change_history() {
    let w = world().await;
    add_member(&w.store, w.ada.id(), w.project.id(), 1, w.bob.id())
        .await
        .unwrap();
    remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
        .await
        .unwrap();
    let history = w
        .store
        .entity_history(
            EntityType::Project,
            w.project.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let members = |v: &serde_json::Value| v["members"].as_array().unwrap().len();
    let sizes: Vec<_> = history.items.iter().map(|e| members(&e.after)).collect();
    assert_eq!(sizes, [1, 2, 1]);
}
