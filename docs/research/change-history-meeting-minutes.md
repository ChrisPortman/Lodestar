# Can Change History produce Meeting Minutes?

Research for issue #70. Sources are this repo at `main` (c6b06d9).

## Answer

**Yes, with no schema change.** Every Action, Risk, Blocker and Comment change is already written to `change_history` with full `before`/`after` snapshots, the Acting Person, and a timestamp. Minutes for `now()-duration..now()` can be built by reading that window and diffing JSON snapshots in the application. What's missing is a windowed query method, a diff/classification step, and name resolution for People. The query is cheap at expected volumes.

## What exists

- **One row per change, same transaction** as the state change, written by the application layer (`docs/adr/0004-change-history-as-snapshots.md`; `record_history` in `crates/datarepo/src/sqlite.rs`).
- **Columns**: `entity_type`, `entity_id`, `project_id`, `version`, `change_kind` (`created`|`updated`), `acting_person_id`, `recorded_at` (RFC 3339, UTC, microseconds, `Z` suffix), `before`, `after`, `caused_by` (`crates/datarepo/migrations/sqlite/20261019054719_persistence_foundation.up.sql`).
- **Index** `change_history_project (project_id, recorded_at)`: this is the exact index a window query needs (same migration).
- **Entity types** covered: person, project, action, risk, blocker, comment (`crates/core/src/history.rs`, `EntityType`).
- **Snapshots** are `serde` serialisations of the aggregates (`history::snapshot`). Fields:
  - Action: `reference`, `description`, `assignee`, `due_date`, `closed`, `version` (`crates/core/src/action.rs`).
  - Risk: `number`, `description`, `likelihood`, `impact`, `closed`, `version` (`crates/core/src/risk.rs`).
  - Blocker: `reference`, `description`, `accountable`, `cleared`, `blocked_actions`, `version` (`crates/core/src/blocker.rs`).
  - Comment: `project_id`, flattened target (type + id), `author`, `body`, `created_at`, `edited_at`, `deleted`, `version` (`crates/core/src/comment.rs`). Comment entries carry the Project ID, so they show up in a project-scoped query (GLOSSARY: Comment changes are recorded as changes to the Comment).
- **Read port** `HistoryQuery` has `entity_history` (paged) and `project_history(project_id)`, which returns *every* entry for the Project with no time bound (`crates/core/src/ports.rs`; `sqlite.rs` `project_history`).

## Mapping to the Minutes requirements

| Need | Source | Status |
|---|---|---|
| Items created in window | `change_kind = 'created'` | Available |
| Items changed, field-level diff | diff `before` vs `after` JSON | Data available; diff code missing |
| Items closed | `before.closed=false, after.closed=true` (Blocker: `cleared`) | Derivable; no explicit `closed` kind (ADR-0004 restricts kinds to created/updated) |
| Reopened | inverse of the above | Derivable |
| Comment added/edited/deleted | comment entries: created / `body` changed / `deleted` false→true | Derivable |
| Acting Person | `acting_person_id` | ID only; name needs a People lookup |
| Item Reference label (A-001) | snapshot `reference`/`number` | Available; note Risk uses `number`, Action/Blocker use `reference` |
| Cascaded changes (e.g. unassign on Member removal) | `caused_by` | Available, can be grouped or hidden |

## What's missing

1. **Windowed query.** Add e.g. `HistoryQuery::project_history_between(project_id, from, to)` with `WHERE project_id = ? AND recorded_at >= ? AND recorded_at < ?`. Because `recorded_at` is fixed-width RFC 3339 UTC text, lexical comparison is chronological, as long as bounds are formatted the same way (`to_rfc3339_opts(SecondsFormat::Micros, true)`).
2. **Net change per item.** An item edited several times in the window should show first `before` vs last `after`, not every intermediate step. That is a group-by-`entity_id` in Rust over the ordered rows. Changes that net to nothing (edited then reverted) can be dropped. Multiple Acting People per item must be listed as a set.
3. **Field diff + classification** (created / changed / closed / reopened / cleared) in core, over `serde_json::Value`. Since old entries keep the shape they were written in and there is no snapshot schema version (ADR-0004), the diff must tolerate missing/extra keys rather than deserialising into current structs.
4. **Name resolution** for `acting_person_id`, `assignee`, `accountable`, comment `author`, and `blocked_actions` (Action IDs → References). One batched lookup of People and Actions per minutes generation.
5. **Comment context**: a Comment entry knows its target ID, not its Reference; resolve via the target item's current row or its own history.
6. **Snapshotting the result.** Minutes are generated once at Meeting creation, so the rendered output should be stored with the Meeting; the history is immutable, so regenerating later gives the same answer anyway, but storing avoids recomputing and pins the rendering.

## Query cost

- One indexed range scan on `change_history_project (project_id, recorded_at)`: O(log N + k) where k = entries in the window. Planned via the index's leading column equality plus range on the second.
- `before`/`after` are full JSON text per row; for these small aggregates that's roughly 0.5–2 KB per row. A weekly meeting on an active Project might see tens to a few hundred entries: well under a megabyte, single-digit milliseconds on SQLite.
- Diffing and grouping in Rust is linear in k.
- Plus one batched People/Action lookup (primary-key `IN (...)`).
- Generated once per Meeting, so there's no read-amplification concern. By contrast the existing unbounded `project_history` would grow with the Project's whole life and shouldn't be used for this.

## Risks / caveats

- `recorded_at` is the server clock at write time; minutes inherit any clock skew. Window boundaries should be half-open to avoid double counting across consecutive Meetings.
- Changes made by the Meeting creation itself (if it writes history) fall at `now()`; with `< now()` they are excluded.
- Person changes have `project_id = NULL` and won't appear, which is the desired behaviour.
