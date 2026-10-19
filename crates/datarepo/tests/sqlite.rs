#![cfg(feature = "sqlite")]

use lodestar_core::health::HealthProbe;
use lodestar_datarepo::sqlite::SqliteDatabase;

fn temp_url(dir: &tempfile::TempDir) -> String {
    format!("sqlite://{}", dir.path().join("lodestar.db").display())
}

#[tokio::test]
async fn connect_creates_the_database_and_is_reachable() {
    let dir = tempfile::tempdir().unwrap();
    let db = SqliteDatabase::connect(&temp_url(&dir)).await.unwrap();

    assert!(dir.path().join("lodestar.db").exists());
    assert!(db.database_reachable().await);
}

#[tokio::test]
async fn connections_use_wal_busy_timeout_and_foreign_keys() {
    let dir = tempfile::tempdir().unwrap();
    let db = SqliteDatabase::connect(&temp_url(&dir)).await.unwrap();
    let pool = db.pool();

    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
        .unwrap();
    let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(pool)
        .await
        .unwrap();
    let fks: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(mode, "wal");
    assert_eq!(timeout, 5000);
    assert_eq!(fks, 1);
}

#[tokio::test]
async fn migrations_are_applied_on_connect() {
    let dir = tempfile::tempdir().unwrap();
    let db = SqliteDatabase::connect(&temp_url(&dir)).await.unwrap();

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(applied >= 1);
}

#[tokio::test]
async fn closed_database_is_unreachable() {
    let dir = tempfile::tempdir().unwrap();
    let db = SqliteDatabase::connect(&temp_url(&dir)).await.unwrap();
    db.pool().close().await;

    assert!(!db.database_reachable().await);
}
