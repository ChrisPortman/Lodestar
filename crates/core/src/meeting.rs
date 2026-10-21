//! Meetings (ADR-0006, #82). A Meeting is created once, when its leader ends
//! it, and never changes afterwards; its Minutes are frozen with it.

pub mod use_cases;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::minutes::Minutes;
use crate::person::PersonId;
use crate::project::{PersonRef, ProjectId};

/// Identifies a [`Meeting`]. Always a UUIDv7 for new Meetings (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MeetingId(Uuid);

impl MeetingId {
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

impl Default for MeetingId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MeetingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Every field of a stored Meeting, for [`Meeting::restore`].
#[derive(Debug, Clone)]
pub struct MeetingParts {
    pub id: MeetingId,
    pub project_id: ProjectId,
    pub leader: PersonId,
    pub attendees: Vec<PersonId>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub minutes: Minutes,
    pub version: u32,
}

/// The Meeting aggregate. Its leader is always its first attendee.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meeting {
    id: MeetingId,
    project_id: ProjectId,
    leader: PersonId,
    attendees: Vec<PersonId>,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    minutes: Minutes,
    version: u32,
}

impl Meeting {
    /// A new Meeting led by `leader`, who is put first among the attendees
    /// and listed once.
    pub fn create(
        id: MeetingId,
        project_id: ProjectId,
        leader: PersonId,
        attendees: &[PersonId],
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
        minutes: Minutes,
    ) -> Self {
        Self {
            id,
            project_id,
            leader,
            attendees: attendees_led_by(leader, attendees),
            started_at,
            ended_at,
            minutes,
            version: 1,
        }
    }

    /// Rebuilds a stored Meeting. For repository adapters only.
    pub fn restore(parts: MeetingParts) -> Self {
        Self {
            id: parts.id,
            project_id: parts.project_id,
            leader: parts.leader,
            attendees: parts.attendees,
            started_at: parts.started_at,
            ended_at: parts.ended_at,
            minutes: parts.minutes,
            version: parts.version,
        }
    }

    pub fn id(&self) -> MeetingId {
        self.id
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn leader(&self) -> PersonId {
        self.leader
    }

    pub fn attendees(&self) -> &[PersonId] {
        &self.attendees
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn ended_at(&self) -> DateTime<Utc> {
        self.ended_at
    }

    pub fn minutes(&self) -> &Minutes {
        &self.minutes
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

/// `leader` first, then the others in the order given, each once.
pub fn attendees_led_by(leader: PersonId, attendees: &[PersonId]) -> Vec<PersonId> {
    let mut out = vec![leader];
    for &a in attendees {
        if !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

/// A Meeting as listed: everything but its Minutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingSummary {
    pub id: MeetingId,
    pub project_id: ProjectId,
    pub leader: PersonRef,
    /// The leader first, then the others in the order given.
    pub attendees: Vec<PersonRef>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub version: u32,
}

/// A Meeting with its Minutes, served by [`crate::ports::MeetingsQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingView {
    pub summary: MeetingSummary,
    pub minutes: Minutes,
}
