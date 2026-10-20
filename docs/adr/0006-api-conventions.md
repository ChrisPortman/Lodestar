# REST API conventions

The `/api/v1` API uses flat URLs for every resource (`/people`, `/projects`, `/actions`, `/risks`, `/blockers`). Items are created with `project_id` in the body and listed with filters (`?project_id=`). Nested URLs are used only for many-to-many links, added with `POST` and removed with `DELETE`: `/projects/{id}/members[/{person_id}]` and `/blockers/{id}/blocked_actions[/{action_id}]`.

**Updates.** `PUT` replaces a resource and requires every writable field (`null` clears a field). Read-only fields (`id`, `reference`, `project_code`, `version`, `closed`, `archived`) may be included only if they match what's stored: a mismatched `version` gets a 412, any other mismatch a 422. OpenAPI marks these fields `readOnly` instead of defining separate input schemas. Finalising is a boolean flag (`closed`, `cleared`, `archived`, and a Comment's one-way `deleted`) changed only through `PATCH` (`application/merge-patch+json`) with that one field. If other states are needed later, they will be separate fields.

**No deletion.** Nothing is deleted, so `DELETE` on a domain resource returns 405. Finalised resources are still returned by `GET`, and lists leave them out unless `include_closed=true` / `include_archived=true` is passed. An archived Project accepts no writes except `PATCH {"archived": false}`. Deleting a whole Project may be added later.

**Concurrency.** Every resource has a `version`, also sent as an `ETag`, and every write needs a matching `If-Match`: 428 if it's missing, 412 if it's out of date.

**Acting Person.** It is held in a signed, `HttpOnly`, `SameSite=Strict` session cookie, set with `POST /session`, with nothing stored on the server. A write without a valid session gets a 401. Authentication will later use the same session mechanism. There is no request header for the Acting Person.

**Formats.** Keys and query parameters are snake_case. Errors are RFC 9457 problem details. Lists use limit/offset pagination (`{items, total, limit, offset}`) and are ordered by ID (UUIDv7, so creation order) unless `sort` is passed. The UI sorts by Reference by default. Actions, Risks and Blockers carry `reference` and `project_code`. Assignees appear as `{id, display_name}`.

We chose `PATCH`-only finalising and a 405 for `DELETE` over using `DELETE` to close items, so that `DELETE` never means anything other than deletion.
