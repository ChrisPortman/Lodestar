//! Meeting Lock use cases. Each runs in one unit of work. The time is passed
//! in, so callers (and tests) own the clock.

use chrono::{DateTime, Utc};

use crate::meeting_lock::MeetingLock;
use crate::person::PersonId;
use crate::ports::{
    MeetingLockRepository, PersonRepository, ProjectRepository, RepoError, Transaction, UnitOfWork,
};
use crate::project::{PersonRef, Project, ProjectId};
use crate::session::SessionId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    #[error("the project does not exist")]
    UnknownProject,
    #[error("only the project leader can start a meeting")]
    NotTheLeader,
    #[error("only members of the project can see its meeting lock")]
    NotAMember,
    #[error("another session holds the meeting lock")]
    Held,
    #[error("this session does not hold the meeting lock")]
    NotHeld,
    #[error("the project has no meeting lock")]
    NoLock,
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// What any Member may see of a lock. It never names the holding session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockStatus {
    pub leader: PersonRef,
    pub acquired_at: DateTime<Utc>,
    pub live: bool,
    pub held_by_you: bool,
}

async fn project_of<T: Transaction>(tx: &mut T, id: ProjectId) -> Result<Project, LockError> {
    match tx.projects().load(id).await {
        Err(RepoError::NotFound) => Err(LockError::UnknownProject),
        other => Ok(other?),
    }
}

/// Acquires the lock for `session` of the Project Leader. A stale lock is
/// taken over; the session's own lock is reclaimed, keeping its
/// `acquired_at`; another session's live lock is [`LockError::Held`].
pub async fn acquire_lock<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    session: &SessionId,
    project_id: ProjectId,
    now: DateTime<Utc>,
) -> Result<MeetingLock, LockError> {
    let mut tx = uow.begin().await?;
    let project = project_of(&mut tx, project_id).await?;
    if project.project_leader() != Some(acting) {
        return Err(LockError::NotTheLeader);
    }
    let before = tx.meeting_locks().load(project_id).await?;
    let after = match &before {
        Some(lock) if lock.is_held_by(session) => lock.heartbeat(now),
        Some(lock) if lock.is_live(now) => return Err(LockError::Held),
        _ => MeetingLock::acquire(project_id, session.clone(), acting, now),
    };
    tx.meeting_locks().save(before.as_ref(), &after).await?;
    tx.commit().await?;
    Ok(after)
}

/// Loads the lock `session` holds, live or its own stale one.
async fn held_lock<T: Transaction>(
    tx: &mut T,
    session: &SessionId,
    project_id: ProjectId,
) -> Result<MeetingLock, LockError> {
    match tx.meeting_locks().load(project_id).await? {
        Some(lock) if lock.is_held_by(session) => Ok(lock),
        _ => Err(LockError::NotHeld),
    }
}

/// Keeps the lock `session` holds live.
pub async fn heartbeat_lock<U: UnitOfWork>(
    uow: &U,
    session: &SessionId,
    project_id: ProjectId,
    now: DateTime<Utc>,
) -> Result<MeetingLock, LockError> {
    let mut tx = uow.begin().await?;
    let before = held_lock(&mut tx, session, project_id).await?;
    let after = before.heartbeat(now);
    tx.meeting_locks().save(Some(&before), &after).await?;
    tx.commit().await?;
    Ok(after)
}

/// Releases the lock `session` holds, deleting it.
pub async fn cancel_lock<U: UnitOfWork>(
    uow: &U,
    session: &SessionId,
    project_id: ProjectId,
) -> Result<(), LockError> {
    let mut tx = uow.begin().await?;
    let lock = held_lock(&mut tx, session, project_id).await?;
    tx.meeting_locks().delete(&lock).await?;
    tx.commit().await?;
    Ok(())
}

/// The Project's lock as a Member sees it from `session`.
pub async fn lock_status<U: UnitOfWork>(
    uow: &U,
    acting: PersonId,
    session: &SessionId,
    project_id: ProjectId,
    now: DateTime<Utc>,
) -> Result<LockStatus, LockError> {
    let mut tx = uow.begin().await?;
    let project = project_of(&mut tx, project_id).await?;
    if !project.members().contains(&acting) {
        return Err(LockError::NotAMember);
    }
    let lock = tx
        .meeting_locks()
        .load(project_id)
        .await?
        .ok_or(LockError::NoLock)?;
    let leader = tx.people().load(lock.leader()).await?;
    Ok(LockStatus {
        leader: PersonRef {
            id: leader.id(),
            display_name: leader.display_name().as_str().to_owned(),
        },
        acquired_at: lock.acquired_at(),
        live: lock.is_live(now),
        held_by_you: lock.is_held_by(session),
    })
}
