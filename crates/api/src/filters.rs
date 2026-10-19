//! List filter parameters shared by the Action, Blocker and Risk lists
//! (FR-92, FR-23).

use lodestar_core::{person::PersonId, ports::AssigneeFilter};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::problem::Problem;

/// The `assignee` (or `accountable`) query value: a Person ID, or
/// `unassigned`.
pub fn assignee(value: Option<&str>, name: &str) -> Result<Option<AssigneeFilter>, Problem> {
    match value {
        None => Ok(None),
        Some("unassigned") => Ok(Some(AssigneeFilter::Unassigned)),
        Some(id) => Uuid::parse_str(id)
            .map(|id| Some(AssigneeFilter::Person(PersonId::from_uuid(id))))
            .map_err(|_| Problem::invalid(format!("{name} must be a Person ID or `unassigned`"))),
    }
}

/// Which Actions or Risks to list by state.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateParam {
    Open,
    Closed,
    All,
}

impl StateParam {
    /// `(include_closed, closed_only)`, with `include_closed` the legacy flag
    /// used when no state is given.
    pub fn flags(state: Option<Self>, include_closed: Option<bool>) -> (bool, bool) {
        match state {
            None => (include_closed.unwrap_or(false), false),
            Some(Self::Open) => (false, false),
            Some(Self::Closed) => (true, true),
            Some(Self::All) => (true, false),
        }
    }
}

/// Which Blockers to list by state.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockerStateParam {
    Open,
    Cleared,
    All,
}

impl BlockerStateParam {
    /// `(include_cleared, cleared_only)`, as [`StateParam::flags`].
    pub fn flags(state: Option<Self>, include_cleared: Option<bool>) -> (bool, bool) {
        match state {
            None => (include_cleared.unwrap_or(false), false),
            Some(Self::Open) => (false, false),
            Some(Self::Cleared) => (true, true),
            Some(Self::All) => (true, false),
        }
    }
}
