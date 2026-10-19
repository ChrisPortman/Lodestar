use lodestar_core::action::Action;
use lodestar_core::action::use_cases::{NewAction, create_action, get_action};
use lodestar_core::blocker::use_cases::{
    BlockerEdit, BlockerError, BlockerFilter, BlockerSort, NewBlocker, create_blocker, get_blocker,
    link_blocked_action, list_blockers, set_blocker_cleared, unlink_blocked_action, update_blocker,
};
use lodestar_core::blocker::{Blocker, BlockerId};
use lodestar_core::history::{ChangeKind, EntityType, HistoryEntry};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::ports::{ClearedFilter, HistoryQuery, PageRequest, RepoError};
use lodestar_core::project::members::{add_member, remove_member};
use lodestar_core::project::use_cases::{NewProject, create_project, set_archived};
use lodestar_core::project::{Project, ProjectId};
use lodestar_core::test_support::InMemoryStore;

struct World {
    store: InMemoryStore,
    ada: Person,
    bob: Person,
    project: Project,
}

/// Ada creates a Project (and so leads it); Bob is added as a Member.
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
    let project = add_member(&store, ada.id(), project.id(), 1, bob.id())
        .await
        .unwrap();
    World {
        store,
        ada,
        bob,
        project,
    }
}

impl World {
    async fn new_blocker(&self, description: &str, accountable: Option<&Person>) -> Blocker {
        create_blocker(
            &self.store,
            self.ada.id(),
            NewBlocker {
                project_id: self.project.id(),
                description,
                accountable: accountable.map(Person::id),
            },
        )
        .await
        .unwrap()
    }

    async fn new_action_in(&self, project: ProjectId, description: &str) -> Action {
        create_action(
            &self.store,
            self.ada.id(),
            NewAction {
                project_id: project,
                description,
                assignee: None,
                due_date: None,
            },
        )
        .await
        .unwrap()
    }

    async fn history(&self, id: BlockerId) -> Vec<HistoryEntry> {
        self.store
            .entity_history(EntityType::Blocker, id.as_uuid(), PageRequest::first(50))
            .await
            .unwrap()
            .items
    }

    async fn blocked_by(&self, action: &Action) -> Vec<String> {
        get_action(&self.store, action.id())
            .await
            .unwrap()
            .blocked_by
            .into_iter()
            .map(|b| b.reference)
            .collect()
    }
}

#[tokio::test]
async fn a_new_blocker_gets_the_next_reference_in_its_project() {
    let w = world().await;
    let first = w.new_blocker("Waiting on vendor", None).await;
    let second = w.new_blocker("No test env", Some(&w.bob)).await;
    assert_eq!(first.reference_label(), "B-001");
    assert_eq!(second.reference_label(), "B-002");
    assert!(!second.cleared());

    let view = get_blocker(&w.store, second.id()).await.unwrap();
    assert_eq!(view.reference, "B-002");
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.accountable.unwrap().display_name, "Bob");
    assert!(view.blocked_actions.is_empty());
}

#[tokio::test]
async fn the_accountable_person_must_be_a_member() {
    let w = world().await;
    let outsider = create_person(&w.store, None, "Cy").await.unwrap();
    let err = create_blocker(
        &w.store,
        w.ada.id(),
        NewBlocker {
            project_id: w.project.id(),
            description: "x",
            accountable: Some(outsider.id()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, BlockerError::AccountableNotAMember);

    let blocker = w.new_blocker("x", None).await;
    let err = update_blocker(
        &w.store,
        w.ada.id(),
        blocker.id(),
        1,
        BlockerEdit {
            description: "x",
            accountable: Some(outsider.id()),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, BlockerError::AccountableNotAMember);
}

#[tokio::test]
async fn a_blocker_can_be_edited_cleared_and_reopened() {
    let w = world().await;
    let blocker = w.new_blocker("Old", None).await;
    let edited = update_blocker(
        &w.store,
        w.ada.id(),
        blocker.id(),
        1,
        BlockerEdit {
            description: "New",
            accountable: Some(w.bob.id()),
        },
    )
    .await
    .unwrap();
    assert_eq!(edited.description().as_str(), "New");
    assert_eq!(edited.accountable(), Some(w.bob.id()));

    let stale = set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 1, true)
        .await
        .unwrap_err();
    assert_eq!(stale, BlockerError::Repo(RepoError::Conflict));
    let cleared = set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 2, true)
        .await
        .unwrap();
    assert!(cleared.cleared());
    let reopened = set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 3, false)
        .await
        .unwrap();
    assert!(!reopened.cleared());
    assert_eq!(reopened.version(), 4);
    assert_eq!(w.history(blocker.id()).await.len(), 4);
}

#[tokio::test]
async fn an_open_blocker_shows_on_the_actions_it_blocks() {
    let w = world().await;
    let action = w.new_action_in(w.project.id(), "Deploy").await;
    let blocker = w.new_blocker("No env", None).await;

    let linked = link_blocked_action(&w.store, w.ada.id(), blocker.id(), 1, action.id())
        .await
        .unwrap();
    assert_eq!(linked.version(), 2);
    let view = get_blocker(&w.store, blocker.id()).await.unwrap();
    let refs: Vec<_> = view.blocked_actions.iter().map(|a| &a.reference).collect();
    assert_eq!(refs, ["A-001"]);
    assert_eq!(w.blocked_by(&action).await, ["B-001"]);

    let again = link_blocked_action(&w.store, w.ada.id(), blocker.id(), 2, action.id())
        .await
        .unwrap();
    assert_eq!(again.version(), 2, "linking twice changes nothing");

    set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 2, true)
        .await
        .unwrap();
    assert!(
        w.blocked_by(&action).await.is_empty(),
        "a cleared blocker does not block"
    );
}

#[tokio::test]
async fn links_change_in_any_state_and_are_recorded() {
    let w = world().await;
    let action = w.new_action_in(w.project.id(), "Deploy").await;
    let blocker = w.new_blocker("No env", None).await;
    set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 1, true)
        .await
        .unwrap();
    link_blocked_action(&w.store, w.ada.id(), blocker.id(), 2, action.id())
        .await
        .unwrap();
    let unlinked = unlink_blocked_action(&w.store, w.bob.id(), blocker.id(), 3, action.id())
        .await
        .unwrap();
    assert!(unlinked.blocked_actions().is_empty());
    assert_eq!(unlinked.version(), 4);

    let history = w.history(blocker.id()).await;
    let last = history.last().unwrap();
    assert_eq!(last.change_kind, ChangeKind::Updated);
    assert_eq!(last.acting_person, w.bob.id());
    assert_eq!(
        last.before.as_ref().unwrap()["blocked_actions"][0],
        action.id().to_string()
    );
}

#[tokio::test]
async fn a_blocker_cannot_block_an_action_in_another_project() {
    let w = world().await;
    let other = create_project(
        &w.store,
        w.ada.id(),
        NewProject {
            name: "Other",
            description: None,
            project_code: "OTHR",
        },
    )
    .await
    .unwrap();
    let elsewhere = w.new_action_in(other.id(), "Elsewhere").await;
    let blocker = w.new_blocker("No env", None).await;
    let err = link_blocked_action(&w.store, w.ada.id(), blocker.id(), 1, elsewhere.id())
        .await
        .unwrap_err();
    assert_eq!(err, BlockerError::ActionInAnotherProject);
    let err = link_blocked_action(
        &w.store,
        w.ada.id(),
        blocker.id(),
        1,
        lodestar_core::action::ActionId::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(err, BlockerError::UnknownAction);
}

#[tokio::test]
async fn blockers_in_an_archived_project_cannot_be_changed() {
    let w = world().await;
    let action = w.new_action_in(w.project.id(), "Deploy").await;
    let blocker = w.new_blocker("No env", None).await;
    set_archived(&w.store, w.ada.id(), w.project.id(), 2, true)
        .await
        .unwrap();
    let err = w_create(&w).await.unwrap_err();
    assert_eq!(err, BlockerError::Archived);
    let err = set_blocker_cleared(&w.store, w.ada.id(), blocker.id(), 1, true)
        .await
        .unwrap_err();
    assert_eq!(err, BlockerError::Archived);
    let err = link_blocked_action(&w.store, w.ada.id(), blocker.id(), 1, action.id())
        .await
        .unwrap_err();
    assert_eq!(err, BlockerError::Archived);
}

async fn w_create(w: &World) -> Result<Blocker, BlockerError> {
    create_blocker(
        &w.store,
        w.ada.id(),
        NewBlocker {
            project_id: w.project.id(),
            description: "More",
            accountable: None,
        },
    )
    .await
}

#[tokio::test]
async fn removing_a_member_unassigns_their_open_blockers() {
    let w = world().await;
    let open = w.new_blocker("Open", Some(&w.bob)).await;
    let cleared = w.new_blocker("Cleared", Some(&w.bob)).await;
    set_blocker_cleared(&w.store, w.ada.id(), cleared.id(), 1, true)
        .await
        .unwrap();

    remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
        .await
        .unwrap();

    let open_after = get_blocker(&w.store, open.id()).await.unwrap();
    assert_eq!(open_after.accountable, None);
    let cleared_after = get_blocker(&w.store, cleared.id()).await.unwrap();
    assert_eq!(
        cleared_after.accountable.map(|a| a.id),
        Some(w.bob.id()),
        "cleared blockers keep their accountable person (FR-22)"
    );

    let removal = w
        .store
        .entity_history(
            EntityType::Project,
            w.project.id().as_uuid(),
            PageRequest::first(50),
        )
        .await
        .unwrap()
        .items
        .pop()
        .unwrap();
    let unassigned = w.history(open.id()).await.pop().unwrap();
    assert_eq!(unassigned.caused_by, Some(removal.id));

    let reopened = set_blocker_cleared(&w.store, w.ada.id(), cleared.id(), 2, false)
        .await
        .unwrap();
    assert_eq!(reopened.accountable(), None);
}

#[tokio::test]
async fn open_blockers_are_listed_by_reference_and_cleared_ones_on_request() {
    let w = world().await;
    let b1 = w.new_blocker("One", None).await;
    w.new_blocker("Two", None).await;
    set_blocker_cleared(&w.store, w.ada.id(), b1.id(), 1, true)
        .await
        .unwrap();
    let labels = |include_cleared| {
        let store = w.store.clone();
        let project_id = Some(w.project.id());
        async move {
            list_blockers(
                &store,
                BlockerFilter {
                    project_id,
                    state: if include_cleared {
                        ClearedFilter::All
                    } else {
                        ClearedFilter::Open
                    },
                    ..BlockerFilter::default()
                },
                PageRequest::first(10),
                BlockerSort::Reference,
            )
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|b| b.reference)
            .collect::<Vec<_>>()
        }
    };
    assert_eq!(labels(false).await, ["B-002"]);
    assert_eq!(labels(true).await, ["B-001", "B-002"]);
}
