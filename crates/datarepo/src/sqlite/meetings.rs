//! The Meeting repository and query (ADR-0006). Meetings are only ever
//! inserted; their Minutes are stored as one JSON document.

use chrono::{DateTime, SecondsFormat, Utc};
use lodestar_core::history::{ChangeContext, ChangeKind, EntityType, HistoryEntryId, snapshot};
use lodestar_core::meeting::{Meeting, MeetingId, MeetingParts, MeetingSummary, MeetingView};
use lodestar_core::minutes::Minutes;
use lodestar_core::person::PersonId;
use lodestar_core::ports::{MeetingRepository, MeetingsQuery, Page, PageRequest, RepoError};
use lodestar_core::project::{PersonRef, ProjectId};
use sqlx::{Sqlite, SqliteConnection};

use super::{
    NewEntry, SqliteStore, corrupt, count, map_constraint, record_history, unexpected, uuid,
    version,
};

pub(super) struct SqliteMeetings<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

fn stored_time(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn timestamp(s: &str) -> Result<DateTime<Utc>, RepoError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| corrupt("timestamp", s))
}

fn minutes(s: &str) -> Result<Minutes, RepoError> {
    serde_json::from_str(s).map_err(|_| corrupt("minutes", s))
}

/// A Meeting's attendees, in their stored order.
async fn attendees(
    conn: &mut SqliteConnection,
    meeting: &str,
) -> Result<Vec<PersonRef>, RepoError> {
    let rows = sqlx::query!(
        "SELECT a.person_id, p.display_name
         FROM meeting_attendees a JOIN people p ON p.id = a.person_id
         WHERE a.meeting_id = ?
         ORDER BY a.position",
        meeting
    )
    .fetch_all(conn)
    .await
    .map_err(unexpected)?;
    rows.into_iter()
        .map(|r| {
            Ok(PersonRef {
                id: PersonId::from_uuid(uuid(&r.person_id)?),
                display_name: r.display_name,
            })
        })
        .collect()
}

impl MeetingRepository for SqliteMeetings<'_> {
    async fn load(&mut self, id: MeetingId) -> Result<Meeting, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            "SELECT project_id, leader_id, started_at, ended_at, minutes, version
             FROM meetings WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        let attendees = attendees(&mut **self.0, &id_text)
            .await?
            .into_iter()
            .map(|a| a.id)
            .collect();
        Ok(Meeting::restore(MeetingParts {
            id,
            project_id: ProjectId::from_uuid(uuid(&row.project_id)?),
            leader: PersonId::from_uuid(uuid(&row.leader_id)?),
            attendees,
            started_at: timestamp(&row.started_at)?,
            ended_at: timestamp(&row.ended_at)?,
            minutes: minutes(&row.minutes)?,
            version: version(row.version)?,
        }))
    }

    async fn create(
        &mut self,
        meeting: &Meeting,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = meeting.id().to_string();
        let project = meeting.project_id().to_string();
        let leader = meeting.leader().to_string();
        let started_at = stored_time(meeting.started_at());
        let ended_at = stored_time(meeting.ended_at());
        let minutes = serde_json::to_string(meeting.minutes())
            .map_err(|e| RepoError::Unexpected(e.to_string()))?;
        let version = i64::from(meeting.version());
        sqlx::query!(
            "INSERT INTO meetings (id, project_id, leader_id, started_at, ended_at, minutes, version)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            id,
            project,
            leader,
            started_at,
            ended_at,
            minutes,
            version
        )
        .execute(&mut **self.0)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::Conflict,
            e => map_constraint(e),
        })?;
        for (position, person) in meeting.attendees().iter().enumerate() {
            let person = person.to_string();
            let position =
                i64::try_from(position).map_err(|e| RepoError::Unexpected(e.to_string()))?;
            sqlx::query!(
                "INSERT INTO meeting_attendees (meeting_id, person_id, position) VALUES (?, ?, ?)",
                id,
                person,
                position
            )
            .execute(&mut **self.0)
            .await
            .map_err(map_constraint)?;
        }
        record_history(
            self.0,
            NewEntry {
                entity_type: EntityType::Meeting,
                entity_id: meeting.id().as_uuid(),
                project_id: Some(meeting.project_id().as_uuid()),
                version: meeting.version(),
                change_kind: ChangeKind::Created,
                before: None,
                after: snapshot(meeting),
            },
            ctx,
        )
        .await
    }
}

/// A `meetings` row with its leader's name.
struct MeetingRow {
    id: String,
    project_id: String,
    leader_id: String,
    leader_name: String,
    started_at: String,
    ended_at: String,
    version: i64,
}

impl SqliteStore {
    async fn summary(&self, r: MeetingRow) -> Result<MeetingSummary, RepoError> {
        let mut conn = self.pool.acquire().await.map_err(unexpected)?;
        Ok(MeetingSummary {
            id: MeetingId::from_uuid(uuid(&r.id)?),
            project_id: ProjectId::from_uuid(uuid(&r.project_id)?),
            leader: PersonRef {
                id: PersonId::from_uuid(uuid(&r.leader_id)?),
                display_name: r.leader_name,
            },
            attendees: attendees(&mut conn, &r.id).await?,
            started_at: timestamp(&r.started_at)?,
            ended_at: timestamp(&r.ended_at)?,
            version: version(r.version)?,
        })
    }
}

impl MeetingsQuery for SqliteStore {
    async fn list_meetings(
        &self,
        project_id: ProjectId,
        page: PageRequest,
    ) -> Result<Page<MeetingSummary>, RepoError> {
        let project = project_id.to_string();
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let rows = sqlx::query_as!(
            MeetingRow,
            r#"SELECT m.id, m.project_id, m.leader_id, p.display_name AS leader_name,
                      m.started_at, m.ended_at, m.version
               FROM meetings m JOIN people p ON p.id = m.leader_id
               WHERE m.project_id = ?
               ORDER BY m.started_at DESC, m.id DESC
               LIMIT ? OFFSET ?"#,
            project,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM meetings WHERE project_id = ?",
            project
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(self.summary(row).await?);
        }
        Ok(Page {
            items,
            total: count(total)?,
        })
    }

    async fn get_meeting(&self, id: MeetingId) -> Result<MeetingView, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            r#"SELECT m.id, m.project_id, m.leader_id, p.display_name AS leader_name,
                      m.started_at, m.ended_at, m.minutes, m.version
               FROM meetings m JOIN people p ON p.id = m.leader_id
               WHERE m.id = ?"#,
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        let minutes = minutes(&row.minutes)?;
        let summary = self
            .summary(MeetingRow {
                id: row.id,
                project_id: row.project_id,
                leader_id: row.leader_id,
                leader_name: row.leader_name,
                started_at: row.started_at,
                ended_at: row.ended_at,
                version: row.version,
            })
            .await?;
        Ok(MeetingView { summary, minutes })
    }
}
