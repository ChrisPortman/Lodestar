//! Actions (FR-30 to FR-33): a piece of work assigned to at most one Member,
//! open or closed. Each Action is its own aggregate (ADR-0003).

pub mod use_cases;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::person::PersonId;
use crate::ports::ReferenceKind;
use crate::project::{Description, PersonRef, ProjectId};

/// Identifies an [`Action`]. Always a UUIDv7 for new Actions (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActionId(Uuid);

impl ActionId {
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

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Every field of a stored Action, for [`Action::restore`].
#[derive(Debug, Clone)]
pub struct ActionParts {
    pub id: ActionId,
    pub project_id: ProjectId,
    pub reference: u32,
    pub description: Description,
    pub assignee: Option<PersonId>,
    pub due_date: Option<NaiveDate>,
    pub closed: bool,
    pub version: u32,
}

/// The Action aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    id: ActionId,
    project_id: ProjectId,
    reference: u32,
    description: Description,
    assignee: Option<PersonId>,
    due_date: Option<NaiveDate>,
    closed: bool,
    version: u32,
}

impl Action {
    /// A new open Action with the given Reference number.
    pub fn create(
        project_id: ProjectId,
        reference: u32,
        description: Description,
        assignee: Option<PersonId>,
        due_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            id: ActionId::new(),
            project_id,
            reference,
            description,
            assignee,
            due_date,
            closed: false,
            version: 1,
        }
    }

    /// Rebuilds a stored Action. For repository adapters only.
    pub fn restore(parts: ActionParts) -> Self {
        Self {
            id: parts.id,
            project_id: parts.project_id,
            reference: parts.reference,
            description: parts.description,
            assignee: parts.assignee,
            due_date: parts.due_date,
            closed: parts.closed,
            version: parts.version,
        }
    }

    pub fn edited(
        &self,
        description: Description,
        assignee: Option<PersonId>,
        due_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            description,
            assignee,
            due_date,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn closed_now(&self) -> Self {
        Self {
            closed: true,
            version: self.version + 1,
            ..self.clone()
        }
    }

    /// The Action reopened, unassigned if its assignee is no longer a Member.
    pub fn reopened(&self, is_member: impl Fn(PersonId) -> bool) -> Self {
        Self {
            closed: false,
            assignee: self.assignee.filter(|&a| is_member(a)),
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn unassigned(&self) -> Self {
        Self {
            assignee: None,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn id(&self) -> ActionId {
        self.id
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn reference(&self) -> u32 {
        self.reference
    }

    pub fn reference_label(&self) -> String {
        ReferenceKind::Action.reference(self.reference)
    }

    pub fn description(&self) -> &Description {
        &self.description
    }

    pub fn assignee(&self) -> Option<PersonId> {
        self.assignee
    }

    pub fn due_date(&self) -> Option<NaiveDate> {
        self.due_date
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

/// A read-only view of an Action, served by [`crate::ports::ActionsQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionView {
    pub id: ActionId,
    pub project_id: ProjectId,
    pub project_code: String,
    /// The label, e.g. `A-001`.
    pub reference: String,
    pub description: String,
    pub assignee: Option<PersonRef>,
    pub due_date: Option<NaiveDate>,
    pub closed: bool,
    pub version: u32,
}
