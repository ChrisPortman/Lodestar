//! Comment use cases. Each change runs in one unit of work, behind the
//! archived-Project guard, and is open only to current Members (FR-120 to
//! FR-125). Nothing here saves the Comment's target.

use crate::action::ActionId;
use crate::blocker::BlockerId;
use crate::comment::{BodyError, Comment, CommentBody, CommentId, CommentView, Target, TargetType};
use crate::history::ChangeContext;
use crate::meeting::MeetingId;
use crate::person::PersonId;
use crate::ports::{
    ActionRepository, BlockerRepository, CommentRepository, CommentsQuery, MeetingRepository, Page,
    PageRequest, RepoError, RiskRepository, Transaction, UnitOfWork,
};
use crate::project::guard::{GuardError, MeetingInProgress, unlocked_project};
use crate::project::{Project, ProjectId};
use crate::risk::RiskId;
use crate::session::Actor;

pub use crate::ports::CommentFilter;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommentError {
    #[error(transparent)]
    InvalidBody(#[from] BodyError),
    #[error("the project does not exist")]
    UnknownProject,
    #[error("the comment's target does not exist in this project")]
    UnknownTarget,
    #[error("only members of the project can comment")]
    NotAMember,
    #[error("only the author can change a comment")]
    NotTheAuthor,
    #[error("the comment has been deleted")]
    Deleted,
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error("{0}")]
    Locked(MeetingInProgress),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl From<GuardError> for CommentError {
    fn from(error: GuardError) -> Self {
        match error {
            GuardError::Archived => Self::Archived,
            GuardError::Locked(m) => Self::Locked(m),
            GuardError::Repo(e) => Self::Repo(e),
        }
    }
}

/// What a new Comment is created from.
#[derive(Debug, Clone, Copy)]
pub struct NewComment<'a> {
    pub project_id: ProjectId,
    pub target: Target,
    pub body: &'a str,
}

/// Fails unless `target` exists and belongs to `project`.
async fn check_target<T: Transaction>(
    tx: &mut T,
    project: ProjectId,
    target: Target,
) -> Result<(), CommentError> {
    let owner = match target.kind {
        TargetType::Project => Ok(ProjectId::from_uuid(target.id)),
        TargetType::Action => tx
            .actions()
            .load(ActionId::from_uuid(target.id))
            .await
            .map(|a| a.project_id()),
        TargetType::Risk => tx
            .risks()
            .load(RiskId::from_uuid(target.id))
            .await
            .map(|r| r.project_id()),
        TargetType::Blocker => tx
            .blockers()
            .load(BlockerId::from_uuid(target.id))
            .await
            .map(|b| b.project_id()),
        TargetType::Meeting => tx
            .meetings()
            .load(MeetingId::from_uuid(target.id))
            .await
            .map(|m| m.project_id()),
    };
    match owner {
        Ok(owner) if owner == project => Ok(()),
        Ok(_) | Err(RepoError::NotFound) => Err(CommentError::UnknownTarget),
        Err(e) => Err(e.into()),
    }
}

fn require_member(project: &Project, acting: PersonId) -> Result<(), CommentError> {
    if project.members().contains(&acting) {
        Ok(())
    } else {
        Err(CommentError::NotAMember)
    }
}

/// Adds a Comment by the Acting Person to a Project or an item in it, open
/// or closed (FR-120).
pub async fn create_comment<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    input: NewComment<'_>,
) -> Result<Comment, CommentError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let body = CommentBody::new(input.body)?;
    let mut tx = uow.begin().await?;
    let project = match unlocked_project(&mut tx, input.project_id, &actor).await {
        Err(GuardError::Repo(RepoError::NotFound)) => return Err(CommentError::UnknownProject),
        other => other?,
    };
    require_member(&project, acting)?;
    check_target(&mut tx, input.project_id, input.target).await?;
    let comment = Comment::create(input.project_id, input.target, acting, body);
    tx.comments()
        .save(None, &comment, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(comment)
}

/// Loads a Comment its author may change: the Project is writable, the
/// version matches, and the Acting Person is its author and still a Member.
async fn own_comment<T: Transaction>(
    tx: &mut T,
    actor: &Actor,
    id: CommentId,
    expected_version: u32,
) -> Result<Comment, CommentError> {
    let acting = actor.person;
    let comment = tx.comments().load(id).await?;
    let project = unlocked_project(tx, comment.project_id(), actor).await?;
    if comment.version() != expected_version {
        return Err(RepoError::Conflict.into());
    }
    if comment.author() != acting {
        return Err(CommentError::NotTheAuthor);
    }
    require_member(&project, acting)?;
    Ok(comment)
}

/// Replaces a Comment's text and marks it edited (FR-122).
pub async fn edit_comment<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    id: CommentId,
    expected_version: u32,
    body: &str,
) -> Result<Comment, CommentError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let body = CommentBody::new(body)?;
    let mut tx = uow.begin().await?;
    let before = own_comment(&mut tx, &actor, id, expected_version).await?;
    if before.is_deleted() {
        return Err(CommentError::Deleted);
    }
    let after = before.edited(body);
    tx.comments()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

/// Deletes a Comment for good, keeping its text for the export (FR-123).
/// Deleting it again changes nothing.
pub async fn delete_comment<U: UnitOfWork>(
    uow: &U,
    actor: impl Into<Actor>,
    id: CommentId,
    expected_version: u32,
) -> Result<Comment, CommentError> {
    let actor: Actor = actor.into();
    let acting = actor.person;
    let mut tx = uow.begin().await?;
    let before = own_comment(&mut tx, &actor, id, expected_version).await?;
    if before.is_deleted() {
        return Ok(before);
    }
    let after = before.deleted();
    tx.comments()
        .save(Some(&before), &after, &ChangeContext::by(acting))
        .await?;
    tx.commit().await?;
    Ok(after)
}

pub async fn list_comments<Q: CommentsQuery>(
    query: &Q,
    filter: CommentFilter,
    page: PageRequest,
) -> Result<Page<CommentView>, CommentError> {
    Ok(query.list_comments(filter, page).await?)
}

pub async fn get_comment<Q: CommentsQuery>(
    query: &Q,
    id: CommentId,
) -> Result<CommentView, CommentError> {
    Ok(query.get_comment(id).await?)
}
