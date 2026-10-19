//! Blocker use cases. Each change runs in one unit of work, behind the
//! archived-Project guard.

use crate::action::ActionId;
use crate::blocker::{Blocker, BlockerId, BlockerView};
use crate::history::ChangeContext;
use crate::person::PersonId;
use crate::ports::{
    ActionRepository, BlockerRepository, BlockersQuery, Page, PageRequest, ReferenceCounter,
    ReferenceKind, RepoError, Transaction, UnitOfWork,
};
use crate::project::guard::{GuardError, writable_project};
use crate::project::{Description, DescriptionError, ProjectId};

pub use crate::ports::BlockerFilter;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockerError {
    #[error(transparent)]
    InvalidDescription(#[from] DescriptionError),
    #[error("the accountable person must be a member of the project")]
    AccountableNotAMember,
    #[error("that project does not exist")]
    UnknownProject,
    #[error("that action does not exist")]
    UnknownAction,
    #[error("a blocker can only block actions in its own project")]
    ActionInAnotherProject,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<GuardError> for BlockerError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a new Blocker is created from.
#[derive(Debug, Clone, Copy)]
pub struct NewBlocker<'a> {
    pub project_id: ProjectId,
    pub description: &'a str,
    /// Must be a Member.
    pub accountable: Option<PersonId>,
}

/// Creates an open Blocker with the next Reference in its Project (FR-51, FR-52).
pub async fn create_blocker<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    input: NewBlocker<'_>,
) -> Result<Blocker, BlockerError> {
    let description = Description::new(input.description)?;
    let mut tx = uow.begin().await?;
    let project = match writable_project(&mut tx, input.project_id).await {
        Err(GuardError::Repo(RepoError::NotFound)) => return Err(BlockerError::UnknownProject),
        other => other?,
    };
    if input
        .accountable
        .is_some_and(|a| !project.members().contains(&a))
    {
        return Err(BlockerError::AccountableNotAMember);
    }
    let number = tx
        .references()
        .next(project.id().as_uuid(), ReferenceKind::Blocker)
        .await?;
    let blocker = Blocker::create(project.id(), number, description, input.accountable);
    tx.blockers()
        .save(None, &blocker, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(blocker)
}

/// Every writable field of a Blocker, as given to [`update_blocker`].
#[derive(Debug, Clone, Copy)]
pub struct BlockerEdit<'a> {
    pub description: &'a str,
    /// A changed accountable Person must be a Member; an unchanged one may
    /// since have been removed (FR-22).
    pub accountable: Option<PersonId>,
}

/// Replaces a Blocker's description and accountable Member (FR-51).
/// Sending what is already stored changes nothing.
pub async fn update_blocker<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: BlockerId,
    expected_version: u32,
    edit: BlockerEdit<'_>,
) -> Result<Blocker, BlockerError> {
    let description = Description::new(edit.description)?;
    let mut tx = uow.begin().await?;
    let before = tx.blockers().load(id).await?;
    let project = writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if edit.accountable != before.accountable()
        && edit
            .accountable
            .is_some_and(|a| !project.members().contains(&a))
    {
        return Err(BlockerError::AccountableNotAMember);
    }
    if before.description() == &description && before.accountable() == edit.accountable {
        return Ok(before);
    }
    let after = before.edited(description, edit.accountable);
    tx.blockers()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Clears or reopens a Blocker (FR-51). Setting the state it already has
/// changes nothing. Reopening a Blocker whose accountable Person is no
/// longer a Member leaves it unassigned, as removal would have (FR-21).
pub async fn set_blocker_cleared<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: BlockerId,
    expected_version: u32,
    cleared: bool,
) -> Result<Blocker, BlockerError> {
    let mut tx = uow.begin().await?;
    let before = tx.blockers().load(id).await?;
    let project = writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if before.cleared() == cleared {
        return Ok(before);
    }
    let after = if cleared {
        before.cleared_now()
    } else {
        before.reopened(|a| project.members().contains(&a))
    };
    tx.blockers()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Links a Blocker to an Action in the same Project that it blocks, whatever
/// state either is in (FR-51, FR-54). Linking an already linked Action
/// changes nothing.
pub async fn link_blocked_action<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: BlockerId,
    expected_version: u32,
    action: ActionId,
) -> Result<Blocker, BlockerError> {
    let mut tx = uow.begin().await?;
    let before = tx.blockers().load(id).await?;
    writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    let target = match tx.actions().load(action).await {
        Err(RepoError::NotFound) => return Err(BlockerError::UnknownAction),
        other => other?,
    };
    if target.project_id() != before.project_id() {
        return Err(BlockerError::ActionInAnotherProject);
    }
    if before.blocked_actions().contains(&action) {
        return Ok(before);
    }
    let after = before.linked_to(action);
    tx.blockers()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Removes a Blocker's link to an Action (FR-51, FR-54). Unlinking an
/// Action that is not linked changes nothing.
pub async fn unlink_blocked_action<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    id: BlockerId,
    expected_version: u32,
    action: ActionId,
) -> Result<Blocker, BlockerError> {
    let mut tx = uow.begin().await?;
    let before = tx.blockers().load(id).await?;
    writable_project(&mut tx, before.project_id()).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if !before.blocked_actions().contains(&action) {
        return Ok(before);
    }
    let after = before.unlinked_from(action);
    tx.blockers()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Every open Blocker in `project` that `person` is accountable for loses
/// its accountable Member, each recorded with `ctx` (FR-21). Called when a
/// Member is removed.
pub(crate) async fn unassign_open_blockers<T: Transaction>(
    tx: &mut T,
    project: ProjectId,
    person: PersonId,
    ctx: &ChangeContext,
) -> Result<(), RepoError> {
    let blockers = tx.blockers().open_accountable(project, person).await?;
    for before in blockers {
        tx.blockers()
            .save(Some(&before), &before.unassigned(), ctx)
            .await?;
    }
    Ok(())
}

pub async fn list_blockers<Q: BlockersQuery>(
    query: &Q,
    filter: BlockerFilter,
    page: PageRequest,
) -> Result<Page<BlockerView>, BlockerError> {
    Ok(query.list_blockers(filter, page).await?)
}

pub async fn get_blocker<Q: BlockersQuery>(
    query: &Q,
    id: BlockerId,
) -> Result<BlockerView, BlockerError> {
    Ok(query.get_blocker(id).await?)
}
