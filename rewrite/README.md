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
    cargo test --workspace              # Rust — 25 tests (grid-core 22, app_lib gamepad mapper 3)
    cd app && npm test                  # frontend focus model — 3 tests
    scripts/check_secret_hygiene.sh     # secret rules

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

## Secret handling
Credentials live only in the OS keyring and in redacting in-memory types.
They never appear in config files, logs, IPC payloads, or fixtures.
See the spec's "Secret handling" section — those rules are normative.

## Manual test checklist

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
