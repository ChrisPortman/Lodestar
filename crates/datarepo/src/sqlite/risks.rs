//! The Risk repository and query (FR-40 to FR-43).

use lodestar_core::history::{ChangeContext, ChangeKind, EntityType, HistoryEntryId, snapshot};
use lodestar_core::ports::{
    Page, PageRequest, ReferenceKind, RepoError, RiskFilter, RiskRepository, RiskSort, RisksQuery,
};
use lodestar_core::project::ProjectId;
use lodestar_core::risk::{Rating, Risk, RiskId, RiskParts, RiskView};
use sqlx::Sqlite;

use super::{
    NewEntry, SqliteStore, count, corrupt, description, map_constraint, record_history,
    unexpected, uuid, version,
};

pub(super) struct SqliteRisks<'a>(pub(super) &'a mut sqlx::Transaction<'static, Sqlite>);

fn rating(v: i64) -> Result<Rating, RepoError> {
    u8::try_from(v)
        .ok()
        .and_then(|v| Rating::new(v).ok())
        .ok_or_else(|| corrupt("rating", &v.to_string()))
}

impl RiskRepository for SqliteRisks<'_> {
    async fn load(&mut self, id: RiskId) -> Result<Risk, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            "SELECT project_id, number, description, likelihood, impact, closed, version
             FROM risks WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        Ok(Risk::restore(RiskParts {
            id,
            project_id: ProjectId::from_uuid(uuid(&row.project_id)?),
            number: version(row.number)?,
            description: description(row.description)?,
            likelihood: rating(row.likelihood)?,
            impact: rating(row.impact)?,
            closed: row.closed != 0,
            version: version(row.version)?,
        }))
    }

    async fn save(
        &mut self,
        before: Option<&Risk>,
        after: &Risk,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = after.id().to_string();
        let description = after.description().as_str();
        let likelihood = after.likelihood().get();
        let impact = after.impact().get();
        let closed = after.closed();
        let new_version = i64::from(after.version());
        let change_kind = match before {
            None => {
                let project = after.project_id().to_string();
                let number = i64::from(after.number());
                sqlx::query!(
                    "INSERT INTO risks
                        (id, project_id, number, description, likelihood, impact, closed, version)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    id,
                    project,
                    number,
                    description,
                    likelihood,
                    impact,
                    closed,
                    new_version
                )
                .execute(&mut **self.0)
                .await
                // The ID and the Reference are both unique; either clash means
                // this Risk (or its Reference) was already created.
                .map_err(|e| match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::Conflict,
                    e => map_constraint(e),
                })?;
                ChangeKind::Created
            }
            Some(before) => {
                let old_version = i64::from(before.version());
                let updated = sqlx::query!(
                    "UPDATE risks SET description = ?, likelihood = ?, impact = ?, closed = ?,
                        version = ?
                     WHERE id = ? AND version = ?",
                    description,
                    likelihood,
                    impact,
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
                entity_type: EntityType::Risk,
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

/// A row of the Risk list query.
struct RiskRow {
    id: String,
    project_id: String,
    project_code: String,
    number: i64,
    description: String,
    likelihood: i64,
    impact: i64,
    closed: i64,
    version: i64,
}

impl TryFrom<RiskRow> for RiskView {
    type Error = RepoError;

    fn try_from(r: RiskRow) -> Result<Self, RepoError> {
        let likelihood = rating(r.likelihood)?;
        let impact = rating(r.impact)?;
        Ok(RiskView {
            id: RiskId::from_uuid(uuid(&r.id)?),
            project_id: ProjectId::from_uuid(uuid(&r.project_id)?),
            project_code: r.project_code,
            reference: ReferenceKind::Risk.reference(version(r.number)?),
            description: r.description,
            likelihood: likelihood.get(),
            impact: impact.get(),
            score: likelihood.get() * impact.get(),
            closed: r.closed != 0,
            version: version(r.version)?,
        })
    }
}

impl RisksQuery for SqliteStore {
    async fn list_risks(
        &self,
        page: PageRequest,
        filter: RiskFilter,
        sort: RiskSort,
    ) -> Result<Page<RiskView>, RepoError> {
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let project = filter.project_id.map(|p| p.to_string());
        let include_closed = filter.include_closed;
        let by_reference = sort == RiskSort::Reference;
        let by_score = sort == RiskSort::Score;
        let rows = sqlx::query_as!(
            RiskRow,
            "SELECT r.id, r.project_id, p.project_code, r.number, r.description,
                    r.likelihood, r.impact, r.closed, r.version
             FROM risks r JOIN projects p ON p.id = r.project_id
             WHERE (?1 IS NULL OR r.project_id = ?1) AND (?2 OR r.closed = 0)
             ORDER BY CASE WHEN ?3 THEN r.likelihood * r.impact END DESC,
                      CASE WHEN ?4 THEN r.project_id END,
                      CASE WHEN ?4 THEN r.number END,
                      r.id
             LIMIT ?5 OFFSET ?6",
            project,
            include_closed,
            by_score,
            by_reference,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM risks
             WHERE (?1 IS NULL OR project_id = ?1) AND (?2 OR closed = 0)",
            project,
            include_closed
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        let items = rows
            .into_iter()
            .map(RiskView::try_from)
            .collect::<Result<_, RepoError>>()?;
        Ok(Page {
            items,
            total: count(total)?,
        })
    }

    async fn get_risk(&self, id: RiskId) -> Result<RiskView, RepoError> {
        let id_text = id.to_string();
        sqlx::query_as!(
            RiskRow,
            "SELECT r.id, r.project_id, p.project_code, r.number, r.description,
                    r.likelihood, r.impact, r.closed, r.version
             FROM risks r JOIN projects p ON p.id = r.project_id
             WHERE r.id = ?",
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?
        .try_into()
    }
}
