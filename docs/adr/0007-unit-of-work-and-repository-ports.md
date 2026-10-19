# Unit of work, repository and query ports

`lodestar-core` defines a **unit of work** port. `begin()` returns a transaction that gives access to the repository for each aggregate. `commit()` commits it, and dropping it without committing rolls back. Each use case runs in one unit of work, which is how rules spanning several aggregates (e.g. removing a Member and unassigning their items) commit or roll back together. The adapters back it directly with a database transaction; in SQLite that is `BEGIN IMMEDIATE`, with WAL mode, a 5-second `busy_timeout` and foreign keys on.

**Repositories** only `load` and `save` aggregates. `save(before, after, ctx)` checks the version (`UPDATE … WHERE id = ? AND version = ?`, with the version going up by one), writes the state, and writes the Change History entry (ADR-0004) using the Acting Person and optional `caused_by` from `ctx`. History therefore can't be skipped by a use case.

**References** are allocated from a counter table keyed by `(project_id, kind)` (`UPDATE … SET next = next + 1 RETURNING next`) inside the same transaction, with a unique constraint as a safety net.

**Reads** go through separate read-only **query** ports, outside any transaction. They return ready-made views (filtered and paged, including `project_code` and the assignee's `display_name`), served from the same tables with joins.

Ports return `NotFound`, `Conflict`, `UniqueViolation { constraint }` or `Unexpected`, so sqlx types never reach core. The traits use Rust's built-in `async fn` with `Send` futures, and use cases are generic over the unit of work (resolved at compile time, no `async_trait` and no boxed trait objects).

We rejected passing a transaction handle into each repository method, because that exposes sqlx to core. We rejected one transaction per save, because it can't cover changes spanning several aggregates.
