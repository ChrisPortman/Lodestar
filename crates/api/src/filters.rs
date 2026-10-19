//! List filter parameters shared by the Action, Blocker and Risk lists
//! (FR-92, FR-23).

use lodestar_core::{
    person::PersonId,
    ports::{AssigneeFilter, ClearedFilter, ClosedFilter},
};
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
#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateParam {
    #[default]
    Open,
    Closed,
    All,
}

impl From<StateParam> for ClosedFilter {
    fn from(state: StateParam) -> Self {
        match state {
            StateParam::Open => Self::Open,
            StateParam::Closed => Self::Closed,
            StateParam::All => Self::All,
        }
    }
}

/// Which Blockers to list by state.
#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockerStateParam {
    #[default]
    Open,
    Cleared,
    All,
}

impl From<BlockerStateParam> for ClearedFilter {
    fn from(state: BlockerStateParam) -> Self {
        match state {
            BlockerStateParam::Open => Self::Open,
            BlockerStateParam::Cleared => Self::Cleared,
            BlockerStateParam::All => Self::All,
        }
    }
}
