-- Meetings (ADR-0006): created once, when the leader ends one, and never
-- changed. `minutes` is the frozen, fully resolved Minutes JSON document.
-- Times are RFC 3339 UTC to the microsecond, so text order is time order.
CREATE TABLE meetings (
    id         TEXT    PRIMARY KEY NOT NULL,
    project_id TEXT    NOT NULL REFERENCES projects (id),
    leader_id  TEXT    NOT NULL REFERENCES people (id),
    started_at TEXT    NOT NULL,
    ended_at   TEXT    NOT NULL,
    minutes    TEXT    NOT NULL CHECK (json_valid(minutes)),
    version    INTEGER NOT NULL CHECK (version >= 1)
) STRICT;

CREATE INDEX meetings_project ON meetings (project_id, started_at, id);

-- A Meeting's attendees, in order: the leader first.
CREATE TABLE meeting_attendees (
    meeting_id TEXT    NOT NULL REFERENCES meetings (id),
    person_id  TEXT    NOT NULL REFERENCES people (id),
    position   INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (meeting_id, person_id),
    UNIQUE (meeting_id, position)
) STRICT;

-- Comments may now be about a Meeting. SQLite cannot change a CHECK, so the
-- table is rebuilt; nothing refers to it.
CREATE TABLE comments_new (
    id          TEXT    PRIMARY KEY NOT NULL,
    project_id  TEXT    NOT NULL REFERENCES projects (id),
    target_type TEXT    NOT NULL
        CHECK (target_type IN ('project', 'action', 'risk', 'blocker', 'meeting')),
    target_id   TEXT    NOT NULL,
    author_id   TEXT    NOT NULL REFERENCES people (id),
    body        TEXT    NOT NULL CHECK (length(body) BETWEEN 1 AND 4000),
    created_at  TEXT    NOT NULL,
    edited_at   TEXT,
    deleted     INTEGER NOT NULL CHECK (deleted IN (0, 1)),
    version     INTEGER NOT NULL CHECK (version >= 1)
) STRICT;

INSERT INTO comments_new SELECT * FROM comments;
DROP TABLE comments;
ALTER TABLE comments_new RENAME TO comments;

CREATE INDEX comments_project ON comments (project_id, created_at, id);
CREATE INDEX comments_target ON comments (target_type, target_id, created_at, id);
