//! The archived-Project guard (FR-14, ADR-0003). Every use case that changes
//! a Project or anything in it loads the Project through [`writable_project`]
//! in its own transaction, so an archived Project is entirely read-only.

use crate::ports::{ProjectRepository, RepoError, Transaction};
use crate::project::{Project, ProjectId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// Loads a Project that may be changed: [`GuardError::Archived`] if it is archived.
pub async fn writable_project<T: Transaction>(
    tx: &mut T,
    id: ProjectId,
) -> Result<Project, GuardError> {
    let project = tx.projects().load(id).await?;
    if project.archived() {
        return Err(GuardError::Archived);
    }
    Ok(project)
}
