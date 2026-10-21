//! Meeting Minutes built from a window of Change History (#81).
//!
//! [`build_minutes`] is pure: it takes the window's history entries and the
//! names it needs, and returns a fully resolved [`Minutes`] document that is
//! stored frozen. It reads snapshots as loose JSON, so entries written by
//! older versions of an aggregate still work (ADR-0004).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::action::ActionId;
use crate::blocker::BlockerId;
use crate::comment::TargetType;
use crate::history::{ChangeKind, EntityType, HistoryEntry};
use crate::person::PersonId;
use crate::ports::{ActionsQuery, BlockersQuery, PeopleQuery, ReferenceKind, RisksQuery};
use crate::risk::RiskId;

/// The version of the stored [`Minutes`] JSON shape.
pub const MINUTES_SCHEMA: u32 = 1;

/// The fixed record of a Meeting, with every name and Reference resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Minutes {
    pub schema: u32,
    pub project_name: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub duration_minutes: i64,
    pub leader: String,
    pub attendees: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Section>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risks: Option<Section>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blockers: Option<Section>,
    /// Comment bodies about the Meeting or the Project as a whole.
    #[serde(default)]
    pub meeting_comments: Vec<String>,
}

/// New and Updated items of one type. Never both empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new: Vec<NewItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<UpdatedItem>,
}

/// An item created in the window, as it stood at the end of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewItem {
    pub reference: String,
    pub title: String,
    /// e.g. `Assigned to Sam Lee`, `Due 31 Oct`.
    #[serde(default)]
    pub state: Vec<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

/// An existing item changed or commented on in the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedItem {
    pub reference: String,
    pub title: String,
    /// Net Changes, e.g. `Due 24 Oct → 7 Nov`, `Reassigned to Sam Lee`, `Closed`.
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

/// What the Minutes say about the Meeting itself, given by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinutesHeader {
    pub project_name: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub leader: String,
    pub attendees: Vec<String>,
}

/// An item's Reference and title, for items with no entries in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLabel {
    pub reference: String,
    pub title: String,
}

/// Names the window's history refers to but does not hold itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookups {
    pub people: HashMap<PersonId, String>,
    pub items: HashMap<Uuid, ItemLabel>,
}

/// Which People and items [`build_minutes`] needs names for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Needed {
    pub people: BTreeSet<PersonId>,
    pub items: BTreeSet<(ItemType, Uuid)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemType {
    Action,
    Risk,
    Blocker,
}

impl ItemType {
    fn of_entity(t: EntityType) -> Option<Self> {
        match t {
            EntityType::Action => Some(Self::Action),
            EntityType::Risk => Some(Self::Risk),
            EntityType::Blocker => Some(Self::Blocker),
            _ => None,
        }
    }

    fn of_target(t: TargetType) -> Option<Self> {
        match t {
            TargetType::Action => Some(Self::Action),
            TargetType::Risk => Some(Self::Risk),
            TargetType::Blocker => Some(Self::Blocker),
            TargetType::Project => None,
        }
    }

    fn reference_kind(self) -> ReferenceKind {
        match self {
            Self::Action => ReferenceKind::Action,
            Self::Risk => ReferenceKind::Risk,
            Self::Blocker => ReferenceKind::Blocker,
        }
    }
}

/// The net effect of the window on one item.
struct ItemWindow<'a> {
    kind: ItemType,
    /// State before the window; `None` when created in it.
    first: Option<&'a Value>,
    last: &'a Value,
    comments: Vec<String>,
}

/// A Comment made in the window that still stands at its end.
struct WindowComment<'a> {
    target: Option<(ItemType, Uuid)>,
    body: &'a str,
}

/// Comments created in the window and not deleted by its end, oldest first.
fn window_comments(history: &[HistoryEntry]) -> Vec<WindowComment<'_>> {
    let mut created: Vec<Uuid> = Vec::new();
    let mut last: HashMap<Uuid, &Value> = HashMap::new();
    for e in history
        .iter()
        .filter(|e| e.entity_type == EntityType::Comment)
    {
        if e.change_kind == ChangeKind::Created {
            created.push(e.entity_id);
        }
        last.insert(e.entity_id, &e.after);
    }
    created
        .into_iter()
        .filter_map(|id| {
            let c = last[&id];
            if c.get("deleted").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let target = c
                .get("target_type")
                .and_then(Value::as_str)
                .and_then(TargetType::parse)
                .and_then(ItemType::of_target)
                .zip(c.get("target_id").and_then(uuid_of));
            Some(WindowComment {
                target,
                body: c.get("body").and_then(Value::as_str).unwrap_or_default(),
            })
        })
        .collect()
}

fn item_windows(history: &[HistoryEntry]) -> BTreeMap<Uuid, ItemWindow<'_>> {
    let mut items: BTreeMap<Uuid, ItemWindow<'_>> = BTreeMap::new();
    for e in history {
        let Some(kind) = ItemType::of_entity(e.entity_type) else {
            continue;
        };
        items
            .entry(e.entity_id)
            .and_modify(|w| w.last = &e.after)
            .or_insert(ItemWindow {
                kind,
                first: match e.change_kind {
                    ChangeKind::Created => None,
                    ChangeKind::Updated => Some(e.before.as_ref().unwrap_or(&e.after)),
                },
                last: &e.after,
                comments: Vec::new(),
            });
    }
    items
}

/// The People and items to look up before calling [`build_minutes`].
pub fn needed(history: &[HistoryEntry]) -> Needed {
    let items = item_windows(history);
    let mut needed = Needed::default();
    for w in items.values() {
        for snap in w.first.into_iter().chain([w.last]) {
            for field in ["assignee", "accountable"] {
                if let Some(id) = snap.get(field).and_then(uuid_of) {
                    needed.people.insert(PersonId::from_uuid(id));
                }
            }
            for id in uuids(snap.get("blocked_actions")) {
                if !items.contains_key(&id) {
                    needed.items.insert((ItemType::Action, id));
                }
            }
        }
    }
    for c in window_comments(history) {
        if let Some((kind, id)) = c.target
            && !items.contains_key(&id)
        {
            needed.items.insert((kind, id));
        }
    }
    needed
}

/// Builds the Minutes for a window from its history entries, oldest first.
pub fn build_minutes(
    header: MinutesHeader,
    history: &[HistoryEntry],
    lookups: &Lookups,
) -> Minutes {
    let mut items = item_windows(history);
    let mut meeting_comments = Vec::new();
    for c in window_comments(history) {
        match c.target {
            None => meeting_comments.push(c.body.to_owned()),
            Some((kind, id)) => items
                .entry(id)
                .or_insert_with(|| ItemWindow {
                    kind,
                    first: None,
                    last: &Value::Null,
                    comments: Vec::new(),
                })
                .comments
                .push(c.body.to_owned()),
        }
    }
    let labels = Labels {
        lookups,
        items: &items,
    };

    let mut sections: BTreeMap<ItemType, Vec<(u32, Entry)>> = BTreeMap::new();
    for (id, w) in &items {
        let (number, entry) = if w.last.is_null() {
            // Only commented on: the item had no entries in the window.
            let label = labels.item(w.kind, *id);
            let entry = Entry::Updated(UpdatedItem {
                reference: label.reference,
                title: label.title,
                changes: Vec::new(),
                comments: w.comments.clone(),
            });
            (u32::MAX, entry)
        } else {
            let reference = labels.reference(w.kind, w.last);
            let title = text(w.last.get("description"));
            let entry = match w.first {
                None => Entry::New(NewItem {
                    reference,
                    title,
                    state: labels.state(w.kind, w.last),
                    comments: w.comments.clone(),
                }),
                Some(first) => {
                    let changes = labels.changes(w.kind, first, w.last);
                    if changes.is_empty() && w.comments.is_empty() {
                        continue;
                    }
                    Entry::Updated(UpdatedItem {
                        reference,
                        title,
                        changes,
                        comments: w.comments.clone(),
                    })
                }
            };
            (number_of(w.last), entry)
        };
        sections.entry(w.kind).or_default().push((number, entry));
    }

    let mut section = |kind| {
        let mut entries = sections.remove(&kind)?;
        entries.sort_by(|a, b| (a.0, a.1.reference()).cmp(&(b.0, b.1.reference())));
        let mut s = Section {
            new: Vec::new(),
            updated: Vec::new(),
        };
        for (_, e) in entries {
            match e {
                Entry::New(n) => s.new.push(n),
                Entry::Updated(u) => s.updated.push(u),
            }
        }
        Some(s)
    };

    Minutes {
        schema: MINUTES_SCHEMA,
        duration_minutes: (header.end - header.start).num_minutes(),
        project_name: header.project_name,
        start: header.start,
        end: header.end,
        leader: header.leader,
        attendees: header.attendees,
        actions: section(ItemType::Action),
        risks: section(ItemType::Risk),
        blockers: section(ItemType::Blocker),
        meeting_comments,
    }
}

enum Entry {
    New(NewItem),
    Updated(UpdatedItem),
}

impl Entry {
    fn reference(&self) -> &str {
        match self {
            Self::New(n) => &n.reference,
            Self::Updated(u) => &u.reference,
        }
    }
}

struct Labels<'a> {
    lookups: &'a Lookups,
    items: &'a BTreeMap<Uuid, ItemWindow<'a>>,
}

impl Labels<'_> {
    fn person(&self, id: Uuid) -> String {
        self.lookups
            .people
            .get(&PersonId::from_uuid(id))
            .cloned()
            .unwrap_or_else(|| "an unknown Person".to_owned())
    }

    fn reference(&self, kind: ItemType, snap: &Value) -> String {
        kind.reference_kind().reference(number_of(snap))
    }

    fn item(&self, kind: ItemType, id: Uuid) -> ItemLabel {
        match self.items.get(&id) {
            Some(w) if !w.last.is_null() => ItemLabel {
                reference: self.reference(w.kind, w.last),
                title: text(w.last.get("description")),
            },
            _ => self.lookups.items.get(&id).cloned().unwrap_or(ItemLabel {
                reference: kind.reference_kind().reference(0),
                title: "an unknown item".to_owned(),
            }),
        }
    }

    fn action_refs(&self, snap: &Value) -> String {
        let mut refs: Vec<String> = uuids(snap.get("blocked_actions"))
            .map(|id| self.item(ItemType::Action, id).reference)
            .collect();
        refs.sort();
        if refs.is_empty() {
            "none".to_owned()
        } else {
            refs.join(", ")
        }
    }

    /// An item's state when new: the things worth saying beyond its title.
    fn state(&self, kind: ItemType, s: &Value) -> Vec<String> {
        let mut out = Vec::new();
        match kind {
            ItemType::Action => {
                if let Some(id) = s.get("assignee").and_then(uuid_of) {
                    out.push(format!("Assigned to {}", self.person(id)));
                }
                if let Some(d) = date(s.get("due_date")) {
                    out.push(format!("Due {d}"));
                }
            }
            ItemType::Risk => {
                out.push(format!("Likelihood {}", text(s.get("likelihood"))));
                out.push(format!("Impact {}", text(s.get("impact"))));
            }
            ItemType::Blocker => {
                if let Some(id) = s.get("accountable").and_then(uuid_of) {
                    out.push(format!("Accountable {}", self.person(id)));
                }
                if uuids(s.get("blocked_actions")).next().is_some() {
                    out.push(format!("Blocks {}", self.action_refs(s)));
                }
            }
        }
        if flag(s, closed_field(kind)) {
            out.push(closed_word(kind).to_owned());
        }
        out
    }

    /// Net Changes from `a` to `b`, ignoring fields either side lacks.
    fn changes(&self, kind: ItemType, a: &Value, b: &Value) -> Vec<String> {
        let changed = |f: &str| match (a.get(f), b.get(f)) {
            (Some(x), Some(y)) => x != y,
            _ => false,
        };
        let mut out = Vec::new();
        if changed("description") {
            out.push(format!(
                "Description {} → {}",
                text(a.get("description")),
                text(b.get("description"))
            ));
        }
        for field in ["assignee", "accountable"] {
            if changed(field) {
                out.push(match b.get(field).and_then(uuid_of) {
                    Some(id) => format!("Reassigned to {}", self.person(id)),
                    None => "Unassigned".to_owned(),
                });
            }
        }
        if changed("due_date") {
            let due = |s: &Value| date(s.get("due_date")).unwrap_or_else(|| "none".to_owned());
            out.push(format!("Due {} → {}", due(a), due(b)));
        }
        for (field, name) in [("likelihood", "Likelihood"), ("impact", "Impact")] {
            if changed(field) {
                out.push(format!(
                    "{name} {} → {}",
                    text(a.get(field)),
                    text(b.get(field))
                ));
            }
        }
        if changed("blocked_actions") {
            let (x, y) = (self.action_refs(a), self.action_refs(b));
            if x != y {
                out.push(format!("Blocks {x} → {y}"));
            }
        }
        let closed = closed_field(kind);
        if changed(closed) {
            out.push(if flag(b, closed) {
                closed_word(kind).to_owned()
            } else {
                "Reopened".to_owned()
            });
        }
        out
    }
}

fn closed_field(kind: ItemType) -> &'static str {
    match kind {
        ItemType::Blocker => "cleared",
        _ => "closed",
    }
}

fn closed_word(kind: ItemType) -> &'static str {
    match kind {
        ItemType::Blocker => "Cleared",
        _ => "Closed",
    }
}

fn number_of(snap: &Value) -> u32 {
    snap.get("reference")
        .or_else(|| snap.get("number"))
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or_default()
}

fn flag(snap: &Value, field: &str) -> bool {
    snap.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "none".to_owned(),
        Some(other) => other.to_string(),
    }
}

fn date(v: Option<&Value>) -> Option<String> {
    let d = NaiveDate::parse_from_str(v?.as_str()?, "%Y-%m-%d").ok()?;
    Some(d.format("%-d %b").to_string())
}

fn uuid_of(v: &Value) -> Option<Uuid> {
    Uuid::parse_str(v.as_str()?).ok()
}

fn uuids(v: Option<&Value>) -> impl Iterator<Item = Uuid> + '_ {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(uuid_of)
}

/// Looks up every name in `needed`, one query per People or item ID.
/// Unknown IDs are left out; [`build_minutes`] shows them as unknown.
pub async fn resolve<S>(store: &S, needed: &Needed) -> Lookups
where
    S: PeopleQuery + ActionsQuery + RisksQuery + BlockersQuery,
{
    let mut lookups = Lookups::default();
    for &id in &needed.people {
        if let Ok(p) = store.get_person(id).await {
            lookups.people.insert(id, p.display_name);
        }
    }
    for &(kind, id) in &needed.items {
        let label = match kind {
            ItemType::Action => store
                .get_action(ActionId::from_uuid(id))
                .await
                .map(|v| (v.reference, v.description)),
            ItemType::Risk => store
                .get_risk(RiskId::from_uuid(id))
                .await
                .map(|v| (v.reference, v.description)),
            ItemType::Blocker => store
                .get_blocker(BlockerId::from_uuid(id))
                .await
                .map(|v| (v.reference, v.description)),
        };
        if let Ok((reference, title)) = label {
            lookups.items.insert(id, ItemLabel { reference, title });
        }
    }
    lookups
}
