-- Meeting Locks (ADR-0006): at most one per Project, held by one browser
-- session. Not a domain resource: no version, no Change History, and
-- cancelling really deletes the row. Times are RFC 3339 UTC to the
-- microsecond, recorded by the server.
CREATE TABLE meeting_locks (
    project_id   TEXT PRIMARY KEY NOT NULL REFERENCES projects (id),
    session_id   TEXT NOT NULL,
    leader_id    TEXT NOT NULL REFERENCES people (id),
    acquired_at  TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL
) STRICT;
