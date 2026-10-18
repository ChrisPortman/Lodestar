# Embedding the Vue frontend in the Rust binary

Research for [issue #5](https://github.com/ChrisPortman/Lodestar/issues/5). Context: NFR-03 requires the Vue/Tailwind SPA build output to be embedded in, and served from, the axum binary.

## 1. rust-embed vs include_dir vs include_bytes!

| | `include_bytes!` | `include_dir` | `rust-embed` |
|---|---|---|---|
| What it is | std macro, one file | Macro embedding a directory tree as a static `Dir` | Derive macro embedding a folder |
| Many hashed Vite filenames | Impractical: one macro per file, names change every build | Yes | Yes |
| Debug builds | Always embedded | Always embedded | **Reads from disk by default in debug** (no recompile for frontend changes); `debug-embed` feature forces embedding |
| Metadata | None | Optional (`metadata` feature) | `sha256_hash`, `last_modified`, MIME (`mime-guess` feature) via `EmbeddedFile::metadata` |
| Compression | No | No | `compression` feature (stores gzip-compressed in the binary, decompresses at access) |
| Include/exclude globs | No | Limited | `#[include]` / `#[exclude]` attributes |
| axum integration | Hand-rolled | Hand-rolled | Hand-rolled (example in repo) or `axum-embed` crate built on it |

Sources: [std `include_bytes!`](https://doc.rust-lang.org/std/macro.include_bytes.html); [include_dir docs](https://docs.rs/include_dir); [rust-embed docs](https://docs.rs/rust-embed) and [README/features](https://github.com/pyrossh/rust-embed) (debug reads from filesystem, `debug-embed`, `compression`, `mime-guess`, `include-exclude`, `EmbeddedFile.metadata.sha256_hash()`), [rust-embed axum example](https://github.com/pyrossh/rust-embed/blob/master/examples/axum.rs); [axum-embed](https://docs.rs/axum-embed) (ServeEmbed with SPA fallback, ETag, precompressed `.br`/`.gz` negotiation).

**Verdict: rust-embed.** It handles Vite's hashed filenames, gives a free dev-loop mode (disk in debug), and exposes a SHA-256 for ETags. `include_bytes!` is unsuitable for a directory of generated names; `include_dir` works but offers nothing extra.

Note: the embedded folder must exist at compile time or the derive fails. Commit a placeholder `frontend/dist/.gitkeep` or have CI build the frontend first.

## 2. Should build.rs run the Vite build?

Cargo's guidance is that build scripts should only touch `OUT_DIR` and declare inputs with `cargo::rerun-if-changed` ([Cargo book: build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)). Running `npm ci && vite build` from build.rs:

- adds a Node toolchain requirement to every `cargo check`/`clippy`/rust-analyzer run, slowing the IDE;
- writes outside `OUT_DIR` (Vite emits to `dist/`) unless reconfigured;
- needs a large, fragile `rerun-if-changed` set (all of `src/`, `package.json`, lockfile, Vite config);
- hides frontend failures inside Cargo output.

**Verdict: separate step.** Build the frontend first (`npm run build`), then `cargo build --release`. Wire them together in one task runner entry (`just build` / Makefile) and in CI, and in the Dockerfile as a multi-stage build (Node stage produces `dist/`, Rust stage copies it in). Optionally a tiny build.rs that only emits `cargo::rerun-if-changed=frontend/dist` so release builds re-embed when `dist` changes (rust-embed also tracks this itself in release mode).

## 3. Dev loop

Two good options, used together:

- **Day-to-day UI work: Vite dev server proxying `/api` to axum.** Vite's `server.proxy` ([Vite docs: server.proxy](https://vite.dev/config/server-options.html#server-proxy)) gives HMR and instant feedback; axum runs via `cargo run` (or `cargo watch`/`bacon`). The browser talks only to Vite, so no CORS config is needed.
- **Checking the integrated binary: `cargo run` alone.** rust-embed's debug mode serves `frontend/dist` from disk, so `npm run build -- --watch` plus `cargo run` exercises the real serving path (fallback, headers) without recompiling Rust.

Don't add CORS to axum just for development; the proxy avoids it.

## 4. SPA route fallback

Vue Router's HTML5 history mode needs the server to return `index.html` for unknown non-file paths ([Vue Router: HTML5 mode server config](https://router.vuejs.org/guide/essentials/history-mode.html)). In axum:

- Nest the API under `/api/v1` (and the OpenAPI doc) first, and give `/api/*` its own JSON 404 fallback so mistyped API calls never get HTML.
- Router `.fallback(static_handler)`([axum `Router::fallback`](https://docs.rs/axum/latest/axum/struct.Router.html#method.fallback)): look up the path in the embed; if found, serve it; if not found **and** the path has no file extension (or the request `Accept`s `text/html`), serve `index.html` with 200; otherwise 404 (so a missing `/assets/x.js` is a real 404, not HTML that breaks with a MIME error).

`axum-embed`'s `ServeEmbed::with_parameters(Some("index.html".into()), FallbackBehavior::Ok, ...)` implements this if a crate is preferred; a hand-written ~40-line handler is equally reasonable and keeps control of headers.

## 5. Caching headers

Vite emits content-hashed filenames under `assets/` ([Vite: build](https://vite.dev/guide/build.html)), so:

- `/assets/*`: `Cache-Control: public, max-age=31536000, immutable` ([MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control)).
- `index.html` (including every fallback response) and other unhashed files: `Cache-Control: no-cache` plus an `ETag` from rust-embed's `sha256_hash`; answer `If-None-Match` with 304. This guarantees clients pick up new deploys.
- Set `Content-Type` from `mime_guess` (rust-embed `mime-guess` feature / `metadata.mimetype()`).

## 6. Compression

Options:

1. **tower-http `CompressionLayer`** ([docs](https://docs.rs/tower-http/latest/tower_http/compression/index.html)): compresses responses on the fly, negotiates `Accept-Encoding`, applies to API JSON too. Simple; costs CPU per request (small at Lodestar's scale).
2. **Precompressed assets**: generate `.br`/`.gz` at build time (e.g. `vite-plugin-compression`) and serve the variant matching `Accept-Encoding` with `Content-Encoding` and `Vary: Accept-Encoding`. `axum-embed` and tower-http `ServeDir::precompressed_br/gzip` do this; with rust-embed by hand it is a lookup of `path + ".br"`.
3. rust-embed's `compression` feature only shrinks the binary; files are decompressed when read, so it does not save transfer.

**Verdict:** start with `CompressionLayer` on the whole router (covers API and assets in one line). Precompression is an optimisation to adopt only if profiling shows a need.

## Recommendation for Lodestar

- Embed `frontend/dist` with **rust-embed** (features `mime-guess`, `include-exclude`); do not use `include_bytes!`/`include_dir`.
- **Keep the Vite build out of build.rs.** A `just build` (or Makefile) target and CI/Docker stages run `npm ci && npm run build` then `cargo build --release`. Commit a `dist/.gitkeep` so `cargo check` works without Node.
- **Dev loop:** Vite dev server with `server.proxy` for `/api` → axum for UI work; rust-embed's disk-in-debug for checking the integrated binary.
- **Routing:** `/api/v1` nested with its own JSON 404; a router fallback serves embedded files, or `index.html` for extension-less paths, else 404.
- **Headers:** `immutable`, one-year cache for `/assets/*`; `no-cache` + SHA-256 ETag for `index.html`; tower-http `CompressionLayer` for compression.
