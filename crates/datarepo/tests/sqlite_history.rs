use lodestar_core::person::use_cases::create_person;
use lodestar_datarepo::sqlite::SqliteStore;
use sqlx::SqlitePool;

#[sqlx::test(migrations = "migrations/sqlite")]
async fn change_history_entries_cannot_be_edited_or_deleted(pool: SqlitePool) {
    create_person(&SqliteStore::new(pool.clone()), None, "Ada")
        .await
        .unwrap();

    let update = sqlx::query("UPDATE change_history SET after = '{}'")
        .execute(&pool)
        .await;
    assert!(update.is_err(), "update should be rejected");
    let delete = sqlx::query("DELETE FROM change_history")
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "delete should be rejected");
}
