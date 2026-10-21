//! The Project write guards. Every use case that changes a Project or
//! anything in it loads the Project through one of them in its own
//! transaction:
//! - [`writable_project`]: an archived Project is entirely read-only (FR-14,
//!   ADR-0003);
//! - [`unlocked_project`]: also, while a Meeting Lock is live, only the
//!   session holding it may write (ADR-0006).

use chrono::{DateTime, Utc};

use crate::meeting_lock::MeetingLock;
use crate::ports::{
    MeetingLockRepository, PersonRepository, ProjectRepository, RepoError, Transaction,
};
use crate::project::{PersonRef, Project, ProjectId};
use crate::session::Actor;

/// A live Meeting Lock, as told to a session it refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingInProgress {
    pub leader: PersonRef,
    pub since: DateTime<Utc>,
}

impl std::fmt::Display for MeetingInProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a meeting led by {} has been in progress since {}; the project is read-only until it ends",
            self.leader.display_name,
            self.since
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    #[error("the project is archived; unarchive it first")]
    Archived,
    #[error("{0}")]
    Locked(MeetingInProgress),
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

/// As [`writable_project`], and [`GuardError::Locked`] unless `actor`'s
/// session holds the Project's live Meeting Lock, if there is one.
pub async fn unlocked_project<T: Transaction>(
    tx: &mut T,
    id: ProjectId,
    actor: &Actor,
) -> Result<Project, GuardError> {
    let project = writable_project(tx, id).await?;
    if let Some(lock) = live_lock(tx, id, actor.now).await?
        && actor.session.as_ref().is_none_or(|s| !lock.is_held_by(s))
    {
        return Err(locked(tx, &lock).await);
    }
    Ok(project)
}

/// [`GuardError::Locked`] if the Project has a live Meeting Lock, whoever
/// holds it.
pub async fn no_live_lock<T: Transaction>(
    tx: &mut T,
    id: ProjectId,
    now: DateTime<Utc>,
) -> Result<(), GuardError> {
    match live_lock(tx, id, now).await? {
        Some(lock) => Err(locked(tx, &lock).await),
        None => Ok(()),
    }
}

async fn live_lock<T: Transaction>(
    tx: &mut T,
    id: ProjectId,
    now: DateTime<Utc>,
) -> Result<Option<MeetingLock>, RepoError> {
    Ok(tx
        .meeting_locks()
        .load(id)
        .await?
        .filter(|lock| lock.is_live(now)))
}

async fn locked<T: Transaction>(tx: &mut T, lock: &MeetingLock) -> GuardError {
    match tx.people().load(lock.leader()).await {
        Ok(leader) => GuardError::Locked(MeetingInProgress {
            leader: PersonRef {
                id: leader.id(),
                display_name: leader.display_name().as_str().to_owned(),
            },
            since: lock.acquired_at(),
        }),
        Err(e) => e.into(),
    }
}
