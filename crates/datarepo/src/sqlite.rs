//! SQLite adapter (ADR-0007): WAL, 5-second busy timeout, foreign keys on.
//! Queries use sqlx's compile-time checked macros against `.sqlx/`.

use std::{str::FromStr, time::Duration};

use chrono::{DateTime, SecondsFormat, Utc};
use lodestar_core::health::HealthProbe;
use lodestar_core::history::{
    ChangeContext, ChangeKind, EntityType, HistoryEntry, HistoryEntryId, snapshot,
};
use lodestar_core::person::{DisplayName, Person, PersonId, PersonView};
use lodestar_core::ports::{
    HistoryQuery, Page, PageRequest, PeopleQuery, PersonRepository, ReferenceCounter,
    ReferenceKind, RepoError, Transaction, UnitOfWork,
};
use sqlx::Sqlite;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/sqlite");

/// Errors opening or migrating the database.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A connection pool to a migrated SQLite database.
#[derive(Clone)]
pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    /// Opens (creating if needed) the database at `url` and applies the
    /// embedded migrations (NFR-22).
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let options = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// A store implementing every repository and query port.
    pub fn store(&self) -> SqliteStore {
        SqliteStore::new(self.pool.clone())
    }

    /// The underlying pool, for adapters in this crate and tests.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

impl HealthProbe for SqliteDatabase {
    async fn database_reachable(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}

/// Implements the unit of work, repository and query ports over a migrated pool.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

pub struct SqliteTx(sqlx::Transaction<'static, Sqlite>);

impl UnitOfWork for SqliteStore {
    type Tx = SqliteTx;

    async fn begin(&self) -> Result<SqliteTx, RepoError> {
        // IMMEDIATE takes the write lock up front, so concurrent writers wait
        // on busy_timeout instead of failing on lock upgrade mid-transaction.
        let tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(unexpected)?;
        Ok(SqliteTx(tx))
    }
}

impl Transaction for SqliteTx {
    fn people(&mut self) -> impl PersonRepository + Send + '_ {
        SqlitePeople(&mut self.0)
    }

    fn references(&mut self) -> impl ReferenceCounter + Send + '_ {
        SqliteReferences(&mut self.0)
    }

    async fn commit(self) -> Result<(), RepoError> {
        self.0.commit().await.map_err(unexpected)
    }
}

struct SqlitePeople<'a>(&'a mut sqlx::Transaction<'static, Sqlite>);

impl PersonRepository for SqlitePeople<'_> {
    async fn load(&mut self, id: PersonId) -> Result<Person, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            "SELECT display_name, version FROM people WHERE id = ?",
            id_text
        )
        .fetch_optional(&mut **self.0)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        Ok(Person::restore(
            id,
            display_name(row.display_name)?,
            version(row.version)?,
        ))
    }

    async fn save(
        &mut self,
        before: Option<&Person>,
        after: &Person,
        ctx: &ChangeContext,
    ) -> Result<HistoryEntryId, RepoError> {
        let id = after.id().to_string();
        let name = after.display_name().as_str();
        let new_version = i64::from(after.version());
        let change_kind = match before {
            None => {
                sqlx::query!(
                    "INSERT INTO people (id, display_name, version) VALUES (?, ?, ?)",
                    id,
                    name,
                    new_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(|e| match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::Conflict,
                    e => unexpected(e),
                })?;
                ChangeKind::Created
            }
            Some(before) => {
                let old_version = i64::from(before.version());
                let updated = sqlx::query!(
                    "UPDATE people SET display_name = ?, version = ? WHERE id = ? AND version = ?",
                    name,
                    new_version,
                    id,
                    old_version
                )
                .execute(&mut **self.0)
                .await
                .map_err(unexpected)?;
                if updated.rows_affected() != 1 {
                    return Err(RepoError::Conflict);
                }
                ChangeKind::Updated
            }
        };
        record_history(
            self.0,
            NewEntry {
                entity_type: EntityType::Person,
                entity_id: after.id().as_uuid(),
                project_id: None,
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

struct NewEntry {
    entity_type: EntityType,
    entity_id: Uuid,
    project_id: Option<Uuid>,
    version: u32,
    change_kind: ChangeKind,
    before: Option<serde_json::Value>,
    after: serde_json::Value,
}

/// Writes one Change History entry in the caller's transaction (ADR-0004).
async fn record_history(
    tx: &mut sqlx::Transaction<'static, Sqlite>,
    entry: NewEntry,
    ctx: &ChangeContext,
) -> Result<HistoryEntryId, RepoError> {
    let id = HistoryEntryId::new();
    let id_text = id.as_uuid().to_string();
    let entity_type = entry.entity_type.as_str();
    let entity_id = entry.entity_id.to_string();
    let project_id = entry.project_id.map(|p| p.to_string());
    let version = i64::from(entry.version);
    let change_kind = entry.change_kind.as_str();
    let acting = ctx.acting_person.to_string();
    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
    let before = entry.before.map(|v| v.to_string());
    let after = entry.after.to_string();
    let caused_by = ctx.caused_by.map(|c| c.as_uuid().to_string());
    sqlx::query!(
        "INSERT INTO change_history
            (id, entity_type, entity_id, project_id, version, change_kind,
             acting_person_id, recorded_at, before, after, caused_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id_text,
        entity_type,
        entity_id,
        project_id,
        version,
        change_kind,
        acting,
        recorded_at,
        before,
        after,
        caused_by
    )
    .execute(&mut **tx)
    .await
    .map_err(map_constraint)?;
    Ok(id)
}

struct SqliteReferences<'a>(&'a mut sqlx::Transaction<'static, Sqlite>);

impl ReferenceCounter for SqliteReferences<'_> {
    async fn next(&mut self, project_id: Uuid, kind: ReferenceKind) -> Result<u32, RepoError> {
        let project = project_id.to_string();
        let kind = kind.as_str();
        let next = sqlx::query_scalar!(
            "INSERT INTO reference_counters (project_id, kind, next) VALUES (?, ?, 1)
             ON CONFLICT (project_id, kind) DO UPDATE SET next = next + 1
             RETURNING next",
            project,
            kind
        )
        .fetch_one(&mut **self.0)
        .await
        .map_err(map_constraint)?;
        version(next)
    }
}

impl PeopleQuery for SqliteStore {
    async fn list_people(&self, page: PageRequest) -> Result<Page<PersonView>, RepoError> {
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let rows = sqlx::query!(
            "SELECT id, display_name, version FROM people
             ORDER BY display_name, id LIMIT ? OFFSET ?",
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!("SELECT COUNT(*) FROM people")
            .fetch_one(&self.pool)
            .await
            .map_err(unexpected)?;
        let items = rows
            .into_iter()
            .map(|r| {
                Ok(PersonView {
                    id: PersonId::from_uuid(uuid(&r.id)?),
                    display_name: r.display_name,
                    version: version(r.version)?,
                })
            })
            .collect::<Result<_, RepoError>>()?;
        Ok(Page {
            items,
            total: count(total)?,
        })
    }

    async fn get_person(&self, id: PersonId) -> Result<PersonView, RepoError> {
        let id_text = id.to_string();
        let row = sqlx::query!(
            "SELECT display_name, version FROM people WHERE id = ?",
            id_text
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unexpected)?
        .ok_or(RepoError::NotFound)?;
        Ok(PersonView {
            id,
            display_name: row.display_name,
            version: version(row.version)?,
        })
    }
}

impl HistoryQuery for SqliteStore {
    async fn entity_history(
        &self,
        entity_type: EntityType,
        entity_id: Uuid,
        page: PageRequest,
    ) -> Result<Page<HistoryEntry>, RepoError> {
        let entity_type_text = entity_type.as_str();
        let entity_id_text = entity_id.to_string();
        let limit = i64::from(page.limit);
        let offset = i64::from(page.offset);
        let rows = sqlx::query!(
            "SELECT id, project_id, version, change_kind, acting_person_id,
                    recorded_at, before, after, caused_by
             FROM change_history
             WHERE entity_type = ? AND entity_id = ?
             ORDER BY version LIMIT ? OFFSET ?",
            entity_type_text,
            entity_id_text,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unexpected)?;
        let total = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM change_history WHERE entity_type = ? AND entity_id = ?",
            entity_type_text,
            entity_id_text
        )
        .fetch_one(&self.pool)
        .await
        .map_err(unexpected)?;
        let items = rows
            .into_iter()
            .map(|r| {
                Ok(HistoryEntry {
                    id: HistoryEntryId::from_uuid(uuid(&r.id)?),
                    entity_type,
                    entity_id,
                    project_id: r.project_id.as_deref().map(uuid).transpose()?,
                    version: version(r.version)?,
                    change_kind: ChangeKind::parse(&r.change_kind)
                        .ok_or_else(|| corrupt("change_kind", &r.change_kind))?,
                    acting_person: PersonId::from_uuid(uuid(&r.acting_person_id)?),
                    recorded_at: DateTime::parse_from_rfc3339(&r.recorded_at)
                        .map_err(|_| corrupt("recorded_at", &r.recorded_at))?
                        .with_timezone(&Utc),
                    before: r.before.as_deref().map(json).transpose()?,
                    after: json(&r.after)?,
                    caused_by: r
                        .caused_by
                        .as_deref()
                        .map(|c| uuid(c).map(HistoryEntryId::from_uuid))
                        .transpose()?,
                })
            })
            .collect::<Result<_, RepoError>>()?;
        Ok(Page {
            items,
            total: count(total)?,
        })
    }
}

fn unexpected(e: sqlx::Error) -> RepoError {
    RepoError::Unexpected(e.to_string())
}

fn map_constraint(e: sqlx::Error) -> RepoError {
    match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => RepoError::UniqueViolation {
            constraint: db
                .constraint()
                .map(str::to_owned)
                .unwrap_or_else(|| db.message().to_owned()),
        },
        e => unexpected(e),
    }
}

fn corrupt(column: &str, value: &str) -> RepoError {
    RepoError::Unexpected(format!("invalid stored {column}: {value:?}"))
}

fn uuid(s: &str) -> Result<Uuid, RepoError> {
    Uuid::parse_str(s).map_err(|_| corrupt("uuid", s))
}

fn version(v: i64) -> Result<u32, RepoError> {
    u32::try_from(v).map_err(|_| corrupt("integer", &v.to_string()))
}

fn count(v: i64) -> Result<u64, RepoError> {
    u64::try_from(v).map_err(|_| corrupt("count", &v.to_string()))
}

fn json(s: &str) -> Result<serde_json::Value, RepoError> {
    serde_json::from_str(s).map_err(|_| corrupt("json", s))
}

fn display_name(s: String) -> Result<DisplayName, RepoError> {
    DisplayName::new(&s).map_err(|_| corrupt("display_name", &s))
}
