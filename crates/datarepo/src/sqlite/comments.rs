//! The Comment repository and query (FR-120 to FR-126).

use chrono::{DateTime, SecondsFormat, Utc};
use lodestar_core::comment::{
    Comment, CommentBody, CommentId, CommentParts, CommentView, Target, TargetType,
};
use lodestar_core::history::{ChangeContext, ChangeKind, EntityType, HistoryEntryId, snapshot};
use lodestar_core::person::PersonId;
use lodestar_core::ports::{
    CommentFilter, CommentRepository, CommentsQuery, Page, PageRequest, RepoError,
};
use lodestar_core::project::{PersonRef, ProjectId};
use sqlx::Sqlite;

use super::{
    NewEntry, SqliteStore, corrupt, count, map_constraint, record_history, unexpected, uuid,
    version,
};

pub(super) struct SqliteComments<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

/// Stored as RFC 3339 UTC to the microsecond, so text order is time order.
fn stored_time(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn timestamp(s: &str) -> Result<DateTime<Utc>, RepoError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| corrupt("timestamp", s))
}

fn target(kind: &str, id: &str) -> Result<Target, RepoError> {
    Ok(Target {
        kind: TargetType::parse(kind).ok_or_else(|| corrupt("target_type", kind))?,
        id: uuid(id)?,
    })
}

impl CommentRepository for SqliteComments<'_> {
    async fn load(&mut self, id: CommentId) -> Result<Comment, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            "SELECT project_id, target_type, target_id, author_id, body, created_at,
                    edited_at, deleted, version
             FROM comments WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        Ok(Comment::restore(CommentParts {
            id,
            project_id: ProjectId::from_uuid(uuid(&row.project_id)?),
            target: target(&row.target_type, &row.target_id)?,
            author: PersonId::from_uuid(uuid(&row.author_id)?),
            body: CommentBody::new(row.body.clone()).map_err(|_| corrupt("body", &row.body))?,
            created_at: timestamp(&row.created_at)?,
            edited_at: row.edited_at.as_deref().map(timestamp).transpose()?,
            deleted: row.deleted != 0,
            version: version(row.version)?,
        }))
    }

    async fn save(
        &mut self,
        before: Option<&Comment>,
        after: &Comment,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = after.id().to_string();
        let body = after.body().as_str();
        let edited_at = after.edited_at().map(stored_time);
        let deleted = after.is_deleted();
        let new_version = i64::from(after.version());
        let change_kind = match before {
            None => {
                let project = after.project_id().to_string();
                let target_type = after.target().kind.as_str();
                let target_id = after.target().id.to_string();
                let author = after.author().to_string();
                let created_at = stored_time(after.created_at());
                sqlx::query!(
                    "INSERT INTO comments
                        (id, project_id, target_type, target_id, author_id, body, created_at,
                         edited_at, deleted, version)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    id,
                    project,
                    target_type,
                    target_id,
                    author,
                    body,
                    created_at,
                    edited_at,
                    deleted,
                    new_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::Conflict,
                    e => map_constraint(e),
                })?;
                ChangeKind::Created
            }
            Some(before) => {
                let old_version = i64::from(before.version());
                let updated = sqlx::query!(
                    "UPDATE comments SET body = ?, edited_at = ?, deleted = ?, version = ?
                     WHERE id = ? AND version = ?",
                    body,
                    edited_at,
                    deleted,
                    new_version,
                    id,
                    old_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(map_constraint)?;
                if updated.rows_affected() != 1 {
                    return Err(RepoError::Conflict);
                }
                ChangeKind::Updated
            }
        };
        record_history(
            self.0,
            NewEntry {
                entity_type: EntityType::Comment,
                entity_id: after.id().as_uuid(),
                project_id: Some(after.project_id().as_uuid()),
                version: after.version(),
                change_kind,
                before: before.map(snapshot),
                after: snapshot(after),
            },
            ctx,
        )
        .await
    }
}

/// A row of the Comment list query.
struct CommentRow {
    id: String,
    project_id: String,
    target_type: String,
    target_id: String,
    author_id: String,
    author_name: String,
    body: String,
    created_at: String,
    edited_at: Option<String>,
    deleted: i64,
    version: i64,
}

impl TryFrom<CommentRow> for CommentView {
    type Error = RepoError;

    fn try_from(r: CommentRow) -> Result<Self, RepoError> {
        Ok(CommentView {
            id: CommentId::from_uuid(uuid(&r.id)?),
            project_id: ProjectId::from_uuid(uuid(&r.project_id)?),
            target: target(&r.target_type, &r.target_id)?,
            author: PersonRef {
                id: PersonId::from_uuid(uuid(&r.author_id)?),
                display_name: r.author_name,
            },
            body: r.body,
            created_at: timestamp(&r.created_at)?,
            edited_at: r.edited_at.as_deref().map(timestamp).transpose()?,
            deleted: r.deleted != 0,
            version: version(r.version)?,
        })
    }
}

impl CommentsQuery for SqliteStore {
    async fn list_comments(
        &self,
        filter: CommentFilter,
        page: PageRequest,
    ) -> Result<Page<CommentView>, RepoError> {
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let project = filter.project_id.map(|p| p.to_string());
        let target_type = filter.target.map(|t| t.kind.as_str());
        let target_id = filter.target.map(|t| t.id.to_string());
        let rows = sqlx::query_as!(
            CommentRow,
            r#"SELECT c.id, c.project_id, c.target_type, c.target_id, c.author_id,
                      pe.display_name AS author_name, c.body, c.created_at, c.edited_at,
                      c.deleted, c.version
               FROM comments c JOIN people pe ON pe.id = c.author_id
               WHERE (?1 IS NULL OR c.project_id = ?1)
                 AND (?2 IS NULL OR (c.target_type = ?2 AND c.target_id = ?3))
               ORDER BY c.created_at, c.id
               LIMIT ?4 OFFSET ?5"#,
            project,
            target_type,
            target_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM comments
             WHERE (?1 IS NULL OR project_id = ?1)
               AND (?2 IS NULL OR (target_type = ?2 AND target_id = ?3))",
            project,
            target_type,
            target_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        let items = rows
            .into_iter()
            .map(CommentView::try_from)
            .collect::<Result<_, RepoError>>()?;
        Ok(Page {
            items,
            total: count(total)?,
        })
    }

    async fn get_comment(&self, id: CommentId) -> Result<CommentView, RepoError> {
        let id_text = id.to_string();
        sqlx::query_as!(
            CommentRow,
            r#"SELECT c.id, c.project_id, c.target_type, c.target_id, c.author_id,
                      pe.display_name AS author_name, c.body, c.created_at, c.edited_at,
                      c.deleted, c.version
               FROM comments c JOIN people pe ON pe.id = c.author_id
               WHERE c.id = ?"#,
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?
        .try_into()
    }
}
