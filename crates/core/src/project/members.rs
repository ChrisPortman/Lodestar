//! Members of a Project (FR-20 to FR-22, FR-83). Members belong to the
//! Project aggregate, so each change bumps the Project's version and writes
//! its Change History entry.

use crate::action::use_cases::unassign_open_actions;
use crate::blocker::use_cases::unassign_open_blockers;
use crate::history::ChangeContext;
use crate::person::PersonId;
use crate::ports::{
    PersonRepository, ProjectRepository, ProjectsQuery, RepoError, Transaction, UnitOfWork,
};
use crate::project::guard::writable_project;
use crate::project::use_cases::ProjectError;
use crate::project::{PersonRef, Project, ProjectId};

/// Adds a Person as a Member (FR-20). Adding an existing Member changes
/// nothing. A Person added back keeps none of their old items (FR-22).
pub async fn add_member<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: ProjectId,
    expected_version: u32,
    person: PersonId,
) -> Result<Project, ProjectError> {
    let mut tx = uow.begin().await?;
    let before = writable_project(&mut tx, id).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    match tx.people().load(person).await {
        Err(RepoError::NotFound) => return Err(ProjectError::UnknownPerson),
        other => other?,
    };
    if before.members().contains(&person) {
        return Ok(before);
    }
    let after = before.with_member(person);
    tx.projects()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Removes a Member (FR-21). Removing the Project Leader leaves no Project
/// Leader (FR-13). Their open items are unassigned in the same transaction,
/// each change caused by the removal's history entry (FR-83, ADR-0004).
pub async fn remove_member<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: ProjectId,
    expected_version: u32,
    person: PersonId,
) -> Result<Project, ProjectError> {
    let mut tx = uow.begin().await?;
    let before = writable_project(&mut tx, id).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if !before.members().contains(&person) {
        return Err(ProjectError::NotAMember);
    }
    let after = before.without_member(person);
    let ctx = ChangeContext::by(acting);
    let entry = tx.projects().save(Some(&before), &after, &ctx).await?;
    unassign_open_items(&mut tx, id, person, &ctx.caused_by(entry)).await?;
    tx.commit().await?;
    Ok(after)
}

/// The cross-aggregate step of removing a Member (FR-21, ADR-0003): every
/// open Action assigned to `person` and every open Blocker they are
/// accountable for becomes unassigned, each as its own versioned change
/// recorded with `ctx` (whose `caused_by` is the removal).
async fn unassign_open_items<T: Transaction>(
    tx: &mut T,
    project: ProjectId,
    person: PersonId,
    ctx: &ChangeContext,
) -> Result<(), RepoError> {
    unassign_open_actions(tx, project, person, ctx).await?;
    unassign_open_blockers(tx, project, person, ctx).await?;
    Ok(())
}

/// A Project's Members, ordered by display name.
pub async fn project_members<Q: ProjectsQuery>(
    query: &Q,
    id: ProjectId,
) -> Result<Vec<PersonRef>, ProjectError> {
    Ok(query.project_members(id).await?)
}
