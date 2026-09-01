# GRID Launcher — Rust rewrite (walking skeleton)

Milestone 1 of the Rust + Tauri rewrite. Behavior contract: `../docs/porting/`.
Spec: `../docs/superpowers/specs/2026-08-31-rust-tauri-walking-skeleton-design.md`.

## Layout
- `crates/grid-core` — UI-agnostic core: config, secrets (OS keyring only),
  RomM client, cover cache, session.
- `app/` — Tauri 2 shell + Svelte 5 frontend. The Tauri package is named `app`.

## Develop
    cd app && npm install && npx tauri dev

## Test
    cargo test --workspace              # Rust — 173 tests
    cd app && npm test                  # frontend — 46 tests
    npx svelte-check                    # SvelteKit type check
    scripts/check_secret_hygiene.sh     # secret rules (unchanged)
    python -m unittest discover tests/  # Python reference suite — ~1624 tests

## Persisted state

- `config.toml` — app configuration (config directory determined by `directories` crate).
- `grid-launcher.db` — SQLite registry of installed games (same config directory).

## New crates (milestone 2)

Added for the install pipeline: `zip`, `tar`, `flate2`, `liblzma`, `bzip2`,
`sevenz-rust2`, `rusqlite` (with bundled SQLite).

## Build
    cd app && npx tauri build           # AppImage on Linux

The AppImage lands at
`app/src-tauri/target/release/bundle/appimage/GRID Launcher (Rust preview)_<version>_amd64.AppImage`
(the workspace shares one `target/` at `rewrite/target/`, so in practice this
resolves to `rewrite/target/release/bundle/appimage/`).

If bundling fails with `strip: ... unknown type [0x13] section '.relr.dyn'`,
the AppImage tooling's bundled `strip` predates your system libraries' RELR
relocations (seen on newer distros such as Fedora/Nobara). Work around it with:

    NO_STRIP=1 npx tauri build

On some NVIDIA/Wayland stacks WebKitGTK's DMABUF renderer cannot allocate GBM
buffers ("Failed to create GBM buffer ... Invalid argument"), which renders the
window blank white. The app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` at startup
on Linux to avoid this; export the variable yourself (e.g. `=0`) to override.

## Secret handling
Credentials live only in the OS keyring and in redacting in-memory types.
They never appear in config files, logs, IPC payloads, or fixtures.
See the spec's "Secret handling" section — those rules are normative.

## Manual test checklist — Milestone 1

Milestone 1's exit gate. These steps need a desktop session, a live RomM
server, and a gamepad, so they are not automated.

1. Run the AppImage on the desktop.
2. Connect to the live RomM server with an API token; confirm connect
   succeeds and the token prompt never echoes the value anywhere (window,
   terminal, logs).
3. Browse platforms; covers populate and scrolling is smooth.
4. Navigate the grid with a real gamepad: d-pad and left stick move focus,
   held stick repeats at a comfortable rate.
5. Quit and relaunch: the session restores without re-entering credentials.
6. `cat ~/.config/grid-launcher/config.toml` — confirm no token/password
   present.

## Manual test checklist — Milestone 2

Core install pipeline exit gate. Desktop session and live RomM required.

1. Set the library path in the UI; confirm it persists in `config.toml`.
2. Install a small single-file game: entry appears, progress and speed move,
   archive extracts, entry completes, card shows the installed badge.
3. Install a multi-file game: all files land in one `<SafeTitle>/` folder.
4. Cancel a download mid-stream: entry shows `cancelled`, partial file gone.
5. Retry a cancelled entry: fresh download starts.
6. Queue two installs: second waits as `queued`, starts when the first
   finishes.
7. Quit and relaunch: installed badges persist (from `grid-launcher.db`).
8. Uninstall from the details overlay: files and badge are gone.
9. `config.toml` and `grid-launcher.db` contain no token or password.
