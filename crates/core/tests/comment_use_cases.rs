use lodestar_core::action::use_cases::{NewAction, create_action};
use lodestar_core::blocker::use_cases::{NewBlocker, create_blocker};
use lodestar_core::comment::use_cases::{
    CommentError, CommentFilter, NewComment, create_comment, delete_comment, edit_comment,
    get_comment, list_comments,
};
use lodestar_core::comment::{BodyError, MAX_BODY_CHARS, Target, TargetType};
use lodestar_core::history::{ChangeKind, EntityType};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::ports::{HistoryQuery, PageRequest, RepoError};
use lodestar_core::project::members::{add_member, remove_member};
use lodestar_core::project::use_cases::{NewProject, create_project, set_archived};
use lodestar_core::project::{Project, ProjectId};
use lodestar_core::risk::use_cases::{NewRisk, create_risk, get_risk, set_closed};
use lodestar_core::test_support::InMemoryStore;
use uuid::Uuid;

/// Ada's Project WEBR, with Bob as a second Member.
async fn setup() -> (InMemoryStore, Person, Person, Project) {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let bob = create_person(&store, Some(ada.id()), "Bob").await.unwrap();
    let project = create_project(&store, ada.id(), web()).await.unwrap();
    let project = add_member(&store, ada.id(), project.id(), project.version(), bob.id())
        .await
        .unwrap();
    (store, ada, bob, project)
}

fn web() -> NewProject<'static> {
    NewProject {
        name: "Website",
        description: None,
        project_code: "WEBR",
    }
}

fn on_project(project: &Project) -> Target {
    Target {
        kind: TargetType::Project,
        id: project.id().as_uuid(),
    }
}

fn new<'a>(project: ProjectId, target: Target, body: &'a str) -> NewComment<'a> {
    NewComment {
        project_id: project,
        target,
        body,
    }
}

async fn a_risk(store: &InMemoryStore, ada: &Person, project: &Project) -> Target {
    let risk = create_risk(
        store,
        ada.id(),
        NewRisk {
            project_id: project.id(),
            description: "Vendor late",
            likelihood: 3,
            impact: 3,
        },
    )
    .await
    .unwrap();
    Target {
        kind: TargetType::Risk,
        id: risk.id().as_uuid(),
    }
}

#[tokio::test]
async fn members_can_comment_on_each_kind_of_target() {
    let (store, ada, _, project) = setup().await;
    let action = create_action(
        &store,
        ada.id(),
        NewAction {
            project_id: project.id(),
            description: "Ship",
            assignee: None,
            due_date: None,
        },
    )
    .await
    .unwrap();
    let blocker = create_blocker(
        &store,
        ada.id(),
        NewBlocker {
            project_id: project.id(),
            description: "Waiting",
            accountable: None,
        },
    )
    .await
    .unwrap();
    let risk = a_risk(&store, &ada, &project).await;
    let targets = [
        on_project(&project),
        Target {
            kind: TargetType::Action,
            id: action.id().as_uuid(),
        },
        risk,
        Target {
            kind: TargetType::Blocker,
            id: blocker.id().as_uuid(),
        },
    ];
    for target in targets {
        let comment = create_comment(&store, ada.id(), new(project.id(), target, "Noted"))
            .await
            .unwrap();
        let view = get_comment(&store, comment.id()).await.unwrap();
        assert_eq!(view.target, target);
        assert_eq!(view.author.display_name, "Ada");
        assert_eq!(view.body, "Noted");
        assert_eq!(
            (view.edited_at, view.deleted, view.version),
            (None, false, 1)
        );
    }
    let listed = list_comments(
        &store,
        CommentFilter {
            project_id: Some(project.id()),
            target: None,
        },
        PageRequest::first(10),
    )
    .await
    .unwrap();
    assert_eq!(listed.total, 4);
}

#[tokio::test]
async fn the_body_is_kept_exactly_but_cannot_be_blank_or_too_long() {
    let (store, ada, _, project) = setup().await;
    let text = "  **bold** <b>x</b>\nline two  ";
    let comment = create_comment(
        &store,
        ada.id(),
        new(project.id(), on_project(&project), text),
    )
    .await
    .unwrap();
    assert_eq!(comment.body().as_str(), text);

    let longest = "é".repeat(MAX_BODY_CHARS);
    create_comment(
        &store,
        ada.id(),
        new(project.id(), on_project(&project), &longest),
    )
    .await
    .unwrap();
    let too_long = "é".repeat(MAX_BODY_CHARS + 1);
    for (body, expected) in [
        ("", BodyError::Empty),
        (" \n\t", BodyError::Empty),
        (too_long.as_str(), BodyError::TooLong),
    ] {
        assert_eq!(
            create_comment(
                &store,
                ada.id(),
                new(project.id(), on_project(&project), body)
            )
            .await
            .unwrap_err(),
            CommentError::InvalidBody(expected)
        );
    }
}

#[tokio::test]
async fn the_target_must_exist_in_the_same_project() {
    let (store, ada, _, project) = setup().await;
    let other = create_project(
        &store,
        ada.id(),
        NewProject {
            name: "Other",
            description: None,
            project_code: "OTHR",
        },
    )
    .await
    .unwrap();
    let elsewhere = a_risk(&store, &ada, &other).await;
    for target in [
        elsewhere,
        on_project(&other),
        Target {
            kind: TargetType::Action,
            id: Uuid::now_v7(),
        },
    ] {
        assert_eq!(
            create_comment(&store, ada.id(), new(project.id(), target, "Hi"))
                .await
                .unwrap_err(),
            CommentError::UnknownTarget,
            "{target:?}"
        );
    }
    assert_eq!(
        create_comment(
            &store,
            ada.id(),
            new(ProjectId::new(), on_project(&project), "Hi")
        )
        .await
        .unwrap_err(),
        CommentError::UnknownProject
    );
}

#[tokio::test]
async fn only_members_can_comment() {
    let (store, ada, _, project) = setup().await;
    let carol = create_person(&store, Some(ada.id()), "Carol")
        .await
        .unwrap();
    assert_eq!(
        create_comment(
            &store,
            carol.id(),
            new(project.id(), on_project(&project), "Hi")
        )
        .await
        .unwrap_err(),
        CommentError::NotAMember
    );
}

#[tokio::test]
async fn closed_items_can_be_commented_on() {
    let (store, ada, _, project) = setup().await;
    let risk = a_risk(&store, &ada, &project).await;
    let id = lodestar_core::risk::RiskId::from_uuid(risk.id);
    set_closed(&store, ada.id(), id, 1, true).await.unwrap();
    create_comment(&store, ada.id(), new(project.id(), risk, "Closed it"))
        .await
        .unwrap();
}

#[tokio::test]
async fn editing_changes_the_body_and_marks_it_edited() {
    let (store, ada, _, project) = setup().await;
    let comment = create_comment(
        &store,
        ada.id(),
        new(project.id(), on_project(&project), "Hi"),
    )
    .await
    .unwrap();
    let edited = edit_comment(&store, ada.id(), comment.id(), 1, "Hello")
        .await
        .unwrap();
    assert_eq!(edited.body().as_str(), "Hello");
    assert_eq!(edited.created_at(), comment.created_at());
    assert!(
        edited
            .edited_at()
            .is_some_and(|e| e >= comment.created_at())
    );
    assert_eq!(edited.version(), 2);

    assert_eq!(
        edit_comment(&store, ada.id(), comment.id(), 1, "Stale")
            .await
            .unwrap_err(),
        CommentError::Repo(RepoError::Conflict)
    );
    assert_eq!(
        edit_comment(&store, ada.id(), comment.id(), 2, " ")
            .await
            .unwrap_err(),
        CommentError::InvalidBody(BodyError::Empty)
    );
}

#[tokio::test]
async fn only_the_author_can_edit_or_delete_and_only_while_a_member() {
    let (store, ada, bob, project) = setup().await;
    let comment = create_comment(
        &store,
        bob.id(),
        new(project.id(), on_project(&project), "Hi"),
    )
    .await
    .unwrap();
    assert_eq!(
        edit_comment(&store, ada.id(), comment.id(), 1, "Mine now")
            .await
            .unwrap_err(),
        CommentError::NotTheAuthor
    );
    assert_eq!(
        delete_comment(&store, ada.id(), comment.id(), 1)
            .await
            .unwrap_err(),
        CommentError::NotTheAuthor
    );

    remove_member(&store, ada.id(), project.id(), project.version(), bob.id())
        .await
        .unwrap();
    assert_eq!(
        edit_comment(&store, bob.id(), comment.id(), 1, "Still mine")
            .await
            .unwrap_err(),
        CommentError::NotAMember
    );
    assert_eq!(
        delete_comment(&store, bob.id(), comment.id(), 1)
            .await
            .unwrap_err(),
        CommentError::NotAMember
    );
}

#[tokio::test]
async fn a_deleted_comment_keeps_its_text_but_cannot_be_edited() {
    let (store, ada, _, project) = setup().await;
    let comment = create_comment(
        &store,
        ada.id(),
        new(project.id(), on_project(&project), "Hi"),
    )
    .await
    .unwrap();
    let deleted = delete_comment(&store, ada.id(), comment.id(), 1)
        .await
        .unwrap();
    assert!(deleted.is_deleted());
    assert_eq!(deleted.version(), 2);
    let view = get_comment(&store, comment.id()).await.unwrap();
    assert!(view.deleted);
    assert_eq!(view.body, "Hi");

    assert_eq!(
        edit_comment(&store, ada.id(), comment.id(), 2, "Back")
            .await
            .unwrap_err(),
        CommentError::Deleted
    );
    let again = delete_comment(&store, ada.id(), comment.id(), 2)
        .await
        .unwrap();
    assert_eq!(again.version(), 2);
}

#[tokio::test]
async fn nothing_can_be_changed_on_an_archived_project() {
    let (store, ada, _, project) = setup().await;
    let comment = create_comment(
        &store,
        ada.id(),
        new(project.id(), on_project(&project), "Hi"),
    )
    .await
    .unwrap();
    set_archived(&store, ada.id(), project.id(), project.version(), true)
        .await
        .unwrap();
    assert_eq!(
        create_comment(
            &store,
            ada.id(),
            new(project.id(), on_project(&project), "Hi")
        )
        .await
        .unwrap_err(),
        CommentError::Archived
    );
    assert_eq!(
        edit_comment(&store, ada.id(), comment.id(), 1, "Hello")
            .await
            .unwrap_err(),
        CommentError::Archived
    );
    assert_eq!(
        delete_comment(&store, ada.id(), comment.id(), 1)
            .await
            .unwrap_err(),
        CommentError::Archived
    );
}

#[tokio::test]
async fn changes_are_recorded_for_the_comment_and_not_its_target() {
    let (store, ada, _, project) = setup().await;
    let risk = a_risk(&store, &ada, &project).await;
    let comment = create_comment(&store, ada.id(), new(project.id(), risk, "Hi"))
        .await
        .unwrap();
    edit_comment(&store, ada.id(), comment.id(), 1, "Hello")
        .await
        .unwrap();
    delete_comment(&store, ada.id(), comment.id(), 2)
        .await
        .unwrap();

    let history = store
        .entity_history(
            EntityType::Comment,
            comment.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let kinds: Vec<_> = history
        .items
        .iter()
        .map(|e| (e.change_kind, e.acting_person))
        .collect();
    assert_eq!(
        kinds,
        [
            (ChangeKind::Created, ada.id()),
            (ChangeKind::Updated, ada.id()),
            (ChangeKind::Updated, ada.id()),
        ]
    );

    let risk_history = store
        .entity_history(EntityType::Risk, risk.id, PageRequest::first(10))
        .await
        .unwrap();
    assert_eq!(risk_history.total, 1);
    let id = lodestar_core::risk::RiskId::from_uuid(risk.id);
    assert_eq!(get_risk(&store, id).await.unwrap().version, 1);
}
