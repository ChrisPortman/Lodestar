//! The Action repository and query (FR-30 to FR-33).

use chrono::NaiveDate;
use lodestar_core::action::{Action, ActionId, ActionParts, ActionView};
use lodestar_core::history::{ChangeContext, ChangeKind, EntityType, HistoryEntryId, snapshot};
use lodestar_core::person::PersonId;
use lodestar_core::ports::{
    ActionFilter, ActionRepository, ActionsQuery, AssigneeFilter, ClosedFilter, Page, PageRequest,
    ReferenceKind, RepoError,
};
use lodestar_core::project::{ItemRef, PersonRef, ProjectId};
use std::collections::HashMap;

use sqlx::Sqlite;
use uuid::Uuid;

use super::blockers::open_blockers_of;

use super::{
    NewEntry, SqliteStore, corrupt, count, description, map_constraint, record_history, unexpected,
    uuid, version,
};

pub(super) struct SqliteActions<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

fn date(s: &str) -> Result<NaiveDate, RepoError> {
    s.parse().map_err(|_| corrupt("date", s))
}

fn person(id: Option<&str>) -> Result<Option<PersonId>, RepoError> {
    id.map(|a| uuid(a).map(PersonId::from_uuid)).transpose()
}

/// A row of the `actions` table.
struct ActionRow {
    id: String,
    project_id: String,
    reference: i64,
    description: String,
    assignee_id: Option<String>,
    due_date: Option<String>,
    closed: i64,
    version: i64,
}

impl TryFrom<ActionRow> for Action {
    type Error = RepoError;

    fn try_from(r: ActionRow) -> Result<Self, RepoError> {
        Ok(Action::restore(ActionParts {
            id: ActionId::from_uuid(uuid(&r.id)?),
            project_id: ProjectId::from_uuid(uuid(&r.project_id)?),
            reference: version(r.reference)?,
            description: description(r.description)?,
            assignee: person(r.assignee_id.as_deref())?,
            due_date: r.due_date.as_deref().map(date).transpose()?,
            closed: r.closed != 0,
            version: version(r.version)?,
        }))
    }
}

impl ActionRepository for SqliteActions<'_> {
    async fn load(&mut self, id: ActionId) -> Result<Action, RepoError> {
        let id_text = id.to_string();
        sqlx::query_as!(
            ActionRow,
            "SELECT id, project_id, reference, description, assignee_id, due_date, closed, version
             FROM actions WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?
        .try_into()
    }

    async fn save(
        &mut self,
        before: Option<&Action>,
        after: &Action,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = after.id().to_string();
        let project = after.project_id().to_string();
        let reference = i64::from(after.reference());
        let description = after.description().as_str();
        let assignee = after.assignee().map(|a| a.to_string());
        let due = after.due_date().map(|d| d.to_string());
        let closed = after.closed();
        let new_version = i64::from(after.version());
        let change_kind = match before {
            None => {
                sqlx::query!(
                    "INSERT INTO actions
                        (id, project_id, reference, description, assignee_id, due_date,
                         closed, version)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    id,
                    project,
                    reference,
                    description,
                    assignee,
                    due,
                    closed,
                    new_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db)
                        if db.is_unique_violation() && db.message().contains("actions.id") =>
                    {
                        RepoError::Conflict
                    }
                    e => map_constraint(e),
                })?;
                ChangeKind::Created
            }
            Some(before) => {
                let old_version = i64::from(before.version());
                let updated = sqlx::query!(
                    "UPDATE actions SET description = ?, assignee_id = ?, due_date = ?,
                        closed = ?, version = ?
                     WHERE id = ? AND version = ?",
                    description,
                    assignee,
                    due,
                    closed,
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
                entity_type: EntityType::Action,
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

    async fn open_assigned_to(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> Result<Vec<Action>, RepoError> {
        let project = project.to_string();
        let person = person.to_string();
        sqlx::query_as!(
            ActionRow,
            "SELECT id, project_id, reference, description, assignee_id, due_date, closed, version
             FROM actions WHERE project_id = ? AND assignee_id = ? AND closed = 0
             ORDER BY reference",
            project,
            person
        )
        .fetch_all(&mut **self.0)
        .await
        .map_err(unexpected)?
        .into_iter()
        .map(Action::try_from)
        .collect()
    }
}

/// A row of the Action view queries.
struct ActionViewRow {
    id: String,
    project_id: String,
    project_code: String,
    reference: i64,
    description: String,
    assignee_id: Option<String>,
    assignee_name: Option<String>,
    due_date: Option<String>,
    closed: i64,
    version: i64,
}

impl ActionViewRow {
    fn into_view(
        self,
        blockers: &mut HashMap<Uuid, Vec<ItemRef>>,
    ) -> Result<ActionView, RepoError> {
        let r = self;
        let id = uuid(&r.id)?;
        let assignee = person(r.assignee_id.as_deref())?.map(|id| PersonRef {
            id,
            display_name: r.assignee_name.unwrap_or_default(),
        });
        Ok(ActionView {
            id: ActionId::from_uuid(id),
            project_id: ProjectId::from_uuid(uuid(&r.project_id)?),
            project_code: r.project_code,
            reference: ReferenceKind::Action.reference(version(r.reference)?),
            description: r.description,
            assignee,
            due_date: r.due_date.as_deref().map(date).transpose()?,
            closed: r.closed != 0,
            blocked_by: blockers.remove(&id).unwrap_or_default(),
            version: version(r.version)?,
        })
    }
}

impl SqliteStore {
    async fn action_views(&self, rows: Vec<ActionViewRow>) -> Result<Vec<ActionView>, RepoError> {
        let ids = rows
            .iter()
            .map(|r| uuid(&r.id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut blockers = open_blockers_of(&self.pool, ids).await?;
        rows.into_iter()
            .map(|r| r.into_view(&mut blockers))
            .collect()
    }
}

impl ActionsQuery for SqliteStore {
    async fn list_actions(
        &self,
        filter: ActionFilter,
        page: PageRequest,
    ) -> Result<Page<ActionView>, RepoError> {
        let project = filter.project_id.map(|p| p.to_string());
        let include_closed = filter.state != ClosedFilter::Open;
        let closed_only = filter.state == ClosedFilter::Closed;
        let unassigned = filter.assignee == Some(AssigneeFilter::Unassigned);
        let assignee = match filter.assignee {
            Some(AssigneeFilter::Person(id)) => Some(id.to_string()),
            _ => None,
        };
        let due_by = filter.due_by.map(|d| d.to_string());
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let rows = sqlx::query_as!(
            ActionViewRow,
            r#"SELECT a.id, a.project_id, p.project_code, a.reference, a.description,
                      a.assignee_id, pe.display_name AS "assignee_name?", a.due_date,
                      a.closed, a.version
               FROM actions a
               JOIN projects p ON p.id = a.project_id
               LEFT JOIN people pe ON pe.id = a.assignee_id
               WHERE (?1 IS NULL OR a.project_id = ?1) AND (?2 OR a.closed = 0)
                 AND (NOT ?3 OR a.closed = 1) AND (NOT ?4 OR a.assignee_id IS NULL)
                 AND (?5 IS NULL OR a.assignee_id = ?5) AND (?6 IS NULL OR a.due_date <= ?6)
               ORDER BY p.project_code, a.reference, a.id LIMIT ?7 OFFSET ?8"#,
            project,
            include_closed,
            closed_only,
            unassigned,
            assignee,
            due_by,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM actions
             WHERE (?1 IS NULL OR project_id = ?1) AND (?2 OR closed = 0)
               AND (NOT ?3 OR closed = 1) AND (NOT ?4 OR assignee_id IS NULL)
               AND (?5 IS NULL OR assignee_id = ?5) AND (?6 IS NULL OR due_date <= ?6)",
            project,
            include_closed,
            closed_only,
            unassigned,
            assignee,
            due_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        let items = self.action_views(rows).await?;
        Ok(Page {
            items,
            total: count(total)?,
        })
    }

    async fn get_action(&self, id: ActionId) -> Result<ActionView, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query_as!(
            ActionViewRow,
            r#"SELECT a.id, a.project_id, p.project_code, a.reference, a.description,
                      a.assignee_id, pe.display_name AS "assignee_name?", a.due_date,
                      a.closed, a.version
               FROM actions a
               JOIN projects p ON p.id = a.project_id
               LEFT JOIN people pe ON pe.id = a.assignee_id
               WHERE a.id = ?"#,
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        self.action_views(vec![row])
            .await?
            .pop()
            .ok_or_else(|| corrupt("actions.id", &id_text))
    }
}
