use lodestar_core::history::{ChangeKind, EntityType};
use lodestar_core::person::{Person, use_cases::create_person};
use lodestar_core::ports::{HistoryQuery, PageRequest, RepoError};
use lodestar_core::project::use_cases::{NewProject, create_project, set_archived};
use lodestar_core::project::{DescriptionError, Project, ProjectId};
use lodestar_core::risk::use_cases::{
    NewRisk, RiskError, RiskFilter, RiskSort, create_risk, get_risk, list_risks, set_closed,
    update_risk,
};
use lodestar_core::risk::{RatingError, RiskEdit};
use lodestar_core::test_support::InMemoryStore;

async fn setup() -> (InMemoryStore, Person, Project) {
    let store = InMemoryStore::new();
    let ada = create_person(&store, None, "Ada").await.unwrap();
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
    (store, ada, project)
}

fn new<'a>(project: &Project, description: &'a str, likelihood: u8, impact: u8) -> NewRisk<'a> {
    NewRisk {
        project_id: project.id(),
        description,
        likelihood,
        impact,
    }
}

fn filter(project: &Project, include_closed: bool) -> RiskFilter {
    RiskFilter {
        project_id: Some(project.id()),
        include_closed,
    }
}

#[tokio::test]
async fn a_new_risk_gets_the_next_reference_and_a_computed_score() {
    let (store, ada, project) = setup().await;
    let first = create_risk(&store, ada.id(), new(&project, " Vendor late ", 3, 4))
        .await
        .unwrap();
    let second = create_risk(&store, ada.id(), new(&project, "Budget cut", 2, 5))
        .await
        .unwrap();

    let view = get_risk(&store, first.id()).await.unwrap();
    assert_eq!(view.reference, "R-001");
    assert_eq!(view.project_code, "WEBR");
    assert_eq!(view.project_id, project.id());
    assert_eq!(view.description, "Vendor late");
    assert_eq!((view.likelihood, view.impact, view.score), (3, 4, 12));
    assert!(!view.closed);
    assert_eq!(view.version, 1);
    assert_eq!(
        get_risk(&store, second.id()).await.unwrap().reference,
        "R-002"
    );
}

#[tokio::test]
async fn likelihood_and_impact_must_be_between_one_and_five() {
    let (store, ada, project) = setup().await;
    for (l, i) in [(0, 3), (6, 3), (3, 0), (3, 6)] {
        let err = create_risk(&store, ada.id(), new(&project, "Risk", l, i))
            .await
            .unwrap_err();
        assert!(
            matches!(err, RiskError::InvalidRating(_)),
            "{l},{i}: {err:?}"
        );
    }
    assert_eq!(
        create_risk(&store, ada.id(), new(&project, " ", 3, 3))
            .await
            .unwrap_err(),
        RiskError::InvalidDescription(DescriptionError::Empty)
    );
    let page = list_risks(
        &store,
        PageRequest::first(10),
        filter(&project, true),
        RiskSort::Id,
    )
    .await
    .unwrap();
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn ratings_out_of_range_are_rejected_on_edit() {
    let (store, ada, project) = setup().await;
    let risk = create_risk(&store, ada.id(), new(&project, "Risk", 3, 3))
        .await
        .unwrap();
    let err = update_risk(
        &store,
        ada.id(),
        risk.id(),
        1,
        RiskEdit {
            description: "Risk",
            likelihood: 3,
            impact: 9,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, RiskError::InvalidRating(RatingError::OutOfRange));
}

#[tokio::test]
async fn a_risk_in_an_unknown_project_is_rejected() {
    let (store, ada, _) = setup().await;
    let err = create_risk(
        &store,
        ada.id(),
        NewRisk {
            project_id: ProjectId::new(),
            description: "Risk",
            likelihood: 1,
            impact: 1,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err, RiskError::UnknownProject);
}

#[tokio::test]
async fn editing_changes_ratings_and_records_history() {
    let (store, ada, project) = setup().await;
    let risk = create_risk(&store, ada.id(), new(&project, "Risk", 1, 1))
        .await
        .unwrap();
    update_risk(
        &store,
        ada.id(),
        risk.id(),
        1,
        RiskEdit {
            description: "Bigger risk",
            likelihood: 5,
            impact: 4,
        },
    )
    .await
    .unwrap();

    let view = get_risk(&store, risk.id()).await.unwrap();
    assert_eq!(view.description, "Bigger risk");
    assert_eq!((view.score, view.version), (20, 2));
    let history = store
        .entity_history(
            EntityType::Risk,
            risk.id().as_uuid(),
            PageRequest::first(10),
        )
        .await
        .unwrap();
    let kinds: Vec<_> = history.items.iter().map(|e| e.change_kind).collect();
    assert_eq!(kinds, [ChangeKind::Created, ChangeKind::Updated]);
    assert_eq!(history.items[0].project_id, Some(project.id().as_uuid()));
    assert_eq!(history.items[1].acting_person, ada.id());
}

#[tokio::test]
async fn editing_from_a_stale_version_is_a_conflict() {
    let (store, ada, project) = setup().await;
    let risk = create_risk(&store, ada.id(), new(&project, "Risk", 1, 1))
        .await
        .unwrap();
    let edit = RiskEdit {
        description: "Risk",
        likelihood: 2,
        impact: 2,
    };
    update_risk(&store, ada.id(), risk.id(), 1, edit)
        .await
        .unwrap();
    assert_eq!(
        update_risk(&store, ada.id(), risk.id(), 1, edit)
            .await
            .unwrap_err(),
        RiskError::Repo(RepoError::Conflict)
    );
}

#[tokio::test]
async fn closed_risks_are_listed_only_on_request_and_can_be_reopened() {
    let (store, ada, project) = setup().await;
    let risk = create_risk(&store, ada.id(), new(&project, "Risk", 1, 1))
        .await
        .unwrap();
    let closed = set_closed(&store, ada.id(), risk.id(), 1, true)
        .await
        .unwrap();
    assert!(closed.closed());
    // Closing again changes nothing.
    let again = set_closed(&store, ada.id(), risk.id(), 2, true)
        .await
        .unwrap();
    assert_eq!(again.version(), 2);

    let open = list_risks(
        &store,
        PageRequest::first(10),
        filter(&project, false),
        RiskSort::Id,
    )
    .await
    .unwrap();
    assert_eq!(open.total, 0);
    let all = list_risks(
        &store,
        PageRequest::first(10),
        filter(&project, true),
        RiskSort::Id,
    )
    .await
    .unwrap();
    assert!(all.items[0].closed);

    let reopened = set_closed(&store, ada.id(), risk.id(), 2, false)
        .await
        .unwrap();
    assert!(!reopened.closed());
    assert_eq!(reopened.version(), 3);
}

#[tokio::test]
async fn risks_in_an_archived_project_cannot_be_changed() {
    let (store, ada, project) = setup().await;
    let risk = create_risk(&store, ada.id(), new(&project, "Risk", 1, 1))
        .await
        .unwrap();
    set_archived(&store, ada.id(), project.id(), 1, true)
        .await
        .unwrap();

    assert_eq!(
        create_risk(&store, ada.id(), new(&project, "Another", 1, 1))
            .await
            .unwrap_err(),
        RiskError::Archived
    );
    let edit = RiskEdit {
        description: "Risk",
        likelihood: 2,
        impact: 2,
    };
    assert_eq!(
        update_risk(&store, ada.id(), risk.id(), 1, edit)
            .await
            .unwrap_err(),
        RiskError::Archived
    );
    assert_eq!(
        set_closed(&store, ada.id(), risk.id(), 1, true)
            .await
            .unwrap_err(),
        RiskError::Archived
    );
}

#[tokio::test]
async fn risks_can_be_sorted_by_score_highest_first() {
    let (store, ada, project) = setup().await;
    // Ties keep creation order.
    for (d, l, i) in [
        ("Low", 1, 2),
        ("Mid", 3, 3),
        ("High", 5, 5),
        ("Also mid", 3, 3),
    ] {
        create_risk(&store, ada.id(), new(&project, d, l, i))
            .await
            .unwrap();
    }
    let page = list_risks(
        &store,
        PageRequest::first(10),
        filter(&project, false),
        RiskSort::Score,
    )
    .await
    .unwrap();
    let order: Vec<_> = page.items.iter().map(|r| r.description.as_str()).collect();
    assert_eq!(order, ["High", "Mid", "Also mid", "Low"]);
}

#[test]
fn references_are_padded_to_three_digits() {
    use lodestar_core::ports::ReferenceKind;
    assert_eq!(ReferenceKind::Risk.reference(7), "R-007");
    assert_eq!(ReferenceKind::Risk.reference(1000), "R-1000");
}
