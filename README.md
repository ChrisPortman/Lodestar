# Lodestar

Lodestar is a small, self-hosted tool for running regular project status meetings. It keeps track of a project's open **Actions**, **Risks** and **Blockers**, so that a project manager or team lead can walk the team through them week to week and keep the project moving.

It is deliberately simple. It is not a replacement for Jira or Microsoft Project, and it doesn't impose any particular methodology.

> **Warning: Lodestar has no authentication yet.** Anyone who can reach it can read and change all data. By default it listens on `127.0.0.1` only. Only expose it on other addresses behind something that controls access, such as a VPN or an authenticating reverse proxy.

## What you can do with it

- **Projects** with a short, unique Project Code (e.g. `WEBR`), a Project Leader and a list of Members. Finished projects are archived, not deleted.
- **Actions**: pieces of work, each assigned to one Member and either open or closed. Open Actions with nobody assigned are flagged for attention.
- **Risks**: things that might harm the project, rated by likelihood and impact.
- **Blockers**: things currently stopping progress, with a Member accountable for clearing each one. A Blocker can be linked to the Actions it blocks.
- **References** such as `A-001`, `R-003` and `B-002`, so everyone in the meeting can refer to the same item.
- **Health Rating**: an overall Red, Amber or Green judgement of the project, with an optional comment.
- **Project on a Page**: a one-page summary of the open Actions, Risks and Blockers plus the Health Rating, for Project Leaders reporting status to managers and stakeholders.
- **Change History**: every change records who made it and when.
- **Project Export**: download a complete copy of a project, including closed items and, optionally, its Change History.

See [GLOSSARY.md](GLOSSARY.md) for the precise meaning of each term.

## Getting started

Lodestar is a single binary for Linux, macOS or Windows with the web interface built in. It stores everything in one SQLite file, so there is no separate database to install.

1. Download the binary for your platform and its `.sha256` file from the [latest release](https://github.com/ChrisPortman/Lodestar/releases). The Linux build needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12, RHEL 9 or later).
2. Verify and install it. On Linux (see [docs/OPERATIONS.md](docs/OPERATIONS.md#install) for macOS and Windows):

   ```sh
   sha256sum -c lodestar-x86_64-unknown-linux-gnu.sha256
   install -m 0755 lodestar-x86_64-unknown-linux-gnu /usr/local/bin/lodestar
   ```

3. Start it:

   ```sh
   lodestar
   ```

4. Open <http://127.0.0.1:8080> in your browser. Choose your name (or add yourself), then create your first project.

With no configuration, Lodestar creates `lodestar.db` (your data) and `lodestar-session.key` (a secret) in the directory you started it from. Keep both safe.

## Running it for a team

[docs/OPERATIONS.md](docs/OPERATIONS.md) covers everything needed to run Lodestar as a service:

- configuration through a TOML file or environment variables,
- the session key,
- an example systemd unit,
- how to back up and restore the database safely while Lodestar is running.

## Getting help and contributing

Found a bug or have an idea? Open an [issue on GitHub](https://github.com/ChrisPortman/Lodestar/issues).

If you'd like to work on Lodestar itself, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Lodestar is released under the [MIT License](LICENSE).
