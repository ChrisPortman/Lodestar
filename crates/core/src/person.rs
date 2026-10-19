//! People: real individuals who exist across the whole system (FR-01, FR-05).

pub mod use_cases;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifies a [`Person`]. Always a UUIDv7 for new people (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PersonId(Uuid);

impl PersonId {
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

impl Default for PersonId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PersonId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub const MAX_DISPLAY_NAME_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DisplayNameError {
    #[error("display name is required")]
    Empty,
    #[error("display name must be at most {MAX_DISPLAY_NAME_CHARS} characters")]
    TooLong,
}

/// A Person's display name: trimmed, non-empty, at most 100 characters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayName(String);

impl DisplayName {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, DisplayNameError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(DisplayNameError::Empty);
        }
        if trimmed.chars().count() > MAX_DISPLAY_NAME_CHARS {
            return Err(DisplayNameError::TooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DisplayName {
    type Error = DisplayNameError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DisplayName> for String {
    fn from(value: DisplayName) -> Self {
        value.0
    }
}

/// A read-only view of a Person, served by [`crate::ports::PeopleQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonView {
    pub id: PersonId,
    pub display_name: String,
    pub version: u32,
}

/// The Person aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    id: PersonId,
    display_name: DisplayName,
    version: u32,
}

impl Person {
    pub fn create(display_name: impl AsRef<str>) -> Result<Self, DisplayNameError> {
        Ok(Self {
            id: PersonId::new(),
            display_name: DisplayName::new(display_name)?,
            version: 1,
        })
    }

    /// Rebuilds a stored Person. For repository adapters only.
    pub fn restore(id: PersonId, display_name: DisplayName, version: u32) -> Self {
        Self {
            id,
            display_name,
            version,
        }
    }

    pub fn renamed(&self, display_name: impl AsRef<str>) -> Result<Self, DisplayNameError> {
        Ok(Self {
            id: self.id,
            display_name: DisplayName::new(display_name)?,
            version: self.version + 1,
        })
    }

    pub fn id(&self) -> PersonId {
        self.id
    }

    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}
