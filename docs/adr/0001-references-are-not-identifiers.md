# UUID identifiers; References are display labels only

Every entity uses a UUIDv7 (time-ordered) as its primary key and as its identifier in the API. Actions, Risks and Blockers also show a human-readable Reference (e.g. A-0001), numbered per Project. Because every Project has its own A-0001, a Reference isn't unique across the system, so it is never used as a key or in the API. It is only a label, shown alongside the Project Code wherever confusion could arise. UUIDs also avoid exposing row counts, and they keep IDs portable across the SQLite to Postgres/MySQL move.
