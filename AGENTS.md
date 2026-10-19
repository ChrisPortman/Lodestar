# Lodestar

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Implementation conventions

### Migrations

Create migrations with `cargo sqlx migrate add -r <name> --source crates/datarepo/migrations/sqlite`, which gives them timestamp prefixes. Never hand-number them (`0002_…`): tickets are built in parallel branches, and numbered names collide when they merge.
