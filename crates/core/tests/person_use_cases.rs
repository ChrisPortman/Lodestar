use lodestar_core::history::{ChangeKind, EntityType};
use lodestar_core::person::DisplayNameError;
use lodestar_core::person::use_cases::{
    PersonError, create_person, get_person, list_people, rename_person,
};
use lodestar_core::ports::{HistoryQuery, PageRequest, PeopleSort, RepoError};
use lodestar_core::test_support::InMemoryStore;

#[tokio::test]
async fn creating_a_person_records_who_created_them() {
    let store = InMemoryStore::new();
    let actor = create_person(&store, None, "Ada").await.unwrap();
    let bea = create_person(&store, Some(actor.id()), " Bea ")
        .await
        .unwrap();

    assert_eq!(
        get_person(&store, bea.id()).await.unwrap().display_name,
        "Bea"
    );
    let history = store
        .entity_history(
            EntityType::Person,
            bea.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    assert_eq!(history.items[0].acting_person, actor.id());
    assert_eq!(history.items[0].change_kind, ChangeKind::Created);
}

#[tokio::test]
async fn a_person_created_with_no_acting_person_created_themselves() {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let history = store
        .entity_history(
            EntityType::Person,
            ada.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    assert_eq!(history.items[0].acting_person, ada.id());
}

#[tokio::test]
async fn an_invalid_name_creates_nobody() {
    let store = InMemoryStore::new();
    let err = create_person(&store, None, "  ").await.unwrap_err();
    assert_eq!(
        err,
        PersonError::InvalidDisplayName(DisplayNameError::Empty)
    );
    assert_eq!(
        list_people(&store, PageRequest::first(10), PeopleSort::DisplayName)
            .await
            .unwrap()
            .total,
        0
    );
}

#[tokio::test]
async fn renaming_a_person_saves_the_new_name() {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let renamed = rename_person(&store, ada.id(), ada.id(), 1, "Ada King")
        .await
        .unwrap();

    assert_eq!(renamed.version(), 2);
    let view = get_person(&store, ada.id()).await.unwrap();
    assert_eq!(view.display_name, "Ada King");
    assert_eq!(view.version, 2);
}

#[tokio::test]
async fn renaming_from_an_out_of_date_version_conflicts() {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    rename_person(&store, ada.id(), ada.id(), 1, "Ada King")
        .await
        .unwrap();
    assert_eq!(
        rename_person(&store, ada.id(), ada.id(), 1, "Ada L")
            .await
            .unwrap_err(),
        PersonError::Repo(RepoError::Conflict)
    );
    assert_eq!(
        get_person(&store, ada.id()).await.unwrap().display_name,
        "Ada King"
    );
}

#[tokio::test]
async fn renaming_an_unknown_person_is_not_found() {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
    let missing = lodestar_core::person::PersonId::new();
    assert_eq!(
        rename_person(&store, ada.id(), missing, 1, "X")
            .await
            .unwrap_err(),
        PersonError::Repo(RepoError::NotFound)
    );
}

#[tokio::test]
async fn people_are_listed_by_name() {
    let store = InMemoryStore::new();
    for name in ["Bea", "Ada"] {
        create_person(&store, None, name).await.unwrap();
    }
    let page = list_people(&store, PageRequest::first(10), PeopleSort::DisplayName)
        .await
        .unwrap();
    let names: Vec<_> = page.items.iter().map(|p| p.display_name.as_str()).collect();
    assert_eq!(names, ["Ada", "Bea"]);
}
