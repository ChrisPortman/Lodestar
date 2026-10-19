use lodestar_datarepo::sqlite::SqliteStore;
use sqlx::SqlitePool;

lodestar_core::port_contract_tests!(
    #[sqlx::test(migrations = "migrations/sqlite")],
    (pool: SqlitePool) => SqliteStore::new(pool)
);
