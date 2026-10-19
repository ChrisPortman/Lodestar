-- Actions (FR-30 to FR-33). The Reference number is a label only (ADR-0001).
CREATE TABLE actions (
    id          TEXT    PRIMARY KEY NOT NULL,
    project_id  TEXT    NOT NULL REFERENCES projects (id),
    reference   INTEGER NOT NULL CHECK (reference >= 1),
    description TEXT    NOT NULL CHECK (length(description) BETWEEN 1 AND 4000),
    assignee_id TEXT    REFERENCES people (id),
    due_date    TEXT    CHECK (due_date IS NULL OR date(due_date) = due_date),
    closed      INTEGER NOT NULL CHECK (closed IN (0, 1)),
    version     INTEGER NOT NULL CHECK (version >= 1),
    CONSTRAINT actions_reference_unique UNIQUE (project_id, reference)
) STRICT;

CREATE INDEX actions_assignee ON actions (project_id, assignee_id) WHERE closed = 0;
