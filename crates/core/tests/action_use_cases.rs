use chrono::NaiveDate;
use lodestar_core::action::use_cases::{
    ActionEdit, ActionError, ActionFilter, NewAction, create_action, get_action, list_actions,
    set_action_closed, update_action,
};
use lodestar_core::action::{Action, ActionId};
use lodestar_core::history::{ChangeKind, EntityType};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::ports::{HistoryQuery, PageRequest, RepoError};
use lodestar_core::project::Project;
use lodestar_core::project::members::{add_member, remove_member};
use lodestar_core::project::use_cases::{NewProject, create_project, set_archived};
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

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

impl World {
    async fn new_action(&self, description: &str, assignee: Option<&Person>) -> Action {
        create_action(
            &self.store,
            self.ada.id(),
            NewAction {
                project_id: self.project.id(),
                description,
                assignee: assignee.map(Person::id),
                due_date: None,
            },
        )
        .await
        .unwrap()
    }

    async fn history(&self, id: ActionId) -> Vec<lodestar_core::history::HistoryEntry> {
        self.store
            .entity_history(EntityType::Action, id.as_uuid(), PageRequest::first(50))
            .await
            .unwrap()
            .items
    }
}

#[tokio::test]
async fn a_new_action_gets_the_next_reference_in_its_project() {
    let w = world().await;
    let first = w.new_action("Draft plan", None).await;
    let second = w.new_action("Book room", Some(&w.bob)).await;
    assert_eq!(first.reference_label(), "A-001");
    assert_eq!(second.reference_label(), "A-002");
    assert!(!second.closed());
    assert_eq!(second.version(), 1);

    let view = get_action(&w.store, second.id()).await.unwrap();
    assert_eq!(view.reference, "A-002");
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.assignee.unwrap().display_name, "Bob");
}

#[tokio::test]
async fn descriptions_are_trimmed_keep_line_breaks_and_are_limited() {
    let w = world().await;
    let action = w.new_action("  line one\nline two \n", None).await;
    assert_eq!(action.description().as_str(), "line one\nline two");

    let too_long = "x".repeat(4001);
    for bad in ["   ", too_long.as_str()] {
        let err = create_action(
            &w.store,
            w.ada.id(),
            NewAction {
                project_id: w.project.id(),
                description: bad,
                assignee: None,
                due_date: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ActionError::InvalidDescription(_)), "{err:?}");
    }
}

#[tokio::test]
async fn the_assignee_must_be_a_member() {
    let w = world().await;
    let carol = create_person(&w.store, None, "Carol").await.unwrap();
    let err = create_action(
        &w.store,
        w.ada.id(),
        NewAction {
            project_id: w.project.id(),
            description: "Do it",
            assignee: Some(carol.id()),
            due_date: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, ActionError::AssigneeNotAMember);

    let action = w.new_action("Do it", None).await;
    let err = update_action(
        &w.store,
        w.ada.id(),
        action.id(),
        1,
        ActionEdit {
            description: "Do it",
            assignee: Some(carol.id()),
            due_date: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, ActionError::AssigneeNotAMember);
}

#[tokio::test]
async fn an_action_can_be_edited_with_a_past_due_date() {
    let w = world().await;
    let action = w.new_action("Draft plan", None).await;
    let after = update_action(
        &w.store,
        w.ada.id(),
        action.id(),
        1,
        ActionEdit {
            description: "Draft the plan",
            assignee: Some(w.bob.id()),
            due_date: Some(date("2001-02-03")),
        },
    )
    .await
    .unwrap();
    assert_eq!(after.version(), 2);
    assert_eq!(after.description().as_str(), "Draft the plan");
    assert_eq!(after.assignee(), Some(w.bob.id()));
    assert_eq!(after.due_date(), Some(date("2001-02-03")));
    assert_eq!(after.reference_label(), "A-001");
}

#[tokio::test]
async fn editing_from_a_stale_version_is_a_conflict() {
    let w = world().await;
    let action = w.new_action("Draft plan", None).await;
    set_action_closed(&w.store, w.ada.id(), action.id(), 1, true)
        .await
        .unwrap();
    let err = update_action(
        &w.store,
        w.ada.id(),
        action.id(),
        1,
        ActionEdit {
            description: "x",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, ActionError::Repo(RepoError::Conflict));
}

#[tokio::test]
async fn an_action_can_be_closed_and_reopened() {
    let w = world().await;
    let action = w.new_action("Draft plan", None).await;
    let closed = set_action_closed(&w.store, w.ada.id(), action.id(), 1, true)
        .await
        .unwrap();
    assert!(closed.closed());
    assert_eq!(closed.version(), 2);
    let again = set_action_closed(&w.store, w.ada.id(), action.id(), 2, true)
        .await
        .unwrap();
    assert_eq!(
        again.version(),
        2,
        "closing a closed action changes nothing"
    );
    let reopened = set_action_closed(&w.store, w.ada.id(), action.id(), 2, false)
        .await
        .unwrap();
    assert!(!reopened.closed());
    assert_eq!(reopened.version(), 3);
}

#[tokio::test]
async fn each_change_is_recorded_in_change_history() {
    let w = world().await;
    let action = w.new_action("Draft plan", None).await;
    update_action(
        &w.store,
        w.bob.id(),
        action.id(),
        1,
        ActionEdit {
            description: "Draft the plan",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap();
    set_action_closed(&w.store, w.ada.id(), action.id(), 2, true)
        .await
        .unwrap();
    set_action_closed(&w.store, w.ada.id(), action.id(), 3, false)
        .await
        .unwrap();

    let history = w.history(action.id()).await;
    let kinds: Vec<_> = history.iter().map(|e| (e.version, e.change_kind)).collect();
    assert_eq!(
        kinds,
        [
            (1, ChangeKind::Created),
            (2, ChangeKind::Updated),
            (3, ChangeKind::Updated),
            (4, ChangeKind::Updated),
        ]
    );
    assert_eq!(history[1].acting_person, w.bob.id());
    assert_eq!(history[0].project_id, Some(w.project.id().as_uuid()));
}

#[tokio::test]
async fn actions_in_an_archived_project_cannot_be_changed() {
    let w = world().await;
    let action = w.new_action("Draft plan", None).await;
    set_archived(&w.store, w.ada.id(), w.project.id(), 2, true)
        .await
        .unwrap();
    let err = create_action(
        &w.store,
        w.ada.id(),
        NewAction {
            project_id: w.project.id(),
            description: "More",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, ActionError::Archived);
    let err = set_action_closed(&w.store, w.ada.id(), action.id(), 1, true)
        .await
        .unwrap_err();
    assert_eq!(err, ActionError::Archived);
}

#[tokio::test]
async fn creating_an_action_in_an_unknown_project_is_rejected() {
    let w = world().await;
    let err = create_action(
        &w.store,
        w.ada.id(),
        NewAction {
            project_id: lodestar_core::project::ProjectId::new(),
            description: "Lost",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, ActionError::UnknownProject);
}

#[tokio::test]
async fn removing_a_member_unassigns_their_open_actions() {
    let w = world().await;
    let open = w.new_action("Open", Some(&w.bob)).await;
    let done = w.new_action("Done", Some(&w.bob)).await;
    let adas = w.new_action("Ada's", Some(&w.ada)).await;
    set_action_closed(&w.store, w.ada.id(), done.id(), 1, true)
        .await
        .unwrap();

    remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
        .await
        .unwrap();

    let open_after = get_action(&w.store, open.id()).await.unwrap();
    assert_eq!(open_after.assignee, None);
    assert_eq!(open_after.version, 2);
    let done_after = get_action(&w.store, done.id()).await.unwrap();
    assert_eq!(
        done_after.assignee.map(|a| a.id),
        Some(w.bob.id()),
        "closed items keep their assignee (FR-22)"
    );
    let adas_after = get_action(&w.store, adas.id()).await.unwrap();
    assert_eq!(adas_after.assignee.map(|a| a.id), Some(w.ada.id()));

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
    assert_eq!(unassigned.acting_person, w.ada.id());
}

#[tokio::test]
async fn reopening_an_action_of_a_removed_member_leaves_it_unassigned() {
    let w = world().await;
    let action = w.new_action("Done", Some(&w.bob)).await;
    set_action_closed(&w.store, w.ada.id(), action.id(), 1, true)
        .await
        .unwrap();
    remove_member(&w.store, w.ada.id(), w.project.id(), 2, w.bob.id())
        .await
        .unwrap();
    let reopened = set_action_closed(&w.store, w.ada.id(), action.id(), 2, false)
        .await
        .unwrap();
    assert_eq!(reopened.assignee(), None);
}

#[tokio::test]
async fn open_actions_are_listed_by_reference_and_closed_ones_on_request() {
    let w = world().await;
    let a1 = w.new_action("One", None).await;
    w.new_action("Two", None).await;
    set_action_closed(&w.store, w.ada.id(), a1.id(), 1, true)
        .await
        .unwrap();
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
    create_action(
        &w.store,
        w.ada.id(),
        NewAction {
            project_id: other.id(),
            description: "Elsewhere",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap();

    let labels = |page: lodestar_core::ports::Page<lodestar_core::action::ActionView>| {
        page.items
            .into_iter()
            .map(|a| format!("{}/{}", a.project_code, a.reference))
            .collect::<Vec<_>>()
    };
    let open = list_actions(
        &w.store,
        ActionFilter {
            project_id: Some(w.project.id()),
            include_closed: false,
        },
        PageRequest::first(10),
    )
    .await
    .unwrap();
    assert_eq!(labels(open), ["WEBR/A-002"]);
    let all = list_actions(
        &w.store,
        ActionFilter {
            project_id: Some(w.project.id()),
            include_closed: true,
        },
        PageRequest::first(10),
    )
    .await
    .unwrap();
    assert_eq!(labels(all), ["WEBR/A-001", "WEBR/A-002"]);
    let everywhere = list_actions(
        &w.store,
        ActionFilter {
            project_id: None,
            include_closed: false,
        },
        PageRequest::first(10),
    )
    .await
    .unwrap();
    assert_eq!(everywhere.total, 2);
}
