//! The Blocker repository and query (FR-50 to FR-54). A Blocker's links to
//! the Actions it blocks live in `blocker_actions` and are saved with it.

use std::collections::{BTreeSet, HashMap};

use lodestar_core::action::ActionId;
use lodestar_core::blocker::{Blocker, BlockerId, BlockerParts, BlockerView};
use lodestar_core::history::{ChangeContext, ChangeKind, EntityType, HistoryEntryId, snapshot};
use lodestar_core::person::PersonId;
use lodestar_core::ports::{
    AssigneeFilter, BlockerFilter, BlockerRepository, BlockersQuery, Page, PageRequest,
    ReferenceKind, RepoError,
};
use lodestar_core::project::{ItemRef, PersonRef, ProjectId};
use sqlx::{Sqlite, SqliteExecutor};
use uuid::Uuid;

use super::{
    NewEntry, SqliteStore, corrupt, count, description, map_constraint, record_history, unexpected,
    uuid, version,
};

pub(super) struct SqliteBlockers<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

fn person(id: Option<&str>) -> Result<Option<PersonId>, RepoError> {
    id.map(|a| uuid(a).map(PersonId::from_uuid)).transpose()
}

/// A JSON array of IDs, for `json_each` in `IN` lists.
fn id_list(ids: impl IntoIterator<Item = Uuid>) -> String {
    serde_json::to_string(&ids.into_iter().map(|i| i.to_string()).collect::<Vec<_>>())
        .expect("strings serialise to JSON")
}

/// A row of the `blockers` table.
struct BlockerRow {
    id: String,
    project_id: String,
    reference: i64,
    description: String,
    accountable_id: Option<String>,
    cleared: i64,
    version: i64,
}

impl BlockerRow {
    fn into_blocker(self, blocked_actions: BTreeSet<ActionId>) -> Result<Blocker, RepoError> {
        Ok(Blocker::restore(BlockerParts {
            id: BlockerId::from_uuid(uuid(&self.id)?),
            project_id: ProjectId::from_uuid(uuid(&self.project_id)?),
            reference: version(self.reference)?,
            description: description(self.description)?,
            accountable: person(self.accountable_id.as_deref())?,
            cleared: self.cleared != 0,
            blocked_actions,
            version: version(self.version)?,
        }))
    }
}

async fn linked_actions(
    tx: &mut sqlx::Transaction<'static, Sqlite>,
    blocker: &str,
) -> Result<BTreeSet<ActionId>, RepoError> {
    sqlx::query_scalar!(
        "SELECT action_id FROM blocker_actions WHERE blocker_id = ?",
        blocker
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(unexpected)?
    .iter()
    .map(|a| uuid(a).map(ActionId::from_uuid))
    .collect()
}

impl SqliteBlockers<'_> {
    async fn restore(&mut self, row: BlockerRow) -> Result<Blocker, RepoError> {
        let links = linked_actions(self.0, &row.id).await?;
        row.into_blocker(links)
    }
}

impl BlockerRepository for SqliteBlockers<'_> {
    async fn load(&mut self, id: BlockerId) -> Result<Blocker, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query_as!(
            BlockerRow,
            "SELECT id, project_id, reference, description, accountable_id, cleared, version
             FROM blockers WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        self.restore(row).await
    }

    async fn save(
        &mut self,
        before: Option<&Blocker>,
        after: &Blocker,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = after.id().to_string();
        let project = after.project_id().to_string();
        let reference = i64::from(after.reference());
        let description = after.description().as_str();
        let accountable = after.accountable().map(|a| a.to_string());
        let cleared = after.cleared();
        let new_version = i64::from(after.version());
        let change_kind = match before {
            None => {
                sqlx::query!(
                    "INSERT INTO blockers
                        (id, project_id, reference, description, accountable_id, cleared, version)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    id,
                    project,
                    reference,
                    description,
                    accountable,
                    cleared,
                    new_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db)
                        if db.is_unique_violation() && db.message().contains("blockers.id") =>
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
                    "UPDATE blockers SET description = ?, accountable_id = ?, cleared = ?,
                        version = ?
                     WHERE id = ? AND version = ?",
                    description,
                    accountable,
                    cleared,
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
        let old_links = before.map(Blocker::blocked_actions);
        if old_links != Some(after.blocked_actions()) {
            sqlx::query!("DELETE FROM blocker_actions WHERE blocker_id = ?", id)
                .execute(&mut **self.0)
                .await
                .map_err(unexpected)?;
            for action in after.blocked_actions() {
                let action = action.to_string();
                sqlx::query!(
                    "INSERT INTO blocker_actions (blocker_id, action_id) VALUES (?, ?)",
                    id,
                    action
                )
                .execute(&mut **self.0)
                .await
                .map_err(map_constraint)?;
            }
        }
        record_history(
            self.0,
            NewEntry {
                entity_type: EntityType::Blocker,
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

    async fn open_accountable(
        &mut self,
        project: ProjectId,
        person: PersonId,
    ) -> Result<Vec<Blocker>, RepoError> {
        let project = project.to_string();
        let person = person.to_string();
        let rows = sqlx::query_as!(
            BlockerRow,
            "SELECT id, project_id, reference, description, accountable_id, cleared, version
             FROM blockers WHERE project_id = ? AND accountable_id = ? AND cleared = 0
             ORDER BY reference",
            project,
            person
        )
        .fetch_all(&mut **self.0)
        .await
        .map_err(unexpected)?;
        let mut blockers = Vec::with_capacity(rows.len());
        for row in rows {
            blockers.push(self.restore(row).await?);
        }
        Ok(blockers)
    }
}

/// For each of `actions`, the open Blockers blocking it, ordered by Reference (FR-53).
pub(super) async fn open_blockers_of(
    db: impl SqliteExecutor<'_>,
    actions: impl IntoIterator<Item = Uuid>,
) -> Result<HashMap<Uuid, Vec<ItemRef>>, RepoError> {
    let ids = id_list(actions);
    let rows = sqlx::query!(
        "SELECT ba.action_id, b.id AS blocker_id, b.reference
         FROM blocker_actions ba JOIN blockers b ON b.id = ba.blocker_id
         WHERE b.cleared = 0 AND ba.action_id IN (SELECT value FROM json_each(?))
         ORDER BY b.reference, b.id",
        ids
    )
    .fetch_all(db)
    .await
    .map_err(unexpected)?;
    let mut by_action: HashMap<Uuid, Vec<ItemRef>> = HashMap::new();
    for r in rows {
        by_action
            .entry(uuid(&r.action_id)?)
            .or_default()
            .push(ItemRef {
                id: uuid(&r.blocker_id)?,
                reference: ReferenceKind::Blocker.reference(version(r.reference)?),
            });
    }
    Ok(by_action)
}

/// For each of `blockers`, the Actions it blocks, ordered by Reference.
async fn blocked_actions_of(
    db: impl SqliteExecutor<'_>,
    blockers: impl IntoIterator<Item = Uuid>,
) -> Result<HashMap<Uuid, Vec<ItemRef>>, RepoError> {
    let ids = id_list(blockers);
    let rows = sqlx::query!(
        "SELECT ba.blocker_id, a.id AS action_id, a.reference
         FROM blocker_actions ba JOIN actions a ON a.id = ba.action_id
         WHERE ba.blocker_id IN (SELECT value FROM json_each(?))
         ORDER BY a.reference, a.id",
        ids
    )
    .fetch_all(db)
    .await
    .map_err(unexpected)?;
    let mut by_blocker: HashMap<Uuid, Vec<ItemRef>> = HashMap::new();
    for r in rows {
        by_blocker
            .entry(uuid(&r.blocker_id)?)
            .or_default()
            .push(ItemRef {
                id: uuid(&r.action_id)?,
                reference: ReferenceKind::Action.reference(version(r.reference)?),
            });
    }
    Ok(by_blocker)
}

/// A row of the Blocker view queries.
struct BlockerViewRow {
    id: String,
    project_id: String,
    project_code: String,
    reference: i64,
    description: String,
    accountable_id: Option<String>,
    accountable_name: Option<String>,
    cleared: i64,
    version: i64,
}

impl BlockerViewRow {
    fn into_view(self, links: &mut HashMap<Uuid, Vec<ItemRef>>) -> Result<BlockerView, RepoError> {
        let id = uuid(&self.id)?;
        let accountable = person(self.accountable_id.as_deref())?.map(|id| PersonRef {
            id,
            display_name: self.accountable_name.unwrap_or_default(),
        });
        Ok(BlockerView {
            id: BlockerId::from_uuid(id),
            project_id: ProjectId::from_uuid(uuid(&self.project_id)?),
            project_code: self.project_code,
            reference: ReferenceKind::Blocker.reference(version(self.reference)?),
            description: self.description,
            accountable,
            cleared: self.cleared != 0,
            blocked_actions: links.remove(&id).unwrap_or_default(),
            version: version(self.version)?,
        })
    }
}

impl SqliteStore {
    async fn blocker_views(
        &self,
        rows: Vec<BlockerViewRow>,
    ) -> Result<Vec<BlockerView>, RepoError> {
        let ids = rows
            .iter()
            .map(|r| uuid(&r.id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut links = blocked_actions_of(&self.pool, ids).await?;
        rows.into_iter().map(|r| r.into_view(&mut links)).collect()
    }
}

impl BlockersQuery for SqliteStore {
    async fn list_blockers(
        &self,
        filter: BlockerFilter,
        page: PageRequest,
    ) -> Result<Page<BlockerView>, RepoError> {
        let project = filter.project_id.map(|p| p.to_string());
        let include_cleared = filter.include_cleared || filter.cleared_only;
        let cleared_only = filter.cleared_only;
        let unassigned = filter.accountable == Some(AssigneeFilter::Unassigned);
        let accountable = match filter.accountable {
            Some(AssigneeFilter::Person(id)) => Some(id.to_string()),
            _ => None,
        };
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let rows = sqlx::query_as!(
            BlockerViewRow,
            r#"SELECT b.id, b.project_id, p.project_code, b.reference, b.description,
                      b.accountable_id, pe.display_name AS "accountable_name?",
                      b.cleared, b.version
               FROM blockers b
               JOIN projects p ON p.id = b.project_id
               LEFT JOIN people pe ON pe.id = b.accountable_id
               WHERE (?1 IS NULL OR b.project_id = ?1) AND (?2 OR b.cleared = 0)
                 AND (NOT ?3 OR b.cleared = 1) AND (NOT ?4 OR b.accountable_id IS NULL)
                 AND (?5 IS NULL OR b.accountable_id = ?5)
               ORDER BY p.project_code, b.reference, b.id LIMIT ?6 OFFSET ?7"#,
            project,
            include_cleared,
            cleared_only,
            unassigned,
            accountable,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM blockers
             WHERE (?1 IS NULL OR project_id = ?1) AND (?2 OR cleared = 0)
               AND (NOT ?3 OR cleared = 1) AND (NOT ?4 OR accountable_id IS NULL)
               AND (?5 IS NULL OR accountable_id = ?5)",
            project,
            include_cleared,
            cleared_only,
            unassigned,
            accountable
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        Ok(Page {
            items: self.blocker_views(rows).await?,
            total: count(total)?,
        })
    }

    async fn get_blocker(&self, id: BlockerId) -> Result<BlockerView, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query_as!(
            BlockerViewRow,
            r#"SELECT b.id, b.project_id, p.project_code, b.reference, b.description,
                      b.accountable_id, pe.display_name AS "accountable_name?",
                      b.cleared, b.version
               FROM blockers b
               JOIN projects p ON p.id = b.project_id
               LEFT JOIN people pe ON pe.id = b.accountable_id
               WHERE b.id = ?"#,
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        self.blocker_views(vec![row])
            .await?
            .pop()
            .ok_or_else(|| corrupt("blockers.id", &id_text))
    }
}
