-- Meeting Comments go with their Meetings.
CREATE TABLE comments_old (
    id          TEXT    PRIMARY KEY NOT NULL,
    project_id  TEXT    NOT NULL REFERENCES projects (id),
    target_type TEXT    NOT NULL CHECK (target_type IN ('project', 'action', 'risk', 'blocker')),
    target_id   TEXT    NOT NULL,
    author_id   TEXT    NOT NULL REFERENCES people (id),
    body        TEXT    NOT NULL CHECK (length(body) BETWEEN 1 AND 4000),
    created_at  TEXT    NOT NULL,
    edited_at   TEXT,
    deleted     INTEGER NOT NULL CHECK (deleted IN (0, 1)),
    version     INTEGER NOT NULL CHECK (version >= 1)
) STRICT;

INSERT INTO comments_old SELECT * FROM comments WHERE target_type <> 'meeting';
DROP TABLE comments;
ALTER TABLE comments_old RENAME TO comments;

CREATE INDEX comments_project ON comments (project_id, created_at, id);
CREATE INDEX comments_target ON comments (target_type, target_id, created_at, id);

DROP TABLE meeting_attendees;
DROP TABLE meetings;
