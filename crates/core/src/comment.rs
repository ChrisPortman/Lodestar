//! Comments (FR-120 to FR-127, ADR-0003). A Comment refers to its Project
//! and its target by ID, so commenting never changes the target.

pub mod use_cases;

use chrono::{DateTime, SubsecRound, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::person::PersonId;
use crate::project::{PersonRef, ProjectId};

/// Identifies a [`Comment`]. Always a UUIDv7 for new Comments (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentId(Uuid);

impl CommentId {
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

impl Default for CommentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CommentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What kind of thing a Comment is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    Project,
    Action,
    Risk,
    Blocker,
    Meeting,
}

impl TargetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Action => "action",
            Self::Risk => "risk",
            Self::Blocker => "blocker",
            Self::Meeting => "meeting",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "project" => Some(Self::Project),
            "action" => Some(Self::Action),
            "risk" => Some(Self::Risk),
            "blocker" => Some(Self::Blocker),
            "meeting" => Some(Self::Meeting),
            _ => None,
        }
    }
}

/// What a Comment is about: the Project itself, an item in it, or one of
/// its Meetings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Target {
    #[serde(rename = "target_type")]
    pub kind: TargetType,
    #[serde(rename = "target_id")]
    pub id: Uuid,
}

pub const MAX_BODY_CHARS: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BodyError {
    #[error("a comment must not be blank")]
    Empty,
    #[error("a comment must be at most {MAX_BODY_CHARS} characters")]
    TooLong,
}

/// A Comment's text, kept exactly as written: not blank, at most 4,000
/// characters (FR-121). Markdown is neither rejected nor parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommentBody(String);

impl CommentBody {
    pub fn new(raw: impl Into<String>) -> Result<Self, BodyError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(BodyError::Empty);
        }
        if raw.chars().count() > MAX_BODY_CHARS {
            return Err(BodyError::TooLong);
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommentBody {
    type Error = BodyError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CommentBody> for String {
    fn from(value: CommentBody) -> Self {
        value.0
    }
}

/// A read-only view of a Comment, served by [`crate::ports::CommentsQuery`].
/// `body` is the stored text even when `deleted`; callers decide what to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentView {
    pub id: CommentId,
    pub project_id: ProjectId,
    pub target: Target,
    pub author: PersonRef,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub version: u32,
}

/// Every field of a stored Comment, for [`Comment::restore`].
#[derive(Debug, Clone)]
pub struct CommentParts {
    pub id: CommentId,
    pub project_id: ProjectId,
    pub target: Target,
    pub author: PersonId,
    pub body: CommentBody,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub version: u32,
}

/// The Comment aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    id: CommentId,
    project_id: ProjectId,
    #[serde(flatten)]
    target: Target,
    author: PersonId,
    body: CommentBody,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    deleted: bool,
    version: u32,
}

/// The current time, to the microsecond that storage keeps.
fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

impl Comment {
    /// A new Comment by `author`, created now.
    pub fn create(
        project_id: ProjectId,
        target: Target,
        author: PersonId,
        body: CommentBody,
    ) -> Self {
        Self {
            id: CommentId::new(),
            project_id,
            target,
            author,
            body,
            created_at: now(),
            edited_at: None,
            deleted: false,
            version: 1,
        }
    }

    /// Rebuilds a stored Comment. For repository adapters only.
    pub fn restore(parts: CommentParts) -> Self {
        Self {
            id: parts.id,
            project_id: parts.project_id,
            target: parts.target,
            author: parts.author,
            body: parts.body,
            created_at: parts.created_at,
            edited_at: parts.edited_at,
            deleted: parts.deleted,
            version: parts.version,
        }
    }

    /// The Comment with new text, marked as edited now (FR-122).
    pub fn edited(&self, body: CommentBody) -> Self {
        Self {
            body,
            edited_at: Some(now()),
            version: self.version + 1,
            ..self.clone()
        }
    }

    /// The Comment deleted by its author. The text is kept (FR-123).
    pub fn deleted(&self) -> Self {
        Self {
            deleted: true,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn id(&self) -> CommentId {
        self.id
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn author(&self) -> PersonId {
        self.author
    }

    pub fn body(&self) -> &CommentBody {
        &self.body
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn edited_at(&self) -> Option<DateTime<Utc>> {
        self.edited_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}
