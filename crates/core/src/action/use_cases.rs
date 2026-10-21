//! Action use cases. Each change runs in one unit of work, behind the
//! archived-Project guard.

use chrono::NaiveDate;

use crate::action::{Action, ActionId, ActionView};
use crate::history::ChangeContext;
use crate::person::PersonId;
use crate::ports::{
    ActionRepository, ActionsQuery, Page, PageRequest, ReferenceCounter, ReferenceKind, RepoError,
    Transaction, UnitOfWork,
};
use crate::project::guard::{GuardError, MeetingInProgress, unlocked_project};
use crate::project::{Description, DescriptionError, ProjectId};
use crate::session::Actor;

pub use crate::ports::{ActionFilter, ActionSort};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    InvalidDescription(#[from] DescriptionError),
    #[error("the assignee must be a member of the project")]
    AssigneeNotAMember,
    #[error("that project does not exist")]
    UnknownProject,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error("{0}")]
    Locked(MeetingInProgress),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<GuardError> for ActionError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Locked(m) => Self::Locked(m),
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a new Action is created from.
#[derive(Debug, Clone, Copy)]
pub struct NewAction<'a> {
    pub project_id: ProjectId,
    pub description: &'a str,
    /// Must be a Member.
    pub assignee: Option<PersonId>,
    /// Dates in the past are allowed (FR-30).
    pub due_date: Option<NaiveDate>,
}

/// Creates an open Action with the next Reference in its Project (FR-31, FR-32).
pub async fn create_action<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    input: NewAction<'_>,
) -> Result<Action, ActionError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let description = Description::new(input.description)?;
    let mut tx = uow.begin().await?;
    let project = match unlocked_project(&mut tx, input.project_id, &actor).await {
        Err(GuardError::Repo(RepoError::NotFound)) => return Err(ActionError::UnknownProject),
        other => other?,
    };
    if input
        .assignee
        .is_some_and(|a| !project.members().contains(&a))
    {
        return Err(ActionError::AssigneeNotAMember);
    }
    let number = tx
        .references()
        .next(project.id().as_uuid(), ReferenceKind::Action)
        .await?;
    let action = Action::create(
        project.id(),
        number,
        description,
        input.assignee,
        input.due_date,
    );
    tx.actions()
        .save(None, &action, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(action)
}

/// Every writable field of an Action, as given to [`update_action`].
#[derive(Debug, Clone, Copy)]
pub struct ActionEdit<'a> {
    pub description: &'a str,
    /// A changed assignee must be a Member; an unchanged one may since have
    /// been removed (FR-22).
    pub assignee: Option<PersonId>,
    pub due_date: Option<NaiveDate>,
}

/// Replaces an Action's description, assignee and due date (FR-31).
/// Sending what is already stored changes nothing.
pub async fn update_action<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    id: ActionId,
    expected_version: u32,
    edit: ActionEdit<'_>,
) -> Result<Action, ActionError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let description = Description::new(edit.description)?;
    let mut tx = uow.begin().await?;
    let before = tx.actions().load(id).await?;
    let project = unlocked_project(&mut tx, before.project_id(), &actor).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if edit.assignee != before.assignee()
        && edit
            .assignee
            .is_some_and(|a| !project.members().contains(&a))
    {
        return Err(ActionError::AssigneeNotAMember);
    }
    if before.description() == &description
        && before.assignee() == edit.assignee
        && before.due_date() == edit.due_date
    {
        return Ok(before);
    }
    let after = before.edited(description, edit.assignee, edit.due_date);
    tx.actions()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Closes or reopens an Action (FR-31). Setting the state it already has
/// changes nothing. Reopening an Action whose assignee is no longer a Member
/// leaves it unassigned, as removal would have (FR-21).
pub async fn set_action_closed<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    id: ActionId,
    expected_version: u32,
    closed: bool,
) -> Result<Action, ActionError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let mut tx = uow.begin().await?;
    let before = tx.actions().load(id).await?;
    let project = unlocked_project(&mut tx, before.project_id(), &actor).await?;
    if before.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if before.closed() == closed {
        return Ok(before);
    }
    let after = if closed {
        before.closed_now()
    } else {
        before.reopened(|a| project.members().contains(&a))
    };
    tx.actions()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Every open Action in `project` assigned to `person` becomes unassigned,
/// each recorded with `ctx` (FR-21). Called when a Member is removed.
pub(crate) async fn unassign_open_actions<T: Transaction>(
    tx: &mut T,
    project: ProjectId,
    person: PersonId,
    ctx: &ChangeContext,
) -> Result<(), RepoError> {
    let actions = tx.actions().open_assigned_to(project, person).await?;
    for before in actions {
        tx.actions()
            .save(Some(&before), &before.unassigned(), ctx)
            .await?;
    }
    Ok(())
}

pub async fn list_actions<Q: ActionsQuery>(
    query: &Q,
    filter: ActionFilter,
    page: PageRequest,
    sort: ActionSort,
) -> Result<Page<ActionView>, ActionError> {
    Ok(query.list_actions(filter, page, sort).await?)
}

pub async fn get_action<Q: ActionsQuery>(
    query: &Q,
    id: ActionId,
) -> Result<ActionView, ActionError> {
    Ok(query.get_action(id).await?)
}
