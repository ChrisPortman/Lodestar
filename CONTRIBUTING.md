# Contributing to Lodestar

Thanks for your interest in working on Lodestar. This guide covers how the codebase is laid out, how to build and test it, and the conventions we follow.

## Before you start

Read these first. They are short, and they explain the decisions the code is built on:

- [docs/OBJECTIVE.md](docs/OBJECTIVE.md): what Lodestar is for, and what it deliberately is not.
- [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md): functional and non-functional requirements. Code and tests refer to them by ID (e.g. `NFR-33`).
- [GLOSSARY.md](GLOSSARY.md): the domain language. Use these terms, and avoid the listed alternatives, in code, UI text, issues and commit messages. A **Reference** (`A-001`) is not an identifier, an **Action** is not a task, and so on.
- [docs/adr/](docs/adr/): architecture decision records. If a change goes against an ADR, raise it in an issue first.

Work is tracked in [GitHub Issues](https://github.com/ChrisPortman/Lodestar/issues). Please open or comment on an issue before starting anything substantial.

## Prerequisites

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | pinned in [`rust-toolchain.toml`](rust-toolchain.toml) | `rustup` installs it automatically, with `rustfmt` and `clippy`. |
| Node.js | pinned in [`.nvmrc`](.nvmrc) | `nvm use` picks it up. |
| [`just`](https://github.com/casey/just) | any recent | The task runner. Every CI step is a `just` recipe. |
| [`sqlx-cli`](https://crates.io/crates/sqlx-cli) | any recent | For migrations and `just sqlx-check`. `cargo install sqlx-cli --no-default-features --features sqlite`. |
| [`cargo-audit`](https://crates.io/crates/cargo-audit) | optional | For `just audit-rust`. |
| `sqlite3` | optional | Handy for inspecting the database. |

## Repository layout

```
crates/
  core/        lodestar-core: domain types, use cases and the port traits
  datarepo/    lodestar-datarepo: repository implementations (SQLite), migrations, .sqlx metadata
  api/         lodestar-api: axum handlers, OpenAPI (utoipa), serving the embedded frontend
  lodestar/    the binary: config, wiring and startup
frontend/      Vue 3 + TypeScript + Vite + Tailwind single-page app
  src/api/     generated API types (schema.d.ts), committed
  e2e/         Playwright smoke tests
openapi/       openapi.json, generated from the backend, committed
docs/          objective, requirements, operations guide, ADRs, deploy examples
```

The backend follows a ports-and-adapters layout ([ADR-0005](docs/adr/0005-workspace-layout.md), [ADR-0007](docs/adr/0007-unit-of-work-and-repository-ports.md)). Dependencies point inward: `api` and `datarepo` depend on `core`, never the other way round. `core` does no IO; it defines the repository traits, and `datarepo` implements them.

## Building and running

```sh
just build     # install frontend deps, build frontend/dist, then the release binary that embeds it
just dev       # backend on :8080, plus the Vite dev server with hot reload, proxying /api to it
```

The API crate embeds `frontend/dist` at compile time, so the frontend must be built at least once before the backend will compile.

For the dev loop, open the URL the Vite dev server prints. Configuration for the backend is described in [docs/OPERATIONS.md](docs/OPERATIONS.md); `log.format = "pretty"` (or `LODESTAR_LOG__FORMAT=pretty`) is easier to read locally.

## Testing and checks

```sh
just test      # cargo test --workspace, then the frontend unit tests (Vitest)
just lint      # rustfmt, clippy (warnings are errors), eslint, prettier and vue-tsc
just e2e       # build the release binary and run the Playwright smoke tests against it
```

CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs the same recipes, plus:

- `just sqlx-check`: fails if the committed sqlx query metadata in `crates/datarepo/.sqlx` is out of date.
- `just contract-check`: fails if the committed OpenAPI document or generated TypeScript types are out of date.
- `just audit-rust` and `just audit-npm`: dependency advisories.

Run whichever of these your change could affect before opening a pull request.

Where tests live:

- Backend unit tests sit next to the code; integration tests are in each crate's `tests/` directory. Use cases in `core` are tested against fakes; `datarepo` has contract tests that run against real SQLite.
- Frontend unit and component tests use Vitest, Testing Library and MSW.
- End-to-end smoke tests in `frontend/e2e` drive the real binary with Playwright.

## Common tasks

### Changing the API

The OpenAPI document is generated from the Rust code with utoipa ([ADR-0002](docs/adr/0002-code-first-openapi-with-utoipa.md)), and the frontend's types are generated from it. After changing a handler or its types:

```sh
just contract
```

Commit the regenerated `openapi/openapi.json` and `frontend/src/api/schema.d.ts` with your change. Follow the conventions in [ADR-0006](docs/adr/0006-api-conventions.md).

### Changing the database schema

Create migrations with sqlx-cli so they get a timestamp prefix:

```sh
cargo sqlx migrate add -r <name> --source crates/datarepo/migrations/sqlite
```

Never hand-number migrations (`0002_…`). Work happens on parallel branches, and numbered names collide when they merge.

Queries use sqlx's compile-time checked macros. After adding or changing a query, refresh the offline metadata against a migrated database and commit `crates/datarepo/.sqlx`:

```sh
export DATABASE_URL=sqlite://$(mktemp -d)/dev.db
sqlx database create
sqlx migrate run --source crates/datarepo/migrations/sqlite
cd crates/datarepo && cargo sqlx prepare -- --all-targets
```

Builds otherwise use `SQLX_OFFLINE=true` and need no database.

### Adding dependencies

Every dependency is a supply-chain risk, so keep them few and justify new ones in the pull request. If an advisory has to be ignored, add it to `.cargo/audit.toml` or `frontend/audit-ignore.json` with the reason.

## Pull requests

1. Branch from `main`.
2. Keep each pull request focused on one issue, and reference it (e.g. `Closes #42`).
3. Write commit messages in the imperative mood, describing what the change does for the user or the codebase.
4. Make sure `just lint` and `just test` pass, along with any of the checks above that your change touches.
5. Update documentation when behaviour changes: [docs/OPERATIONS.md](docs/OPERATIONS.md) for anything an operator sees, and the glossary or an ADR for changes to the domain or architecture.

## Releasing

Maintainers release by pushing a `v*` tag. [`release.yml`](.github/workflows/release.yml) runs CI on the tagged commit, builds a binary per platform with `just release <target>`, and publishes them with their checksums as a GitHub Release.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
