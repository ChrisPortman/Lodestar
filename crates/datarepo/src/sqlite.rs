//! SQLite adapter (ADR-0007): WAL, 5-second busy timeout, foreign keys on.

use std::{str::FromStr, time::Duration};

use lodestar_core::health::HealthProbe;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

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
