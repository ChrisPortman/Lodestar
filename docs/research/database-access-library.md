# Database access library: sqlx vs Diesel vs SeaORM

Question ([#4](https://github.com/ChrisPortman/Lodestar/issues/4)): how do sqlx, Diesel and SeaORM compare for Lodestar on SQLite today, adding Postgres/MySQL later behind one repository port, compile-time query checking, migrations on startup, UUIDs in SQLite, async with Tokio, and maintenance?

Researched 2026-10-19. Versions checked on crates.io: sqlx 0.9.0, diesel 2.3.14, diesel-async 0.9.2, sea-orm 2.0.4.

## Comparison

| Concern | sqlx | Diesel (+ diesel-async) | SeaORM |
|---|---|---|---|
| SQLite today | Yes. The `sqlite` feature bundles libsqlite3 (or use `sqlite-unbundled`) [1] | Yes, sync `SqliteConnection` [4] | Yes, through sqlx-sqlite underneath [7] |
| Postgres/MySQL later | Yes: Postgres, MySQL, MariaDB and SQLite [1]. SQL is hand-written, so dialect differences (`?` vs `$1` placeholders, `RETURNING`, upsert syntax) are ours to handle. The `Any` driver exists but loses compile-time checks [2] | Yes, all three backends [4]. The query DSL is backend-generic, so one implementation can serve several backends using generic bounds or the `MultiConnection` derive [5] | Yes, all three. Queries are built with SeaQuery and rendered for each backend at runtime, so one implementation works on every backend through `DatabaseConnection` [7] |
| Compile-time checking | The `query!`/`query_as!` macros check SQL at build time, either against a live database or against offline metadata that `cargo sqlx prepare` writes to `.sqlx/`. The metadata is used when `SQLX_OFFLINE=true` or `DATABASE_URL` is unset [2][3]. Each query is checked against one backend | The Rust compiler type-checks queries against `schema.rs`, which `diesel print-schema` generates and you commit. No database is needed at build time [4] | No SQL checking. Entity types give compile-time safety for columns and types, but whether a query is valid is only found out at runtime [7] |
| Migrations on startup | `sqlx::migrate!()` embeds `./migrations` in the binary; call `.run(&pool).await` at startup [3] | `diesel_migrations::embed_migrations!` + `run_pending_migrations`. These are sync, so with diesel-async they run in `spawn_blocking` or through `AsyncMigrationHarness` [6] | `sea-orm-migration`: migrations are written in Rust and run with `Migrator::up(&db, None)` at startup [8] |
| UUIDs in SQLite | The `uuid` feature stores `Uuid` as a 16-byte BLOB; `uuid::fmt::Hyphenated` stores it as TEXT [2] | No built-in `Uuid` support for SQLite (the `uuid` feature only maps Postgres `UUID`). Needs a custom `ToSql`/`FromSql` newtype over `Binary` or `Text` [4] | `Uuid` column type is supported; on SQLite it is stored as a BLOB via sqlx [7] |
| Async with Tokio | Natively async, with the `runtime-tokio` feature [1] | Core Diesel is sync. `diesel-async` is natively async for Postgres/MySQL, but on SQLite it only runs through `SyncConnectionWrapper` on a blocking thread [5] | Natively async, with Tokio runtime features [7] |
| Maintenance | Active. launchbadge/sqlx released 0.9.0 in May 2026 and has commits up to Oct 2026 [9] | Active. diesel-rs/diesel ships 2.3.x patch releases (latest Oct 2026). diesel-async is maintained separately by the same lead [9] | Active. SeaQL/sea-orm is on the 2.0 line, with a release in Sep 2026 and commits in Oct 2026 [9] |

## Analysis for Lodestar

- Lodestar wants a repository port with SQLite now and Postgres/MySQL later. With sqlx, each backend gets its own adapter (or carefully portable SQL), and each needs its own compile-time check metadata. That means more code per backend, but the code is plain SQL and the port boundary already keeps the backends apart.
- Diesel gives the strongest compile-time safety without needing a database at build time, and supports generic multi-backend code. Its weak points for Lodestar are that SQLite is sync-only (async only through a wrapper) and that UUIDs in SQLite need a custom type.
- SeaORM is the easiest way to run one implementation on all three backends. In exchange, it gives up compile-time query checking and adds an ORM layer and a migration system written in a Rust DSL.
- UUIDs (ADR-0001): sqlx and SeaORM store `Uuid` on SQLite as a BLOB with no extra work. Whether to use BLOB or TEXT is a separate decision; TEXT is easier to read when inspecting the database by hand.

## Recommendation

Use **sqlx** with the `runtime-tokio`, `sqlite`, `uuid`, `migrate` and `macros` features. Write the SQLite repository adapter with `query!`/`query_as!` and commit the `.sqlx/` offline metadata, so that builds and CI need no database (`SQLX_OFFLINE=true`). Embed migrations with `sqlx::migrate!()` and run them at startup. When Postgres/MySQL arrive, add an adapter for each backend behind the same port, each with its own migrations directory. sqlx is natively async on Tokio, fits axum, and stores `Uuid` directly. Diesel is the strongest alternative if we want one generic implementation for every backend, but on SQLite it is sync-only and needs a custom UUID type.

## Sources

1. sqlx README: https://github.com/launchbadge/sqlx/blob/main/README.md
2. sqlx docs (`query!`, offline mode, `Any`, SQLite types): https://docs.rs/sqlx/latest/sqlx/macro.query.html, https://docs.rs/sqlx/latest/sqlx/sqlite/types/index.html
3. sqlx-cli README (`cargo sqlx prepare`) and `migrate!`: https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md, https://docs.rs/sqlx/latest/sqlx/macro.migrate.html
4. Diesel docs and guides: https://docs.rs/diesel/latest/diesel/, https://diesel.rs/guides/
5. diesel-async README (`SyncConnectionWrapper` for SQLite): https://github.com/weiznich/diesel_async; Diesel `MultiConnection`: https://docs.rs/diesel/latest/diesel/derive.MultiConnection.html
6. diesel_migrations: https://docs.rs/diesel_migrations/latest/diesel_migrations/
7. SeaORM docs: https://www.sea-ql.org/SeaORM/docs/index/
8. SeaORM migrations: https://www.sea-ql.org/SeaORM/docs/migration/running-migration/
9. crates.io and GitHub API metadata (versions, last release and push dates), fetched 2026-10-19.
