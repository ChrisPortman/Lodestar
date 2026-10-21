use chrono::{DateTime, TimeZone, Utc};
use lodestar_core::history::{ChangeKind, EntityType, HistoryEntry, HistoryEntryId};
use lodestar_core::minutes::{
    ItemLabel, Lookups, Minutes, NewItem, Section, UpdatedItem, build_minutes, needed, resolve,
};
use lodestar_core::person::PersonId;
use lodestar_core::ports::{PersonRepository, Transaction, UnitOfWork};
use lodestar_core::test_support::{InMemoryStore, a_person};
use serde_json::{Value, json};
use uuid::Uuid;

const ACTION: &str = "00000000-0000-0000-0000-00000000000a";
const RISK: &str = "00000000-0000-0000-0000-00000000000b";
const BLOCKER: &str = "00000000-0000-0000-0000-00000000000c";
const SAM: &str = "00000000-0000-0000-0000-0000000000f1";
const JO: &str = "00000000-0000-0000-0000-0000000000f2";

fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 21, 9, minute, 0).unwrap()
}

/// A run of entries for one entity: each `after` snapshot in turn, the first
/// created in the window when `created`, else updated from `before`.
fn entity(
    kind: EntityType,
    id: &str,
    before: Option<Value>,
    afters: &[Value],
) -> Vec<HistoryEntry> {
    let mut prev = before.clone();
    afters
        .iter()
        .enumerate()
        .map(|(i, after)| {
            let entry = HistoryEntry {
                id: HistoryEntryId::new(),
                entity_type: kind,
                entity_id: Uuid::parse_str(id).unwrap(),
                project_id: None,
                version: i as u32 + 1,
                change_kind: if prev.is_none() {
                    ChangeKind::Created
                } else {
                    ChangeKind::Updated
                },
                acting_person: PersonId::new(),
                recorded_at: at(i as u32),
                before: prev.clone(),
                after: after.clone(),
                caused_by: None,
            };
            prev = Some(after.clone());
            entry
        })
        .collect()
}

fn action(description: &str, assignee: Option<&str>, due: Option<&str>, closed: bool) -> Value {
    json!({"reference": 12, "description": description, "assignee": assignee,
           "due_date": due, "closed": closed, "version": 1})
}

fn comment(target_type: &str, target: &str, body: &str, deleted: bool) -> Value {
    json!({"target_type": target_type, "target_id": target, "body": body, "deleted": deleted})
}

fn lookups() -> Lookups {
    let mut l = Lookups::default();
    l.people.insert(
        PersonId::from_uuid(Uuid::parse_str(SAM).unwrap()),
        "Sam Lee".into(),
    );
    l.people.insert(
        PersonId::from_uuid(Uuid::parse_str(JO).unwrap()),
        "Jo Park".into(),
    );
    l
}

fn minutes(history: &[HistoryEntry], lookups: &Lookups) -> Minutes {
    let header = lodestar_core::minutes::MinutesHeader {
        project_name: "Website rebuild".into(),
        start: at(0),
        end: at(45),
        leader: "Sam Lee".into(),
        attendees: vec!["Sam Lee".into(), "Jo Park".into()],
    };
    build_minutes(header, history, lookups)
}

fn updated(reference: &str, title: &str, changes: &[&str], comments: &[&str]) -> Section {
    Section {
        new: vec![],
        updated: vec![UpdatedItem {
            reference: reference.into(),
            title: title.into(),
            changes: changes.iter().map(|s| s.to_string()).collect(),
            comments: comments.iter().map(|s| s.to_string()).collect(),
        }],
    }
}

#[test]
fn action_cases() {
    let open = action("Book removalists", Some(SAM), Some("2026-10-24"), false);
    let cases: Vec<(&str, Vec<HistoryEntry>, Option<Section>)> = vec![
        (
            "create then edit in the window is New, in its final state",
            entity(
                EntityType::Action,
                ACTION,
                None,
                &[
                    open.clone(),
                    action(
                        "Book removalists for stage 2",
                        Some(JO),
                        Some("2026-10-31"),
                        false,
                    ),
                ],
            ),
            Some(Section {
                new: vec![NewItem {
                    reference: "A-012".into(),
                    title: "Book removalists for stage 2".into(),
                    state: vec!["Assigned to Jo Park".into(), "Due 31 Oct".into()],
                    comments: vec![],
                }],
                updated: vec![],
            }),
        ),
        (
            "edit then revert leaves no Changes and is left out",
            entity(
                EntityType::Action,
                ACTION,
                Some(open.clone()),
                &[
                    action("Other", Some(SAM), Some("2026-10-24"), false),
                    open.clone(),
                ],
            ),
            None,
        ),
        (
            "net Changes per field",
            entity(
                EntityType::Action,
                ACTION,
                Some(open.clone()),
                &[
                    action("Book removalists", Some(SAM), Some("2026-11-01"), false),
                    action("Book movers", Some(JO), Some("2026-11-07"), true),
                ],
            ),
            Some(updated(
                "A-012",
                "Book movers",
                &[
                    "Description Book removalists → Book movers",
                    "Reassigned to Jo Park",
                    "Due 24 Oct → 7 Nov",
                    "Closed",
                ],
                &[],
            )),
        ),
        (
            "close then reopen nets to nothing",
            entity(
                EntityType::Action,
                ACTION,
                Some(open.clone()),
                &[
                    action("Book removalists", Some(SAM), Some("2026-10-24"), true),
                    open.clone(),
                ],
            ),
            None,
        ),
        (
            "reopening a closed Action",
            entity(
                EntityType::Action,
                ACTION,
                Some(action(
                    "Book removalists",
                    Some(SAM),
                    Some("2026-10-24"),
                    true,
                )),
                std::slice::from_ref(&open),
            ),
            Some(updated("A-012", "Book removalists", &["Reopened"], &[])),
        ),
        (
            "unassigning",
            entity(
                EntityType::Action,
                ACTION,
                Some(open.clone()),
                &[action("Book removalists", None, Some("2026-10-24"), false)],
            ),
            Some(updated("A-012", "Book removalists", &["Unassigned"], &[])),
        ),
        (
            "an older snapshot missing a field is tolerated",
            entity(
                EntityType::Action,
                ACTION,
                Some(json!({"reference": 12, "description": "Book removalists", "closed": false})),
                std::slice::from_ref(&open),
            ),
            None,
        ),
    ];
    for (name, history, expected) in cases {
        assert_eq!(minutes(&history, &lookups()).actions, expected, "{name}");
    }
}

#[test]
fn risks_and_blockers_use_their_own_fields() {
    let risk = |l: u8, closed: bool| json!({"number": 3, "description": "Supplier fails", "likelihood": l, "impact": 4, "closed": closed});
    let blocker = |accountable: Option<&str>, blocked: Vec<&str>, cleared: bool| {
        json!({"reference": 1, "description": "No lease", "accountable": accountable,
               "blocked_actions": blocked, "cleared": cleared})
    };
    let mut history = entity(
        EntityType::Risk,
        RISK,
        Some(risk(2, false)),
        &[risk(4, true)],
    );
    history.extend(entity(
        EntityType::Blocker,
        BLOCKER,
        Some(blocker(Some(SAM), vec![], false)),
        &[blocker(Some(JO), vec![ACTION], true)],
    ));
    let mut l = lookups();
    l.items.insert(
        Uuid::parse_str(ACTION).unwrap(),
        ItemLabel {
            reference: "A-012".into(),
            title: "Book removalists".into(),
        },
    );

    let m = minutes(&history, &l);

    assert_eq!(
        m.risks,
        Some(updated(
            "R-003",
            "Supplier fails",
            &["Likelihood 2 → 4", "Closed"],
            &[]
        ))
    );
    assert_eq!(
        m.blockers,
        Some(updated(
            "B-001",
            "No lease",
            &["Reassigned to Jo Park", "Blocks none → A-012", "Cleared"],
            &[]
        ))
    );
    assert_eq!(m.actions, None);
}

#[test]
fn comments_made_in_the_window_go_with_their_item_or_the_meeting() {
    let project = Uuid::now_v7().to_string();
    let c1 = "00000000-0000-0000-0000-0000000000c1";
    let c2 = "00000000-0000-0000-0000-0000000000c2";
    let c3 = "00000000-0000-0000-0000-0000000000c3";
    let c4 = "00000000-0000-0000-0000-0000000000c4";
    let mut history = entity(
        EntityType::Comment,
        c1,
        None,
        &[
            comment("action", ACTION, "Quote too high", false),
            comment("action", ACTION, "Quote far too high", false),
        ],
    );
    history.extend(entity(
        EntityType::Comment,
        c2,
        None,
        &[
            comment("action", ACTION, "Typo", false),
            comment("action", ACTION, "Typo", true),
        ],
    ));
    history.extend(entity(
        EntityType::Comment,
        c3,
        None,
        &[comment("project", &project, "Budget approved", false)],
    ));
    // Made before the window, edited during it: not a Comment made in the window.
    history.extend(entity(
        EntityType::Comment,
        c4,
        Some(comment("action", ACTION, "Old", false)),
        &[comment("action", ACTION, "Old, edited", false)],
    ));

    let n = needed(&history);
    assert_eq!(
        n.items
            .iter()
            .map(|(_, id)| id.to_string())
            .collect::<Vec<_>>(),
        [ACTION]
    );
    let mut l = lookups();
    l.items.insert(
        Uuid::parse_str(ACTION).unwrap(),
        ItemLabel {
            reference: "A-012".into(),
            title: "Book removalists".into(),
        },
    );
    let m = minutes(&history, &l);

    assert_eq!(
        m.actions,
        Some(updated(
            "A-012",
            "Book removalists",
            &[],
            &["Quote far too high"]
        ))
    );
    assert_eq!(m.meeting_comments, ["Budget approved"]);
}

#[test]
fn comments_on_another_meeting_are_left_out() {
    let earlier_meeting = Uuid::now_v7().to_string();
    let history = entity(
        EntityType::Comment,
        "00000000-0000-0000-0000-0000000000c5",
        None,
        &[comment("meeting", &earlier_meeting, "Follow-up", false)],
    );
    assert_eq!(needed(&history), Default::default());
    let m = minutes(&history, &lookups());
    assert!(m.meeting_comments.is_empty());
    assert_eq!(m.actions, None);
}

#[test]
fn the_document_has_a_header_and_a_stable_versioned_json_shape() {
    let history = entity(
        EntityType::Risk,
        RISK,
        None,
        &[
            json!({"number": 1, "description": "Rain", "likelihood": 2, "impact": 3, "closed": false}),
        ],
    );
    let m = minutes(&history, &lookups());

    let stored = serde_json::to_value(&m).unwrap();
    assert_eq!(
        stored,
        json!({
            "schema": 1,
            "project_name": "Website rebuild",
            "start": "2026-10-21T09:00:00Z",
            "end": "2026-10-21T09:45:00Z",
            "duration_minutes": 45,
            "leader": "Sam Lee",
            "attendees": ["Sam Lee", "Jo Park"],
            "risks": {"new": [{"reference": "R-001", "title": "Rain",
                               "state": ["Likelihood 2", "Impact 3"], "comments": []}]},
            "meeting_comments": []
        })
    );
    assert_eq!(serde_json::from_value::<Minutes>(stored).unwrap(), m);
}

#[tokio::test]
async fn names_are_resolved_from_the_store() {
    let store = InMemoryStore::default();
    let person = a_person().named("Ada").build();
    let mut tx = store.begin().await.unwrap();
    tx.people()
        .save(
            None,
            &person,
            &lodestar_core::history::ChangeContext::by(person.id()),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let history = entity(
        EntityType::Action,
        ACTION,
        None,
        &[action(
            "Call",
            Some(&person.id().as_uuid().to_string()),
            None,
            false,
        )],
    );

    let l = resolve(&store, &needed(&history)).await;

    assert_eq!(l.people.get(&person.id()).map(String::as_str), Some("Ada"));
}
