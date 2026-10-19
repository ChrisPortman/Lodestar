# Workspace layout: core, datarepo, api, binary

The backend is a Cargo workspace with four crates, all `publish = false`:

- `lodestar-core` (`crates/core`): the domain and its use cases, plus the port traits. It is organised by part of the domain (`project`, `action`, `risk`, `blocker`, `person`, `history`).
- `lodestar-datarepo` (`crates/datarepo`): implements the repository ports, with one module per backend behind a Cargo feature (`sqlite`, the default). Migrations live in `migrations/<backend>/`, alongside the sqlx `.sqlx/` metadata.
- `lodestar-api` (`crates/api`): axum handlers, utoipa, and serving the embedded `frontend/dist`.
- `lodestar` (`crates/lodestar`): the binary, covering config, wiring and startup.

Dependencies point inward: `api` → `core` and `datarepo` → `core`. The port traits live in `core`, not `datarepo`, because the traits use domain types and the use cases call the traits. Putting them in `datarepo` would create a dependency cycle.

We combined the domain and its use cases in one crate, accepting that "the domain does no IO" is enforced by convention rather than by the compiler.

Known consequence: sqlx's compile-time checked macros check against one database per crate. Only SQLite uses them in the MVP. When a second backend is added, either move it to its own crate (`lodestar-datarepo-<backend>`) to keep compile-time checks, or write its queries unchecked and rely on integration tests.

Also decided: `frontend/` is a standalone Vite project at the repo root; `lodestar openapi` writes `openapi/openapi.json` (committed); the TypeScript types are generated into `frontend/src/api/schema.d.ts` (committed); and `just` is the task runner.
