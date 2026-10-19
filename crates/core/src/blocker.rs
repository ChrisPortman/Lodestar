//! Blockers (FR-50 to FR-54): something stopping progress, with at most one
//! accountable Member, open or cleared, and links to the Actions it blocks.
//! Each Blocker is its own aggregate (ADR-0003); its links are part of it.

pub mod use_cases;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::action::ActionId;
use crate::person::PersonId;
use crate::ports::ReferenceKind;
use crate::project::{Description, ItemRef, PersonRef, ProjectId};

/// Identifies a [`Blocker`]. Always a UUIDv7 for new Blockers (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockerId(Uuid);

impl BlockerId {
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

impl Default for BlockerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BlockerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Every field of a stored Blocker, for [`Blocker::restore`].
#[derive(Debug, Clone)]
pub struct BlockerParts {
    pub id: BlockerId,
    pub project_id: ProjectId,
    pub reference: u32,
    pub description: Description,
    pub accountable: Option<PersonId>,
    pub cleared: bool,
    pub blocked_actions: BTreeSet<ActionId>,
    pub version: u32,
}

/// The Blocker aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocker {
    id: BlockerId,
    project_id: ProjectId,
    reference: u32,
    description: Description,
    accountable: Option<PersonId>,
    cleared: bool,
    blocked_actions: BTreeSet<ActionId>,
    version: u32,
}

impl Blocker {
    /// A new open Blocker with the given Reference number and no links.
    pub fn create(
        project_id: ProjectId,
        reference: u32,
        description: Description,
        accountable: Option<PersonId>,
    ) -> Self {
        Self {
            id: BlockerId::new(),
            project_id,
            reference,
            description,
            accountable,
            cleared: false,
            blocked_actions: BTreeSet::new(),
            version: 1,
        }
    }

    /// Rebuilds a stored Blocker. For repository adapters only.
    pub fn restore(parts: BlockerParts) -> Self {
        Self {
            id: parts.id,
            project_id: parts.project_id,
            reference: parts.reference,
            description: parts.description,
            accountable: parts.accountable,
            cleared: parts.cleared,
            blocked_actions: parts.blocked_actions,
            version: parts.version,
        }
    }

    fn next(&self) -> Self {
        Self {
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn edited(&self, description: Description, accountable: Option<PersonId>) -> Self {
        Self {
            description,
            accountable,
            ..self.next()
        }
    }

    pub fn cleared_now(&self) -> Self {
        Self {
            cleared: true,
            ..self.next()
        }
    }

    /// The Blocker reopened, unassigned if its accountable Person is no
    /// longer a Member.
    pub fn reopened(&self, is_member: impl Fn(PersonId) -> bool) -> Self {
        Self {
            cleared: false,
            accountable: self.accountable.filter(|&a| is_member(a)),
            ..self.next()
        }
    }

    pub fn unassigned(&self) -> Self {
        Self {
            accountable: None,
            ..self.next()
        }
    }

    pub fn linked_to(&self, action: ActionId) -> Self {
        let mut after = self.next();
        after.blocked_actions.insert(action);
        after
    }

    pub fn unlinked_from(&self, action: ActionId) -> Self {
        let mut after = self.next();
        after.blocked_actions.remove(&action);
        after
    }

    pub fn id(&self) -> BlockerId {
        self.id
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn reference(&self) -> u32 {
        self.reference
    }

    pub fn reference_label(&self) -> String {
        ReferenceKind::Blocker.reference(self.reference)
    }

    pub fn description(&self) -> &Description {
        &self.description
    }

    pub fn accountable(&self) -> Option<PersonId> {
        self.accountable
    }

    pub fn cleared(&self) -> bool {
        self.cleared
    }

    pub fn blocked_actions(&self) -> &BTreeSet<ActionId> {
        &self.blocked_actions
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

/// A read-only view of a Blocker, served by [`crate::ports::BlockersQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockerView {
    pub id: BlockerId,
    pub project_id: ProjectId,
    pub project_code: String,
    /// The label, e.g. `B-001`.
    pub reference: String,
    pub description: String,
    pub accountable: Option<PersonRef>,
    pub cleared: bool,
    /// The Actions it blocks, ordered by Reference.
    pub blocked_actions: Vec<ItemRef>,
    pub version: u32,
}
