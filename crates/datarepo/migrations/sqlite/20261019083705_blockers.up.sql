-- Blockers (FR-50 to FR-54). The Reference number is a label only (ADR-0001).
CREATE TABLE blockers (
    id             TEXT    PRIMARY KEY NOT NULL,
    project_id     TEXT    NOT NULL REFERENCES projects (id),
    reference      INTEGER NOT NULL CHECK (reference >= 1),
    description    TEXT    NOT NULL CHECK (length(description) BETWEEN 1 AND 4000),
    accountable_id TEXT    REFERENCES people (id),
    cleared        INTEGER NOT NULL CHECK (cleared IN (0, 1)),
    version        INTEGER NOT NULL CHECK (version >= 1),
    CONSTRAINT blockers_reference_unique UNIQUE (project_id, reference)
) STRICT;

CREATE INDEX blockers_accountable ON blockers (project_id, accountable_id) WHERE cleared = 0;

-- The Actions each Blocker blocks; part of the Blocker aggregate.
CREATE TABLE blocker_actions (
    blocker_id TEXT NOT NULL REFERENCES blockers (id),
    action_id  TEXT NOT NULL REFERENCES actions (id),
    PRIMARY KEY (blocker_id, action_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX blocker_actions_action ON blocker_actions (action_id);
