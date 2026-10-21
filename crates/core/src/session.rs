//! The browser session an Acting Person works through (ADR-0006).

use chrono::{DateTime, Utc};

use crate::person::PersonId;

/// Identifies one browser session, e.g. to hold a Meeting Lock. Opaque to
/// core: it is minted and carried by the api layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Who is changing something, through which session, and when. A session
/// that does not hold a Project's live Meeting Lock may not change it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub person: PersonId,
    /// `None` for a session from before session ids existed: it never holds a lock.
    pub session: Option<SessionId>,
    pub now: DateTime<Utc>,
}

/// A person acting now, through no session: refused while any lock is live.
impl From<PersonId> for Actor {
    fn from(person: PersonId) -> Self {
        Self {
            person,
            session: None,
            now: Utc::now(),
        }
    }
}
