use chrono::{DateTime, SubsecRound, TimeDelta, Utc};
use lodestar_core::action::use_cases::{ActionError, NewAction, create_action};
use lodestar_core::meeting_lock::use_cases::acquire_lock;
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::project::Project;
use lodestar_core::project::use_cases::{NewProject, ProjectError, create_project, set_archived};
use lodestar_core::session::{Actor, SessionId};
use lodestar_core::test_support::InMemoryStore;

/// Ada leads WEBR and holds its Meeting Lock from her "tab" session at t0.
struct World {
    store: InMemoryStore,
    ada: Person,
    project: Project,
    t0: DateTime<Utc>,
}

async fn world() -> World {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
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
    let t0 = Utc::now();
    acquire_lock(&store, ada.id(), &SessionId::new("tab"), project.id(), t0)
        .await
        .unwrap();
    World {
        store,
        ada,
        project,
        t0,
    }
}

impl World {
    fn actor(&self, session: &str, after: TimeDelta) -> Actor {
        Actor {
            person: self.ada.id(),
            session: Some(SessionId::new(session)),
            now: self.t0 + after,
        }
    }

    async fn create(&self, actor: Actor) -> Result<(), ActionError> {
        create_action(
            &self.store,
            actor,
            NewAction {
                project_id: self.project.id(),
                description: "Book the room",
                assignee: None,
                due_date: None,
            },
        )
        .await
        .map(|_| ())
    }
}

#[tokio::test]
async fn a_live_lock_refuses_other_sessions_and_names_the_leader() {
    let w = world().await;
    let Err(ActionError::Locked(meeting)) = w.create(w.actor("other", TimeDelta::zero())).await
    else {
        panic!("expected Locked");
    };
    assert_eq!(meeting.leader.display_name, "Ada");
    assert_eq!(meeting.since, w.t0.trunc_subsecs(6));
}

#[tokio::test]
async fn the_holding_session_writes_normally() {
    let w = world().await;
    w.create(w.actor("tab", TimeDelta::zero())).await.unwrap();
}

#[tokio::test]
async fn a_stale_lock_refuses_nothing() {
    let w = world().await;
    w.create(w.actor("other", TimeDelta::minutes(3)))
        .await
        .unwrap();
}

#[tokio::test]
async fn archiving_is_refused_even_for_the_holder() {
    let w = world().await;
    let result = set_archived(
        &w.store,
        w.actor("tab", TimeDelta::zero()),
        w.project.id(),
        1,
        true,
    )
    .await;
    assert!(matches!(result, Err(ProjectError::Locked(_))));
}
