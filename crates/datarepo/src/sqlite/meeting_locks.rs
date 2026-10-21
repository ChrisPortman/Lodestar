//! The Meeting Lock repository (ADR-0006). Every change is conditional on
//! the stored row still matching what the use case loaded.

use chrono::{DateTime, SecondsFormat, Utc};
use lodestar_core::meeting_lock::{MeetingLock, MeetingLockParts};
use lodestar_core::person::PersonId;
use lodestar_core::ports::{MeetingLockRepository, RepoError};
use lodestar_core::project::ProjectId;
use lodestar_core::session::SessionId;
use sqlx::Sqlite;

use super::{corrupt, map_constraint, unexpected, uuid};

pub(super) struct SqliteMeetingLocks<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

fn stored_time(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn timestamp(s: &str) -> Result<DateTime<Utc>, RepoError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| corrupt("timestamp", s))
}

impl MeetingLockRepository for SqliteMeetingLocks<'_> {
    async fn load(&mut self, project: ProjectId) -> Result<Option<MeetingLock>, RepoError> {
        let project_text = project.to_string();
        let Some(row) = sqlx::query!(
            "SELECT session_id, leader_id, acquired_at, heartbeat_at
             FROM meeting_locks WHERE project_id = ?",
            project_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        else {
            return Ok(None);
        };
        Ok(Some(MeetingLock::restore(MeetingLockParts {
            project_id: project,
            session: SessionId::new(row.session_id),
            leader: PersonId::from_uuid(uuid(&row.leader_id)?),
            acquired_at: timestamp(&row.acquired_at)?,
            heartbeat_at: timestamp(&row.heartbeat_at)?,
        })))
    }

    async fn save(
        &mut self,
        before: Option<&MeetingLock>,
        after: &MeetingLock,
    ) -> Result<(), RepoError> {
        let project = after.project_id().to_string();
        let session = after.session().as_str();
        let leader = after.leader().to_string();
        let acquired_at = stored_time(after.acquired_at());
        let heartbeat_at = stored_time(after.heartbeat_at());
        let changed = match before {
            None => sqlx::query!(
                "INSERT INTO meeting_locks
                    (project_id, session_id, leader_id, acquired_at, heartbeat_at)
                 VALUES (?, ?, ?, ?, ?)",
                project,
                session,
                leader,
                acquired_at,
                heartbeat_at
            )
            .execute(&mut **self.0)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::Conflict,
                e => map_constraint(e),
            })?,
            Some(before) => {
                let old_session = before.session().as_str();
                let old_heartbeat = stored_time(before.heartbeat_at());
                sqlx::query!(
                    "UPDATE meeting_locks
                     SET session_id = ?, leader_id = ?, acquired_at = ?, heartbeat_at = ?
                     WHERE project_id = ? AND session_id = ? AND heartbeat_at = ?",
                    session,
                    leader,
                    acquired_at,
                    heartbeat_at,
                    project,
                    old_session,
                    old_heartbeat
                )
                .execute(&mut **self.0)
                .await
                .map_err(map_constraint)?
            }
        };
        if changed.rows_affected() != 1 {
            return Err(RepoError::Conflict);
        }
        Ok(())
    }

    async fn delete(&mut self, lock: &MeetingLock) -> Result<(), RepoError> {
        let project = lock.project_id().to_string();
        let session = lock.session().as_str();
        let heartbeat_at = stored_time(lock.heartbeat_at());
        let deleted = sqlx::query!(
            "DELETE FROM meeting_locks
             WHERE project_id = ? AND session_id = ? AND heartbeat_at = ?",
            project,
            session,
            heartbeat_at
        )
        .execute(&mut **self.0)
        .await
        .map_err(unexpected)?;
        if deleted.rows_affected() != 1 {
            return Err(RepoError::Conflict);
        }
        Ok(())
    }
}
