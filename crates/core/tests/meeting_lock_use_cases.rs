use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use lodestar_core::meeting_lock::use_cases::{
    LockError, acquire_lock, cancel_lock, heartbeat_lock, lock_status,
};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::project::members::add_member;
use lodestar_core::project::use_cases::{NewProject, create_project};
use lodestar_core::project::{Project, ProjectId};
use lodestar_core::session::SessionId;
use lodestar_core::test_support::InMemoryStore;

/// A fake clock: `t(s)` is `s` seconds after a fixed instant.
fn t(seconds: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 21, 9, 0, 0).unwrap() + TimeDelta::seconds(seconds)
}

fn tab(name: &str) -> SessionId {
    SessionId::new(name)
}

/// Ada leads WEBR; Bob is a Member; Cy is not.
async fn setup() -> (InMemoryStore, Person, Person, Person, Project) {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let bob = create_person(&store, Some(ada.id()), "Bob").await.unwrap();
    let cy = create_person(&store, Some(ada.id()), "Cy").await.unwrap();
    let project = create_project(
        &store,
        ada.id(),
        NewProject {
            name: "Website",
            description: None,
            project_code: "WEBR",
        },
    )
    .await
    .unwrap();
    let project = add_member(&store, ada.id(), project.id(), project.version(), bob.id())
        .await
        .unwrap();
    (store, ada, bob, cy, project)
}

#[tokio::test]
async fn the_leader_acquires_a_lock_that_members_can_see() {
    let (store, ada, bob, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();

    let seen = lock_status(&store, bob.id(), &tab("b"), project.id(), t(10))
        .await
        .unwrap();
    assert_eq!(seen.leader.id, ada.id());
    assert_eq!(seen.leader.display_name, "Ada");
    assert_eq!(seen.acquired_at, t(0));
    assert!(seen.live);
    assert!(!seen.held_by_you);

    let own = lock_status(&store, ada.id(), &tab("a"), project.id(), t(10))
        .await
        .unwrap();
    assert!(own.held_by_you);
}

#[tokio::test]
async fn only_the_project_leader_may_acquire() {
    let (store, _, bob, _, project) = setup().await;
    assert_eq!(
        acquire_lock(&store, bob.id(), &tab("b"), project.id(), t(0)).await,
        Err(LockError::NotTheLeader)
    );
}

#[tokio::test]
async fn an_unknown_project_has_no_lock_to_take() {
    let (store, ada, ..) = setup().await;
    assert_eq!(
        acquire_lock(&store, ada.id(), &tab("a"), ProjectId::new(), t(0)).await,
        Err(LockError::UnknownProject)
    );
}

#[tokio::test]
async fn a_lock_is_live_until_exactly_two_minutes_after_its_last_heartbeat() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    let a = tab("a");
    let at = |s| lock_status(&store, ada.id(), &a, project.id(), t(s));
    assert!(at(119).await.unwrap().live);
    assert!(!at(120).await.unwrap().live);

    heartbeat_lock(&store, &tab("a"), project.id(), t(100))
        .await
        .unwrap();
    assert!(at(219).await.unwrap().live);
    assert!(!at(220).await.unwrap().live);
}

#[tokio::test]
async fn a_live_lock_held_by_another_session_is_not_broken() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    assert_eq!(
        acquire_lock(&store, ada.id(), &tab("other"), project.id(), t(119)).await,
        Err(LockError::Held)
    );
}

#[tokio::test]
async fn a_stale_lock_is_taken_over_by_another_session() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    let taken = acquire_lock(&store, ada.id(), &tab("other"), project.id(), t(120))
        .await
        .unwrap();
    assert_eq!(taken.acquired_at(), t(120));

    assert_eq!(
        heartbeat_lock(&store, &tab("a"), project.id(), t(121)).await,
        Err(LockError::NotHeld)
    );
    assert!(
        !lock_status(&store, ada.id(), &tab("a"), project.id(), t(121))
            .await
            .unwrap()
            .held_by_you
    );
}

#[tokio::test]
async fn a_session_reclaims_its_own_stale_lock_keeping_when_it_was_acquired() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    let reclaimed = acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(500))
        .await
        .unwrap();
    assert_eq!(reclaimed.acquired_at(), t(0));
    assert!(reclaimed.is_live(t(600)));
}

#[tokio::test]
async fn a_session_may_heartbeat_its_own_stale_lock() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    let lock = heartbeat_lock(&store, &tab("a"), project.id(), t(500))
        .await
        .unwrap();
    assert!(lock.is_live(t(500)));
}

#[tokio::test]
async fn only_the_holding_session_may_heartbeat_or_cancel() {
    let (store, ada, _, _, project) = setup().await;
    assert_eq!(
        heartbeat_lock(&store, &tab("a"), project.id(), t(0)).await,
        Err(LockError::NotHeld)
    );
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    assert_eq!(
        heartbeat_lock(&store, &tab("b"), project.id(), t(1)).await,
        Err(LockError::NotHeld)
    );
    assert_eq!(
        cancel_lock(&store, &tab("b"), project.id()).await,
        Err(LockError::NotHeld)
    );
}

#[tokio::test]
async fn cancelling_removes_the_lock() {
    let (store, ada, _, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    cancel_lock(&store, &tab("a"), project.id()).await.unwrap();
    assert_eq!(
        lock_status(&store, ada.id(), &tab("a"), project.id(), t(1)).await,
        Err(LockError::NoLock)
    );
}

#[tokio::test]
async fn only_members_may_read_the_lock() {
    let (store, ada, _, cy, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), t(0))
        .await
        .unwrap();
    assert_eq!(
        lock_status(&store, cy.id(), &tab("c"), project.id(), t(1)).await,
        Err(LockError::NotAMember)
    );
}
