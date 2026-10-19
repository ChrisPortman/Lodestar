//! Risks (FR-40 to FR-44, ADR-0003). There is no Risk-to-Blocker conversion:
//! close the Risk and create a Blocker by hand (FR-44).

pub mod use_cases;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::{Description, ProjectId};

/// Identifies a [`Risk`]. Always a UUIDv7 for new Risks (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RiskId(Uuid);

impl RiskId {
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

impl Default for RiskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RiskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub const MIN_RATING: u8 = 1;
pub const MAX_RATING: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RatingError {
    #[error("likelihood and impact must be between {MIN_RATING} and {MAX_RATING}")]
    OutOfRange,
}

/// A likelihood or impact rating, 1 to 5 (FR-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Rating(u8);

impl Rating {
    pub fn new(value: u8) -> Result<Self, RatingError> {
        if (MIN_RATING..=MAX_RATING).contains(&value) {
            Ok(Self(value))
        } else {
            Err(RatingError::OutOfRange)
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Rating {
    type Error = RatingError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Rating> for u8 {
    fn from(value: Rating) -> Self {
        value.0
    }
}

/// The editable fields of a Risk, as given by a client.
#[derive(Debug, Clone, Copy)]
pub struct RiskEdit<'a> {
    pub description: &'a str,
    pub likelihood: u8,
    pub impact: u8,
}

/// A read-only view of a Risk, served by [`crate::ports::RisksQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskView {
    pub id: RiskId,
    pub project_id: ProjectId,
    pub project_code: String,
    /// `R-001`, … (ADR-0001).
    pub reference: String,
    pub description: String,
    pub likelihood: u8,
    pub impact: u8,
    /// Likelihood × impact, calculated, never stored (FR-41).
    pub score: u8,
    pub closed: bool,
    pub version: u32,
}

/// Every field of a stored Risk, for [`Risk::restore`].
#[derive(Debug, Clone)]
pub struct RiskParts {
    pub id: RiskId,
    pub project_id: ProjectId,
    pub number: u32,
    pub description: Description,
    pub likelihood: Rating,
    pub impact: Rating,
    pub closed: bool,
    pub version: u32,
}

/// The Risk aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Risk {
    id: RiskId,
    project_id: ProjectId,
    /// The number in its Reference.
    number: u32,
    description: Description,
    likelihood: Rating,
    impact: Rating,
    closed: bool,
    version: u32,
}

impl Risk {
    /// A new, open Risk with Reference number `number` in its Project.
    pub fn create(
        project_id: ProjectId,
        number: u32,
        description: Description,
        likelihood: Rating,
        impact: Rating,
    ) -> Self {
        Self {
            id: RiskId::new(),
            project_id,
            number,
            description,
            likelihood,
            impact,
            closed: false,
            version: 1,
        }
    }

    /// Rebuilds a stored Risk. For repository adapters only.
    pub fn restore(parts: RiskParts) -> Self {
        Self {
            id: parts.id,
            project_id: parts.project_id,
            number: parts.number,
            description: parts.description,
            likelihood: parts.likelihood,
            impact: parts.impact,
            closed: parts.closed,
            version: parts.version,
        }
    }

    pub fn edited(&self, description: Description, likelihood: Rating, impact: Rating) -> Self {
        Self {
            description,
            likelihood,
            impact,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn with_closed(&self, closed: bool) -> Self {
        Self {
            closed,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn id(&self) -> RiskId {
        self.id
    }

    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn number(&self) -> u32 {
        self.number
    }

    pub fn description(&self) -> &Description {
        &self.description
    }

    pub fn likelihood(&self) -> Rating {
        self.likelihood
    }

    pub fn impact(&self) -> Rating {
        self.impact
    }

    /// Likelihood × impact (FR-41).
    pub fn score(&self) -> u8 {
        self.likelihood.0 * self.impact.0
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}
