-- Risks are closed, never deleted (FR-40, FR-42). The score is calculated (FR-41).
CREATE TABLE risks (
    id          TEXT    PRIMARY KEY NOT NULL,
    project_id  TEXT    NOT NULL REFERENCES projects (id),
    number      INTEGER NOT NULL CHECK (number >= 1),
    description TEXT    NOT NULL CHECK (length(description) BETWEEN 1 AND 4000),
    likelihood  INTEGER NOT NULL CHECK (likelihood BETWEEN 1 AND 5),
    impact      INTEGER NOT NULL CHECK (impact BETWEEN 1 AND 5),
    closed      INTEGER NOT NULL CHECK (closed IN (0, 1)),
    version     INTEGER NOT NULL CHECK (version >= 1),
    CONSTRAINT risks_reference_unique UNIQUE (project_id, number)
) STRICT;
