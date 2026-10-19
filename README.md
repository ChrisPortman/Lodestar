# Lodestar

A small, self-hosted project manager for running project meetings. It ships as a single Linux binary with the web UI built in, backed by SQLite.

> **The MVP has no authentication.** It listens on `127.0.0.1` by default. Exposing it beyond loopback is a deliberate choice: see [docs/OPERATIONS.md](docs/OPERATIONS.md).

- **Run it:** [docs/OPERATIONS.md](docs/OPERATIONS.md) covers install, configuration, the session key, systemd and backups.
- **Build it:** `just build` for development, `just release` for the release binary and checksum in `dist/`. `just test` and `just lint` run the checks; CI runs the same `just` recipes.
- **Requirements and design:** [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md), [docs/adr/](docs/adr/), [GLOSSARY.md](GLOSSARY.md).
