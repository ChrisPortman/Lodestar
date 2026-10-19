//! Risk use cases. Each change runs in one unit of work, behind the
//! archived-Project guard.

use crate::history::ChangeContext;
use crate::person::PersonId;
use crate::ports::{
    Page, PageRequest, ReferenceCounter, ReferenceKind, RepoError, RiskRepository, RisksQuery,
    Transaction, UnitOfWork,
};
use crate::project::guard::{GuardError, writable_project};
use crate::project::{Description, DescriptionError, ProjectId};
use crate::risk::{Rating, RatingError, Risk, RiskEdit, RiskId, RiskView};

pub use crate::ports::{RiskFilter, RiskSort};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RiskError {
    #[error(transparent)]
    InvalidDescription(#[from] DescriptionError),
    #[error(transparent)]
    InvalidRating(#[from] RatingError),
    #[error("the project does not exist")]
    UnknownProject,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<GuardError> for RiskError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a new Risk is created from.
#[derive(Debug, Clone, Copy)]
pub struct NewRisk<'a> {
    pub project_id: ProjectId,
    pub description: &'a str,
    pub likelihood: u8,
    pub impact: u8,
}

fn validate(
    description: &str,
    likelihood: u8,
    impact: u8,
) -> Result<(Description, Rating, Rating), RiskError> {
    Ok((
        Description::new(description)?,
        Rating::new(likelihood)?,
        Rating::new(impact)?,
    ))
}

/// Creates an open Risk with the next Reference in its Project (FR-42, FR-43).
pub async fn create_risk<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    input: NewRisk<'_>,
) -> Result<Risk, RiskError> {
    let (description, likelihood, impact) =
        validate(input.description, input.likelihood, input.impact)?;
    let mut tx = uow.begin().await?;
    match writable_project(&mut tx, input.project_id).await {
        Err(GuardError::Repo(RepoError::NotFound)) => return Err(RiskError::UnknownProject),
        other => other?,
    };
    let number = tx
        .references()
        .next(input.project_id.as_uuid(), ReferenceKind::Risk)
        .await?;
    let risk = Risk::create(input.project_id, number, description, likelihood, impact);
    tx.risks()
        .save(None, &risk, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(risk)
}

/// Replaces a Risk's description, likelihood and impact (FR-42).
pub async fn update_risk<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: RiskId,
    expected_version: u32,
    edit: RiskEdit<'_>,
) -> Result<Risk, RiskError> {
    let (description, likelihood, impact) =
        validate(edit.description, edit.likelihood, edit.impact)?;
    let mut tx = uow.begin().await?;
    let before = tx.risks().load(id).await?;
    writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    let after = before.edited(description, likelihood, impact);
    tx.risks()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Closes or reopens a Risk (FR-42). Setting the state it already has
/// changes nothing.
pub async fn set_closed<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: RiskId,
    expected_version: u32,
    closed: bool,
) -> Result<Risk, RiskError> {
    let mut tx = uow.begin().await?;
    let before = tx.risks().load(id).await?;
    writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if before.closed() == closed {
        return Ok(before);
    }
    let after = before.with_closed(closed);
    tx.risks()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn list_risks<Q: RisksQuery>(
    query: &Q,
    page: PageRequest,
    filter: RiskFilter,
    sort: RiskSort,
) -> Result<Page<RiskView>, RiskError> {
    Ok(query.list_risks(page, filter, sort).await?)
}

pub async fn get_risk<Q: RisksQuery>(query: &Q, id: RiskId) -> Result<RiskView, RiskError> {
    Ok(query.get_risk(id).await?)
}
