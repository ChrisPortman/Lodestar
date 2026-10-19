use lodestar_core::history::{ChangeKind, EntityType};
use lodestar_core::person::{Person, PersonId, use_cases::create_person};
use lodestar_core::ports::{HistoryQuery, PageRequest, RepoError, UnitOfWork};
use lodestar_core::project::guard::{GuardError, writable_project};
use lodestar_core::project::use_cases::{
    NewProject, ProjectError, ProjectSort, create_project, get_project, list_projects,
    set_archived, suggest_project_code, update_project,
};
use lodestar_core::project::{DescriptionError, ProjectCodeError, ProjectNameError};
use lodestar_core::test_support::InMemoryStore;

async fn setup() -> (InMemoryStore, Person) {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    (store, ada)
}

fn new<'a>(name: &'a str, code: &'a str) -> NewProject<'a> {
    NewProject {
        name,
        description: None,
        project_code: code,
    }
}

#[tokio::test]
async fn the_creator_becomes_member_and_project_leader() {
    let (store, ada) = setup().await;
    let project = create_project(
        &store,
        ada.id(),
        NewProject {
            name: " Website Rebuild ",
            description: Some("  New site  "),
            project_code: "WEBR",
        },
    )
    .await
    .unwrap();

    assert_eq!(
        project.members().iter().copied().collect::<Vec<_>>(),
        [ada.id()]
    );
    let view = get_project(&store, project.id()).await.unwrap();
    assert_eq!(view.name, "Website Rebuild");
    assert_eq!(view.description.as_deref(), Some("New site"));
    assert_eq!(view.project_code, "WEBR");
    assert!(!view.archived);
    assert_eq!(view.project_leader.unwrap().id, ada.id());
    assert_eq!(view.version, 1);
}

#[tokio::test]
async fn invalid_input_creates_nothing() {
    let (store, ada) = setup().await;
    let long_name = "x".repeat(101);
    let long_description = "x".repeat(4001);
    let cases = [
        (
            new("  ", "WEBR"),
            ProjectError::InvalidName(ProjectNameError::Empty),
        ),
        (
            new(&long_name, "WEBR"),
            ProjectError::InvalidName(ProjectNameError::TooLong),
        ),
        (
            new("Web", "webr"),
            ProjectError::InvalidCode(ProjectCodeError::Invalid),
        ),
        (
            NewProject {
                description: Some(" "),
                ..new("Web", "WEBR")
            },
            ProjectError::InvalidDescription(DescriptionError::Empty),
        ),
        (
            NewProject {
                description: Some(&long_description),
                ..new("Web", "WEBR")
            },
            ProjectError::InvalidDescription(DescriptionError::TooLong),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            create_project(&store, ada.id(), input).await.unwrap_err(),
            expected
        );
    }
    let page = list_projects(&store, PageRequest::first(10), true, ProjectSort::Id)
        .await
        .unwrap();
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn project_codes_are_unique() {
    let (store, ada) = setup().await;
    create_project(&store, ada.id(), new("Web", "WEBR"))
        .await
        .unwrap();
    assert_eq!(
        create_project(&store, ada.id(), new("Other", "WEBR"))
            .await
            .unwrap_err(),
        ProjectError::CodeTaken
    );
}

#[tokio::test]
async fn an_unknown_acting_person_cannot_create() {
    let (store, _) = setup().await;
    assert_eq!(
        create_project(&store, PersonId::new(), new("Web", "WEBR"))
            .await
            .unwrap_err(),
        ProjectError::UnknownActingPerson
    );
}

#[tokio::test]
async fn suggestions_skip_codes_in_use() {
    let (store, ada) = setup().await;
    assert_eq!(
        suggest_project_code(&store, "Website")
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "WEBS"
    );
    create_project(&store, ada.id(), new("Website", "WEBS"))
        .await
        .unwrap();
    assert_eq!(
        suggest_project_code(&store, "Website")
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "WEBS1"
    );
}

#[tokio::test]
async fn editing_changes_name_and_description_but_never_the_code() {
    let (store, ada) = setup().await;
    let p = create_project(
        &store,
        ada.id(),
        NewProject {
            description: Some("Old"),
            ..new("Web", "WEBR")
        },
    )
    .await
    .unwrap();
    let edited = update_project(&store, ada.id(), p.id(), 1, "Website", None)
        .await
        .unwrap();
    assert_eq!(edited.version(), 2);
    let view = get_project(&store, p.id()).await.unwrap();
    assert_eq!(view.name, "Website");
    assert_eq!(view.description, None);
    assert_eq!(view.project_code, "WEBR");

    assert_eq!(
        update_project(&store, ada.id(), p.id(), 1, "Stale", None)
            .await
            .unwrap_err(),
        ProjectError::Repo(RepoError::Conflict)
    );
}

#[tokio::test]
async fn an_archived_project_rejects_edits_until_unarchived() {
    let (store, ada) = setup().await;
    let p = create_project(&store, ada.id(), new("Web", "WEBR"))
        .await
        .unwrap();
    let archived = set_archived(&store, ada.id(), p.id(), 1, true)
        .await
        .unwrap();
    assert!(archived.archived());

    assert_eq!(
        update_project(&store, ada.id(), p.id(), 2, "Website", None)
            .await
            .unwrap_err(),
        ProjectError::Archived
    );
    let mut tx = store.begin().await.unwrap();
    assert_eq!(
        writable_project(&mut tx, p.id()).await.unwrap_err(),
        GuardError::Archived
    );
    drop(tx);

    set_archived(&store, ada.id(), p.id(), 2, false)
        .await
        .unwrap();
    update_project(&store, ada.id(), p.id(), 3, "Website", None)
        .await
        .unwrap();
}

#[tokio::test]
async fn archived_projects_are_listed_only_when_asked() {
    let (store, ada) = setup().await;
    let web = create_project(&store, ada.id(), new("Web", "WEBR"))
        .await
        .unwrap();
    create_project(&store, ada.id(), new("Alpha", "ALPH"))
        .await
        .unwrap();
    set_archived(&store, ada.id(), web.id(), 1, true)
        .await
        .unwrap();

    let names = |include_archived| {
        let store = store.clone();
        async move {
            list_projects(
                &store,
                PageRequest::first(10),
                include_archived,
                ProjectSort::Name,
            )
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|p| p.name)
            .collect::<Vec<_>>()
        }
    };
    assert_eq!(names(false).await, ["Alpha"]);
    assert_eq!(names(true).await, ["Alpha", "Web"]);
}

#[tokio::test]
async fn every_change_is_in_the_change_history() {
    let (store, ada) = setup().await;
    let p = create_project(&store, ada.id(), new("Web", "WEBR"))
        .await
        .unwrap();
    update_project(&store, ada.id(), p.id(), 1, "Website", None)
        .await
        .unwrap();
    set_archived(&store, ada.id(), p.id(), 2, true)
        .await
        .unwrap();

    let history = store
        .entity_history(
            EntityType::Project,
            p.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let kinds: Vec<_> = history
        .items
        .iter()
        .map(|e| (e.version, e.change_kind))
        .collect();
    assert_eq!(
        kinds,
        [
            (1, ChangeKind::Created),
            (2, ChangeKind::Updated),
            (3, ChangeKind::Updated)
        ]
    );
    assert!(
        history
            .items
            .iter()
            .all(|e| e.acting_person == ada.id() && e.project_id == Some(p.id().as_uuid()))
    );
}
