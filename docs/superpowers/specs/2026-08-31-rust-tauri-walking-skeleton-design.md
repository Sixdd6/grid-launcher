# Rust/Tauri rewrite — walking skeleton design

Status: approved design, pre-implementation
Date: 2026-08-31
Requirements baseline: `docs/porting/` (behavior contract for all subsystems)

## Context

GRID Launcher is being rewritten from Python/PySide6 to Rust + Tauri 2 with a
Svelte 5 frontend. The `docs/porting/` documents are the behavior contract; the
README there defines the subsystem port order. This spec covers only milestone 1,
the walking skeleton: the smallest end-to-end slice that proves every risky part
of the stack. Later milestones get their own specs.

Decisions already made (with the user, 2026-08-31):

- **Stack**: Rust + Tauri 2; Svelte 5 frontend (chosen for compositor-friendly
  animation with minimal runtime overhead and fast image delivery via the asset
  protocol).
- **Location**: `rewrite/` subdirectory of this repository (Cargo workspace).
- **Formats**: fresh start. No on-disk compatibility with the Python app. None of
  the Python defects listed in `docs/porting/README.md` are reproduced. A
  one-time importer is a possible future milestone, out of scope here.
- **Milestone 1**: walking skeleton (this document).

## Goals

A user can launch the app, connect to a RomM server, and browse their library as
a cover grid — with mouse, keyboard, and gamepad — smoothly. Everything behind
that flow is production-shaped: crate boundaries, secret handling, error
handling, tests, CI, and an AppImage build.

## Non-goals (milestone 1)

Install pipeline, game launch, cloud saves, emulator autoconfig, Discover,
TV-mode views (only the navigation/input substrate is proven), self-update,
Windows packaging (code stays portable; only Linux is built and tested), cache
eviction, importer from the Python app's state.

## Architecture

```
rewrite/
  Cargo.toml               # workspace: crates/grid-core, app/src-tauri
  crates/grid-core/        # pure-Rust library; no Tauri dependency
    src/config.rs          #   config load/save (TOML)
    src/secrets.rs         #   keyring access, SecretString wrapping
    src/romm/              #   HTTP client: auth, platforms, roms, pagination
    src/covers.rs          #   cover cache: fetch, store, lookup
    src/identity.rs        #   game identity key (minimal: rom id)
  app/
    src-tauri/             # Tauri 2 shell
      src/commands.rs      #   connect, list_platforms, list_games
      src/assets.rs        #   custom protocol serving covers from cache
      src/gamepad.rs       #   gilrs poll thread -> navigation events
    src/                   # Svelte 5 frontend
      routes/connect       #   server/credentials form
      routes/library       #   virtualized cover grid
      lib/focus/           #   focus/navigation model (keyboard + gamepad events)
```

Boundary rule: `grid-core` never depends on Tauri and owns all I/O policy
(HTTP, disk, keyring). The Tauri layer is command plumbing and OS integration.
The frontend owns rendering and focus visuals only; it holds no credentials and
performs no HTTP.

## Secret handling (normative, all milestones)

1. Tokens exist in exactly two places: the OS keyring (`keyring` crate — Secret
   Service/KWallet on Linux, Credential Manager on Windows) and process memory.
   Never in config files, caches, logs, fixtures, or IPC payloads. The config
   schema has no secret fields at all.
2. In memory the token is a `secrecy::SecretString` from the moment of capture:
   Debug/Display render `[REDACTED]`, zeroized on drop, raw access only via an
   explicit `expose_secret()` at the single point where the Authorization header
   is built. The login form's plain string is wrapped at the IPC boundary and
   dropped.
3. The frontend never receives credentials. Commands return session state
   (connected, username, server URL) only. The cover asset protocol attaches
   auth on the Rust side.
4. `tracing` policy: no header dumps anywhere; any request-level logging strips
   `Authorization` before formatting; error types carry status code plus a
   sanitized body excerpt, never the outgoing request.
5. Test fixtures use obviously fake tokens; CI greps committed fixtures for
   real-looking bearer values and fails on a hit.

## Component specs

### grid-core: config

- Location: `<XDG_CONFIG_HOME>/grid-launcher/config.toml` (via `directories`
  crate; Windows equivalent when that milestone comes). Distinct from the Python
  app's `~/.grid-launcher/` — the two apps do not collide.
- Milestone-1 schema: `server_url: String`, `username: String`, plus a
  `schema_version: u32 = 1` field for forward migration. Unknown keys are
  preserved on rewrite (round-trip via `toml_edit` or tolerant serde).
- Writes are atomic: write `config.toml.tmp`, fsync, rename (the Python app's
  non-atomic write is a documented defect; not reproduced).

### grid-core: romm client

- `reqwest` with rustls; TLS verification always on (the Python PS3-firmware
  exception is a defect, not a behavior to port).
- Endpoints (per `docs/porting/01-romm-api.md`): `GET /api/users/me` (auth
  probe), `GET /api/platforms`, `GET /api/roms` paginated (page size 200,
  loop until short page — same algorithm as the reference).
- Auth: the same two modes as the reference (doc 01) — bearer token, or HTTP
  basic with username/password. The Authorization header is built in exactly one
  function (the `expose_secret()` point). Exact login/token endpoints are taken
  from `openapi.json` 5.2.0 at implementation time.
- Errors: one `RommError` enum (connection, auth, HTTP status + sanitized
  excerpt, decode). Failure classification mirrors the reference's
  user-facing categories (doc 01) without its wording.

### grid-core: cover cache

- Location: `<XDG_CACHE_HOME>/grid-launcher/covers/`.
- One filename scheme: SHA-256 of the identity basis (rom id; fallback
  `title|platform`), extension from content sniffing — the reference's dual
  scheme and missing auth header are documented defects, not ported.
- Fetch-on-miss with in-flight deduplication; negative results cached in memory
  for the session.

### Tauri shell

- Commands: `connect(server_url, username, password_or_token) -> SessionState`,
  `disconnect()`, `list_platforms() -> Vec<Platform>`,
  `list_games(platform_id) -> Vec<GameSummary>`.
- Custom protocol `covers://<key>` streams files from the cover cache,
  triggering a fetch on miss; browser-native decode and HTTP caching do the
  rest. No base64 over IPC.
- Gamepad: `gilrs` on a dedicated thread emitting the doc-09 navigation event
  vocabulary (`up/down/left/right/accept/back/...`) to the frontend via Tauri
  events. Dead zone 0.3, repeat interval 0.2 s (carried from the reference).
  Keyboard produces the same events in the frontend so the focus model has one
  input vocabulary.

### Frontend

- Svelte 5, TypeScript, Vite. Two routes: connect form, library grid.
- Grid is virtualized (windowed rendering); covers load through `covers://`
  URLs with width hints; focus movement is transform/opacity animation only
  (compositor-driven), using Svelte springs.
- Focus model: a small `lib/focus/` module consuming the navigation-event
  vocabulary; spatial movement within the grid, explicit focus order elsewhere.
  This module is the seed of TV mode.

## Testing and CI

- `grid-core` unit tests: config round-trip + atomic write, pagination against
  recorded fixtures (`wiremock` or static JSON), cover-key derivation, error
  classification. Fixture tokens are fake.
- Frontend: `svelte-check` + build as the milestone bar; component tests come
  with TV mode.
- New workflow `.github/workflows/rust-skeleton.yml`: `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test`, frontend build, and the fixture
  secret-grep guard. Existing Python workflows untouched.
- Exit proof: `cargo tauri build` produces a Linux AppImage that connects to a
  live RomM server and browses the library with a gamepad.

## Risks

- Webview animation jank on low-end hardware → mitigated by compositor-only
  animation and virtualization; measured on the grid before adding effects.
- `gilrs` guide-button capture varies by platform/driver → guide behavior is a
  TV-mode concern; skeleton only proves the event pipeline.
- Tauri 2 API drift vs training data → implementation verifies against current
  Tauri/Svelte docs (context7) before scaffolding.

## After this milestone

Subsystem milestones follow `docs/porting/README.md` order (identity → full API
→ install → launch → autoconfig → cloud → images → TV), each getting its own
spec against the corresponding porting doc.
