//! Change History: one snapshot entry per change to any aggregate (ADR-0004).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::person::PersonId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HistoryEntryId(Uuid);

impl HistoryEntryId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for HistoryEntryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Who is making a change, and optionally which entry triggered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeContext {
    pub acting_person: PersonId,
    pub caused_by: Option<HistoryEntryId>,
}

impl ChangeContext {
    pub fn by(acting_person: PersonId) -> Self {
        Self {
            acting_person,
            caused_by: None,
        }
    }

    pub fn caused_by(mut self, entry: HistoryEntryId) -> Self {
        self.caused_by = Some(entry);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    Person,
    Project,
}

impl EntityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Project => "project",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "person" => Some(Self::Person),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Updated,
}

impl ChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "updated" => Some(Self::Updated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    pub id: HistoryEntryId,
    pub entity_type: EntityType,
    pub entity_id: Uuid,
    /// `None` for entities outside any Project, such as People.
    pub project_id: Option<Uuid>,
    /// The entity version this change produced.
    pub version: u32,
    pub change_kind: ChangeKind,
    pub acting_person: PersonId,
    pub recorded_at: DateTime<Utc>,
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
    pub caused_by: Option<HistoryEntryId>,
}

/// The JSON snapshot stored in a history entry.
pub fn snapshot<T: Serialize>(entity: &T) -> serde_json::Value {
    serde_json::to_value(entity).expect("domain entities serialise to JSON")
}
