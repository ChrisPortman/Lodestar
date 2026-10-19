# Running Lodestar

Lodestar ships as a single Linux binary with the web UI built in. It stores everything in one SQLite database.

> **Warning: the MVP has no authentication.** Anyone who can reach the port can read and change all data. By default Lodestar listens on `127.0.0.1` only. Binding it to any other address is a deliberate choice: only do it behind something that controls access, such as a VPN or an authenticating reverse proxy.

## Install

Each [GitHub Release](https://github.com/ChrisPortman/Lodestar/releases) has `lodestar-x86_64-unknown-linux-gnu` and its `.sha256`. The binary needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12, RHEL 9 or later).

```sh
sha256sum -c lodestar-x86_64-unknown-linux-gnu.sha256
install -m 0755 lodestar-x86_64-unknown-linux-gnu /usr/local/bin/lodestar
lodestar --version
```

To build it yourself, run `just release`. The binary and checksum land in `dist/`.

## Run

```sh
lodestar                          # uses ./lodestar.toml if it exists, else defaults
lodestar --config /etc/lodestar/lodestar.toml
curl http://127.0.0.1:8080/api/v1/healthz
```

With no configuration, Lodestar listens on `127.0.0.1:8080`, and creates `lodestar.db` and `lodestar-session.key` in the working directory.

Exit codes: `64` means a bad command line, and `78` means invalid configuration. Both are logged with the offending key. On SIGTERM or SIGINT, Lodestar stops accepting connections, finishes the requests in flight, closes the database and exits `0`.

## Configuration

Settings come from three places. Later ones win:

1. Built-in defaults.
2. A TOML file: `--config <path>`, else `$LODESTAR_CONFIG`, else `./lodestar.toml` if present. A file named explicitly must exist.
3. Environment variables `LODESTAR_<SECTION>__<KEY>` (note the double underscore), e.g. `LODESTAR_SERVER__PORT=9000` or `LODESTAR_DATABASE__URL=sqlite:///srv/lodestar.db`.

[`deploy/lodestar.toml`](deploy/lodestar.toml) lists every key with its default:

| Key | Default | |
| --- | --- | --- |
| `server.bind` | `127.0.0.1` | See the warning above. |
| `server.port` | `8080` | |
| `database.url` | `sqlite://lodestar.db` | Relative to the working directory. |
| `log.level` | `info` | A `tracing` filter, e.g. `info,lodestar_api=debug`. |
| `log.format` | `json` | `json` or `pretty`. |
| `session.key_file` | `lodestar-session.key` | See below. |
| `session.key` | unset | Base64 key. Overrides `key_file`. |

Unknown keys are rejected.

### Session key

The session key signs and encrypts the session cookie. It must be at least 32 bytes once base64-decoded.

- If `session.key` (or `LODESTAR_SESSION__KEY`) is set, it is used.
- Otherwise Lodestar reads `session.key_file`. If the file does not exist, Lodestar generates a 64-byte key and writes it there with mode `0600`.

Treat the key as a secret. Keep it out of version control and keep it in your backups next to the database. If the key is lost or changed, everyone is signed out and must pick their name again; no project data is lost. To rotate it, stop Lodestar, delete the file, and start Lodestar again. To provide your own key, generate one with:

```sh
head -c 64 /dev/urandom | base64 -w0 > lodestar-session.key && chmod 600 lodestar-session.key
```

## systemd

[`deploy/lodestar.service`](deploy/lodestar.service) is an example unit. It runs as a dynamic user, keeps state in `/var/lib/lodestar`, and stops restarting on exit codes 64 and 78.

```sh
install -D -m 0644 docs/deploy/lodestar.toml /etc/lodestar/lodestar.toml
install -m 0644 docs/deploy/lodestar.service /etc/systemd/system/lodestar.service
systemctl daemon-reload
systemctl enable --now lodestar
journalctl -u lodestar -f
```

## Backups

The database runs in WAL mode, so recent writes may be in `lodestar.db-wal` rather than `lodestar.db`. **Do not back up the database by copying the file while Lodestar is running**: the copy may be inconsistent. Use SQLite's online backup instead. Both commands below are safe while Lodestar runs:

```sh
sqlite3 /var/lib/lodestar/lodestar.db ".backup '/backups/lodestar-$(date +%F).db'"
# or, which also compacts the copy:
sqlite3 /var/lib/lodestar/lodestar.db "VACUUM INTO '/backups/lodestar-$(date +%F).db'"
```

Check a backup with `sqlite3 <backup> "PRAGMA integrity_check"`, which should print `ok`. Back up the session key file as well.

To restore, stop Lodestar, replace `lodestar.db` with the backup, delete any `lodestar.db-wal` and `lodestar.db-shm` next to it, and start Lodestar again.

## Releasing

Pushing a `v*` tag runs [`release.yml`](../.github/workflows/release.yml): it runs CI on the tagged commit, builds with `just release` on Ubuntu 22.04, and publishes a GitHub Release with the binary and its checksum.
