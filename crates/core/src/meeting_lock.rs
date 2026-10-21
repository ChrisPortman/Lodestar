//! The Meeting Lock (ADR-0006): at most one per Project, held by one browser
//! session of its Project Leader. It is not a domain resource, so it has no
//! version and no Change History.

pub mod use_cases;

use chrono::{DateTime, SubsecRound, TimeDelta, Utc};

use crate::person::PersonId;
use crate::project::ProjectId;
use crate::session::SessionId;

/// How long a lock stays live after its last heartbeat.
pub const LIVE_FOR: TimeDelta = TimeDelta::minutes(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingLock {
    project_id: ProjectId,
    session: SessionId,
    leader: PersonId,
    acquired_at: DateTime<Utc>,
    heartbeat_at: DateTime<Utc>,
}

/// Everything a stored lock is restored from.
#[derive(Debug, Clone)]
pub struct MeetingLockParts {
    pub project_id: ProjectId,
    pub session: SessionId,
    pub leader: PersonId,
    pub acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
}

impl MeetingLock {
    /// A lock acquired, and so heartbeaten, at `now` (kept to the
    /// microsecond, as stored).
    pub fn acquire(
        project_id: ProjectId,
        session: SessionId,
        leader: PersonId,
        now: DateTime<Utc>,
    ) -> Self {
        let now = now.trunc_subsecs(6);
        Self {
            project_id,
            session,
            leader,
            acquired_at: now,
            heartbeat_at: now,
        }
    }

    pub fn restore(parts: MeetingLockParts) -> Self {
        Self {
            project_id: parts.project_id,
            session: parts.session,
            leader: parts.leader,
            acquired_at: parts.acquired_at,
            heartbeat_at: parts.heartbeat_at,
        }
    }

    /// The same lock, heartbeaten at `now`.
    #[must_use]
    pub fn heartbeat(&self, now: DateTime<Utc>) -> Self {
        Self {
            heartbeat_at: now.trunc_subsecs(6),
            ..self.clone()
        }
    }

    /// Live while its last heartbeat is under [`LIVE_FOR`] old. A stale lock
    /// blocks nothing.
    pub fn is_live(&self, now: DateTime<Utc>) -> bool {
        now - self.heartbeat_at < LIVE_FOR
    }

    pub fn is_held_by(&self, session: &SessionId) -> bool {
        self.session == *session
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn session(&self) -> &SessionId {
        &self.session
    }

    pub fn leader(&self) -> PersonId {
        self.leader
    }

    pub fn acquired_at(&self) -> DateTime<Utc> {
        self.acquired_at
    }

    pub fn heartbeat_at(&self) -> DateTime<Utc> {
        self.heartbeat_at
    }
}
