//! Projects (FR-10 to FR-18, ADR-0003).

mod code;
pub mod guard;
pub mod members;
pub mod use_cases;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::person::PersonId;

pub use code::{MAX_CODE_CHARS, ProjectCode, ProjectCodeError, suggest_code};

/// Identifies a [`Project`]. Always a UUIDv7 for new Projects (NFR-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(Uuid);

impl ProjectId {
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

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_DESCRIPTION_CHARS: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectNameError {
    #[error("name is required")]
    Empty,
    #[error("name must be at most {MAX_NAME_CHARS} characters")]
    TooLong,
}

/// A Project's name: trimmed, non-empty, at most 100 characters (FR-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectName(String);

impl ProjectName {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, ProjectNameError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(ProjectNameError::Empty);
        }
        if trimmed.chars().count() > MAX_NAME_CHARS {
            return Err(ProjectNameError::TooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ProjectName {
    type Error = ProjectNameError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ProjectName> for String {
    fn from(value: ProjectName) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptionError {
    #[error("description must not be blank")]
    Empty,
    #[error("description must be at most {MAX_DESCRIPTION_CHARS} characters")]
    TooLong,
}

/// A description: trimmed, non-empty, at most 4,000 characters (FR-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Description(String);

impl Description {
    pub fn new(raw: impl AsRef<str>) -> Result<Self, DescriptionError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(DescriptionError::Empty);
        }
        if trimmed.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(DescriptionError::TooLong);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// `None` stays `None`; a given description must be valid.
    pub fn optional(raw: Option<&str>) -> Result<Option<Self>, DescriptionError> {
        raw.map(Self::new).transpose()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Description {
    type Error = DescriptionError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Description> for String {
    fn from(value: Description) -> Self {
        value.0
    }
}

/// Red, Amber or Green (FR-70).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rag {
    Red,
    Amber,
    Green,
}

impl Rag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Amber => "amber",
            Self::Green => "green",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "red" => Some(Self::Red),
            "amber" => Some(Self::Amber),
            "green" => Some(Self::Green),
            _ => None,
        }
    }
}

/// A Project's Health Rating, with an optional comment (FR-70).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthRating {
    pub rating: Rag,
    pub comment: Option<Description>,
}

/// Only a Member can be the Project Leader (FR-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the project leader must be a member")]
pub struct LeaderNotAMember;

/// A Person as shown alongside something that refers to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRef {
    pub id: PersonId,
    pub display_name: String,
}

/// A read-only view of a Project, served by [`crate::ports::ProjectsQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub description: Option<String>,
    pub project_code: String,
    pub archived: bool,
    pub project_leader: Option<PersonRef>,
    pub version: u32,
}

/// Every field of a stored Project, for [`Project::restore`].
#[derive(Debug, Clone)]
pub struct ProjectParts {
    pub id: ProjectId,
    pub name: ProjectName,
    pub description: Option<Description>,
    pub project_code: ProjectCode,
    pub archived: bool,
    pub project_leader: Option<PersonId>,
    pub members: BTreeSet<PersonId>,
    pub health_rating: Option<HealthRating>,
    pub version: u32,
}

/// The Project aggregate. Each change produces a new value with the next version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    id: ProjectId,
    name: ProjectName,
    description: Option<Description>,
    project_code: ProjectCode,
    archived: bool,
    project_leader: Option<PersonId>,
    members: BTreeSet<PersonId>,
    health_rating: Option<HealthRating>,
    version: u32,
}

impl Project {
    /// A new Project, with its creator as its only Member and its Project Leader (FR-12).
    pub fn create(
        name: ProjectName,
        description: Option<Description>,
        project_code: ProjectCode,
        creator: PersonId,
    ) -> Self {
        Self {
            id: ProjectId::new(),
            name,
            description,
            project_code,
            archived: false,
            project_leader: Some(creator),
            members: BTreeSet::from([creator]),
            health_rating: None,
            version: 1,
        }
    }

    /// Rebuilds a stored Project. For repository adapters only.
    pub fn restore(parts: ProjectParts) -> Self {
        Self {
            id: parts.id,
            name: parts.name,
            description: parts.description,
            project_code: parts.project_code,
            archived: parts.archived,
            project_leader: parts.project_leader,
            members: parts.members,
            health_rating: parts.health_rating,
            version: parts.version,
        }
    }

    /// The Project with a new name, description and Project Leader, who must
    /// be a Member (FR-13). The code never changes (FR-11).
    pub fn edited(
        &self,
        name: ProjectName,
        description: Option<Description>,
        project_leader: Option<PersonId>,
    ) -> Result<Self, LeaderNotAMember> {
        if project_leader.is_some_and(|l| !self.members.contains(&l)) {
            return Err(LeaderNotAMember);
        }
        Ok(Self {
            name,
            description,
            project_leader,
            version: self.version + 1,
            ..self.clone()
        })
    }

    /// The Project with `person` as a Member (FR-20).
    pub fn with_member(&self, person: PersonId) -> Self {
        let mut members = self.members.clone();
        members.insert(person);
        Self {
            members,
            version: self.version + 1,
            ..self.clone()
        }
    }

    /// The Project without `person` as a Member. Removing the Project Leader
    /// leaves the Project with no Project Leader (FR-13, FR-21).
    pub fn without_member(&self, person: PersonId) -> Self {
        let mut members = self.members.clone();
        members.remove(&person);
        Self {
            members,
            project_leader: self.project_leader.filter(|l| *l != person),
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn with_archived(&self, archived: bool) -> Self {
        Self {
            archived,
            version: self.version + 1,
            ..self.clone()
        }
    }

    pub fn id(&self) -> ProjectId {
        self.id
    }

    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    pub fn description(&self) -> Option<&Description> {
        self.description.as_ref()
    }

    pub fn project_code(&self) -> &ProjectCode {
        &self.project_code
    }

    pub fn archived(&self) -> bool {
        self.archived
    }

    pub fn project_leader(&self) -> Option<PersonId> {
        self.project_leader
    }

    pub fn members(&self) -> &BTreeSet<PersonId> {
        &self.members
    }

    pub fn health_rating(&self) -> Option<&HealthRating> {
        self.health_rating.as_ref()
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}
