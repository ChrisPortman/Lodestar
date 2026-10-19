default: build

# Build the frontend, then the binary that embeds it.
build: frontend
    cargo build --release

# Install frontend dependencies and build frontend/dist.
frontend:
    cd frontend && npm ci && npm run build

test:
    cargo test --workspace
    cd frontend && npm test

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cd frontend && npm run lint && npm run typecheck

# --- CI recipes: each step in .github/workflows/ci.yml calls one of these. ---

frontend-ci:
    cd frontend && npm ci

frontend-lint:
    cd frontend && npm run lint

frontend-typecheck:
    cd frontend && npm run typecheck

frontend-test:
    cd frontend && npm test

frontend-build:
    cd frontend && npx vite build

fmt-check:
    cargo fmt --all --check

clippy:
    SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings

backend-test:
    SQLX_OFFLINE=true cargo test --workspace

# Create and migrate a scratch SQLite DB, then fail if crates/datarepo/.sqlx is stale.
sqlx-check:
    #!/usr/bin/env bash
    set -euo pipefail
    db="$(mktemp -d)/lodestar-sqlx.db"
    export DATABASE_URL="sqlite://$db"
    sqlx database create
    sqlx migrate run --source crates/datarepo/migrations/sqlite
    cd crates/datarepo && cargo sqlx prepare --check -- --all-targets

# Regenerate openapi/openapi.json and frontend/src/api/schema.d.ts.
contract:
    SQLX_OFFLINE=true cargo run -q -p lodestar -- openapi openapi/openapi.json
    cd frontend && npx openapi-typescript ../openapi/openapi.json -o src/api/schema.d.ts && npx prettier --write src/api/schema.d.ts

# Fail if the generated API contract differs from what is committed (NFR-62).
contract-check: contract
    git diff --exit-code -- openapi frontend/src/api

# Ignored advisories live in .cargo/audit.toml, each with a reason (NFR-33).
audit-rust:
    cargo audit

# `npm audit --audit-level=high`, minus advisories in frontend/audit-ignore.json.
audit-npm:
    cd frontend && node scripts/npm-audit.mjs

# Backend on :8080 and Vite dev server proxying /api to it.
dev:
    cargo run -p lodestar & cd frontend && npm run dev
