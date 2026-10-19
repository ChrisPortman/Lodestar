-- People exist across the whole system and are never deleted (FR-01, FR-05).
CREATE TABLE people (
    id           TEXT    PRIMARY KEY NOT NULL,
    display_name TEXT    NOT NULL CHECK (length(display_name) BETWEEN 1 AND 100),
    version      INTEGER NOT NULL CHECK (version >= 1)
) STRICT;

CREATE INDEX people_display_name ON people (display_name, id);

-- One shared Change History table, written by the application (ADR-0004).
CREATE TABLE change_history (
    id               TEXT    PRIMARY KEY NOT NULL,
    entity_type      TEXT    NOT NULL,
    entity_id        TEXT    NOT NULL,
    project_id       TEXT,
    version          INTEGER NOT NULL,
    change_kind      TEXT    NOT NULL CHECK (change_kind IN ('created', 'updated')),
    acting_person_id TEXT    NOT NULL,
    recorded_at      TEXT    NOT NULL,
    before           TEXT,
    after            TEXT    NOT NULL,
    caused_by        TEXT    REFERENCES change_history (id),
    UNIQUE (entity_type, entity_id, version)
) STRICT;

CREATE INDEX change_history_project ON change_history (project_id, recorded_at);

-- Entries are never edited or deleted (FR-81). These guard; they don't write.
CREATE TRIGGER change_history_no_update BEFORE UPDATE ON change_history
BEGIN
    SELECT RAISE(ABORT, 'change history entries are never edited');
END;

CREATE TRIGGER change_history_no_delete BEFORE DELETE ON change_history
BEGIN
    SELECT RAISE(ABORT, 'change history entries are never deleted');
END;

-- Reference allocation per (project, kind) (ADR-0001, ADR-0007).
CREATE TABLE reference_counters (
    project_id TEXT    NOT NULL,
    kind       TEXT    NOT NULL CHECK (kind IN ('action', 'risk', 'blocker')),
    next       INTEGER NOT NULL CHECK (next >= 1),
    CONSTRAINT reference_counters_unique UNIQUE (project_id, kind)
) STRICT;
