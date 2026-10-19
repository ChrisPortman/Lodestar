-- Projects are archived, never deleted (FR-14, FR-15). The code never changes (FR-11).
CREATE TABLE projects (
    id                TEXT    PRIMARY KEY NOT NULL,
    name              TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    description       TEXT    CHECK (length(description) BETWEEN 1 AND 4000),
    project_code      TEXT    NOT NULL CHECK (length(project_code) BETWEEN 2 AND 10),
    archived          INTEGER NOT NULL CHECK (archived IN (0, 1)),
    project_leader_id TEXT    REFERENCES people (id),
    health_rating     TEXT    CHECK (health_rating IN ('red', 'amber', 'green')),
    health_comment    TEXT    CHECK (length(health_comment) BETWEEN 1 AND 4000),
    version           INTEGER NOT NULL CHECK (version >= 1),
    CONSTRAINT projects_project_code_unique UNIQUE (project_code),
    CHECK (health_comment IS NULL OR health_rating IS NOT NULL)
) STRICT;

CREATE INDEX projects_name ON projects (name, id);

CREATE TABLE project_members (
    project_id TEXT NOT NULL REFERENCES projects (id),
    person_id  TEXT NOT NULL REFERENCES people (id),
    PRIMARY KEY (project_id, person_id)
) STRICT;
