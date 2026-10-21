use chrono::{DateTime, TimeDelta, Utc};
use lodestar_core::comment::use_cases::{NewComment, create_comment, edit_comment};
use lodestar_core::comment::{Target, TargetType};
use lodestar_core::meeting::use_cases::{
    MeetingError, NewMeeting, create_meeting, get_meeting, list_meetings,
};
use lodestar_core::meeting_lock::use_cases::{acquire_lock, lock_status};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::ports::PageRequest;
use lodestar_core::project::members::add_member;
use lodestar_core::project::use_cases::{NewProject, create_project};
use lodestar_core::project::{Project, ProjectId};
use lodestar_core::session::SessionId;
use lodestar_core::test_support::InMemoryStore;

/// Change History records real time, so the clock here does too.
fn now() -> DateTime<Utc> {
    Utc::now()
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

fn meeting<'a>(
    project: ProjectId,
    attendees: &'a [lodestar_core::person::PersonId],
    comments: &'a [&'a str],
) -> NewMeeting<'a> {
    NewMeeting {
        project_id: project,
        attendee_ids: attendees,
        comments,
    }
}

#[tokio::test]
async fn ending_a_meeting_records_it_and_releases_the_lock() {
    let (store, ada, bob, _, project) = setup().await;
    let lock = acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
        .await
        .unwrap();
    create_comment(
        &store,
        bob.id(),
        NewComment {
            project_id: project.id(),
            target: Target {
                kind: TargetType::Project,
                id: project.id().as_uuid(),
            },
            body: "Launch moved",
        },
    )
    .await
    .unwrap();

    let ended = now();
    let created = create_meeting(
        &store,
        ada.id(),
        &tab("a"),
        meeting(project.id(), &[bob.id()], &["Good progress"]),
        ended,
    )
    .await
    .unwrap();

    assert_eq!(created.leader(), ada.id());
    assert_eq!(created.attendees(), [ada.id(), bob.id()]);
    assert_eq!(created.started_at(), lock.acquired_at());
    assert!(created.ended_at() >= created.started_at());
    let minutes = created.minutes();
    assert_eq!(minutes.leader, "Ada");
    assert_eq!(minutes.attendees, ["Ada", "Bob"]);
    assert_eq!(minutes.project_name, "Website");
    assert_eq!(minutes.meeting_comments, ["Launch moved", "Good progress"]);
    assert!(
        lock_status(&store, ada.id(), &tab("a"), project.id(), now())
            .await
            .is_err(),
        "the lock is released"
    );

    let listed = list_meetings(&store, bob.id(), project.id(), PageRequest::first(10))
        .await
        .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].id, created.id());
    assert_eq!(listed.items[0].attendees[1].display_name, "Bob");
    let read = get_meeting(&store, bob.id(), created.id()).await.unwrap();
    assert_eq!(&read.minutes, minutes);
}

#[tokio::test]
async fn meeting_comments_are_ordinary_comments_and_the_minutes_stay_fixed() {
    let (store, ada, bob, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
        .await
        .unwrap();
    let created = create_meeting(
        &store,
        ada.id(),
        &tab("a"),
        meeting(project.id(), &[bob.id()], &["Agreed scope"]),
        now(),
    )
    .await
    .unwrap();
    let target = Target {
        kind: TargetType::Meeting,
        id: created.id().as_uuid(),
    };
    let comments = lodestar_core::comment::use_cases::list_comments(
        &store,
        lodestar_core::comment::use_cases::CommentFilter {
            project_id: None,
            target: Some(target),
        },
        PageRequest::first(10),
    )
    .await
    .unwrap();
    assert_eq!(comments.total, 1);
    let comment = &comments.items[0];
    assert_eq!(comment.author.id, ada.id());

    edit_comment(&store, ada.id(), comment.id, comment.version, "Scope cut")
        .await
        .unwrap();
    create_comment(
        &store,
        bob.id(),
        NewComment {
            project_id: project.id(),
            target,
            body: "Added later",
        },
    )
    .await
    .unwrap();

    let read = get_meeting(&store, bob.id(), created.id()).await.unwrap();
    assert_eq!(read.minutes.meeting_comments, ["Agreed scope"]);
}

#[tokio::test]
async fn only_the_session_holding_the_lock_may_end_the_meeting() {
    let (store, ada, bob, _, project) = setup().await;
    let attendees = [bob.id()];
    let input = meeting(project.id(), &attendees, &[]);
    assert_eq!(
        create_meeting(&store, ada.id(), &tab("a"), input, now())
            .await
            .unwrap_err(),
        MeetingError::NotHeld
    );
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
        .await
        .unwrap();
    assert_eq!(
        create_meeting(&store, ada.id(), &tab("b"), input, now())
            .await
            .unwrap_err(),
        MeetingError::NotHeld
    );
}

#[tokio::test]
async fn a_session_may_end_a_meeting_on_its_own_stale_lock() {
    let (store, ada, bob, _, project) = setup().await;
    let start = now() - TimeDelta::minutes(30);
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), start)
        .await
        .unwrap();
    let created = create_meeting(
        &store,
        ada.id(),
        &tab("a"),
        meeting(project.id(), &[bob.id()], &[]),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(created.started_at(), lock_start(start));
}

fn lock_start(t: DateTime<Utc>) -> DateTime<Utc> {
    use chrono::SubsecRound;
    t.trunc_subsecs(6)
}

#[tokio::test]
async fn attendees_must_be_given_and_be_members() {
    let (store, ada, bob, cy, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
        .await
        .unwrap();
    assert_eq!(
        create_meeting(
            &store,
            ada.id(),
            &tab("a"),
            meeting(project.id(), &[], &[]),
            now()
        )
        .await
        .unwrap_err(),
        MeetingError::NoAttendees
    );
    assert_eq!(
        create_meeting(
            &store,
            ada.id(),
            &tab("a"),
            meeting(project.id(), &[bob.id(), cy.id()], &[]),
            now()
        )
        .await
        .unwrap_err(),
        MeetingError::AttendeeNotAMember
    );
    assert!(
        lock_status(&store, ada.id(), &tab("a"), project.id(), now())
            .await
            .is_ok(),
        "a failed create keeps the lock"
    );
    let listed = list_meetings(&store, ada.id(), project.id(), PageRequest::first(10))
        .await
        .unwrap();
    assert_eq!(listed.total, 0);
}

#[tokio::test]
async fn naming_the_leader_as_an_attendee_lists_them_once() {
    let (store, ada, bob, _, project) = setup().await;
    acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
        .await
        .unwrap();
    let created = create_meeting(
        &store,
        ada.id(),
        &tab("a"),
        meeting(project.id(), &[bob.id(), ada.id()], &[]),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(created.attendees(), [ada.id(), bob.id()]);
}

#[tokio::test]
async fn meetings_are_listed_newest_first_and_only_to_members() {
    let (store, ada, bob, cy, project) = setup().await;
    let mut ids = Vec::new();
    for _ in 0..2 {
        acquire_lock(&store, ada.id(), &tab("a"), project.id(), now())
            .await
            .unwrap();
        let m = create_meeting(
            &store,
            ada.id(),
            &tab("a"),
            meeting(project.id(), &[bob.id()], &[]),
            now(),
        )
        .await
        .unwrap();
        ids.push(m.id());
    }
    let listed = list_meetings(&store, bob.id(), project.id(), PageRequest::first(10))
        .await
        .unwrap();
    let listed: Vec<_> = listed.items.iter().map(|m| m.id).collect();
    assert_eq!(listed, [ids[1], ids[0]]);

    assert_eq!(
        list_meetings(&store, cy.id(), project.id(), PageRequest::first(10))
            .await
            .unwrap_err(),
        MeetingError::NotAMember
    );
    assert_eq!(
        get_meeting(&store, cy.id(), ids[0]).await.unwrap_err(),
        MeetingError::NotAMember
    );
}
