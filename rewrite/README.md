# GRID Launcher — Rust rewrite (walking skeleton)

Milestone 1 of the Rust + Tauri rewrite. Behavior contract: `../docs/porting/`.
Spec: `../docs/superpowers/specs/2026-08-31-rust-tauri-walking-skeleton-design.md`.

## Layout
- `crates/grid-core` — UI-agnostic core: config, secrets (OS keyring only),
  RomM client, cover cache, session.
- `app/` — Tauri 2 shell + Svelte 5 frontend. The Tauri package is named `app`.
- `e2e/` — WebdriverIO end-to-end harness and the mock RomM server it runs against.

## Develop
    cd app && npm install && npx tauri dev

## Test
    cargo test --workspace              # Rust — 316 tests
    cd app && npm test                  # frontend — 71 tests
    npx svelte-check                    # SvelteKit type check
    scripts/check_secret_hygiene.sh     # secret rules (unchanged)
    python -m unittest discover tests/  # Python reference suite — ~1624 tests
    scripts/e2e.sh                      # end-to-end suite (see below)

## E2E tests

`scripts/e2e.sh` drives the real Tauri binary with WebdriverIO. It builds an
e2e-only build of the app, starts a mock RomM server, and runs the specs in
`e2e/specs/` against them. The `emulator-catalog` group also starts a second
mock, `e2e/mock-romm/mock-forge.mjs`, standing in for GitHub and a
direct-download provider (redream.io); the app's `e2e` cargo feature reads
`GRID_LAUNCHER_E2E_FORGE_BASE` and redirects forge requests to it at request
time (`launch/forge.rs`'s `effective_url`) while keeping every other URL
real, so the catalog's scrape regexes are exercised against genuine markup.

    scripts/e2e.sh                  # build, then run every stage
    scripts/e2e.sh connect          # run one stage group
    scripts/e2e.sh library install  # or several — any number of names
    E2E_SKIP_BUILD=1 scripts/e2e.sh # reuse the existing binary
    E2E_KEEP=1 scripts/e2e.sh       # keep the temp run directory

There is no separate `E2E_ONLY` variable — the positional group-name filter
above is already the way to run a subset, so `E2E_SKIP_BUILD=1
scripts/e2e.sh downloads` is the fast inner loop for one group's specs.

Exit codes: 0 pass, 1 a stage group failed, 2 a prerequisite is missing or the
binary is not a stamped e2e build. A failing stage prints the app's own
stdout/stderr, the wdio output, the mock server log, and the mock's request
log.

A failed stage group is reset (fresh data dir, fresh mock) and rerun once
before it counts as failed. A failing group does not stop the run — later
groups still execute and the script exits nonzero at the end, so one pass
shows every group's result.

`E2E_SKIP_BUILD=1` requires a build stamp written by a previous `e2e.sh`
build, and refuses to run if the binary was rebuilt outside the script (a
plain `cargo build -p app` produces the same path without the `e2e` feature,
which otherwise fails opaquely).

### Prerequisites

    sudo dnf install -y xorg-x11-server-Xvfb dbus-daemon gnome-keyring nodejs npm sqlite

`e2e/node_modules` installs itself on the first run. No `tauri-driver` or
`webkit2gtk-driver` is needed: the app embeds its own WebDriver server behind
the `e2e` cargo feature.

### What it does to your machine

Nothing outside a temp directory. Each stage gets its own
`GRID_LAUNCHER_DATA_DIR`, so `~/.config/grid-launcher` is never read or
written. The whole run happens inside `dbus-run-session` with a throwaway
gnome-keyring whose files and control socket go to a redirected
`XDG_DATA_HOME` and `XDG_RUNTIME_DIR`, so the real login keyring is untouched
and no stray daemon can claim `/run/user/$UID/keyring`.

The app runs under `xvfb-run` with `GDK_BACKEND=x11` and `WAYLAND_DISPLAY`
unset. Both are load-bearing: GTK prefers Wayland when it can see the
session's compositor socket, which would put the app window on your real
desktop and ignore Xvfb entirely.

An exit trap kills everything belonging to the run and removes the temp
directory. It matches processes by the run directory in their environment
rather than by process group, because D-Bus activates helpers
(`xdg-desktop-portal-*`, `ksecretd`, `gnome-keyring-daemon`) that are children
of the private bus and outlive `dbus-run-session`. A green run leaves no
`/tmp/grid-e2e-*` behind; a failed run keeps one on purpose, for its logs.

### Coverage

| Stage group | Specs | Covers |
| --- | --- | --- |
| `connect` | `connect.spec.ts` | wrong token → "rejected the credentials"; fixture token → library; the token never reaches the DOM; `config.toml` holds server_url + username and no secret |
| `connect-restore` | `connect-restore-a/-b.spec.ts` | connect, then relaunch the binary against the same data dir and keyring — the session restores with no credential re-entry |
| `library` | `library.spec.ts` | both fixture platforms render; selecting platform 1 shows its cards, including the server `name: null` game falling back to `fs_name_no_ext`; a cover `<img>` gets a real `src` and a nonzero `naturalWidth` (the regression test for the asset-protocol-scope fix); `ArrowRight` moves the focused card |
| `install` | `install-a/-b.spec.ts` | the library-path banner appears when unset and hides once a path is saved; installing rom 101 from the details overlay reaches a `Completed` download row and an `installed` badge, and extracts `game.sfc` under the temp library dir; the badge survives a relaunch (part b); uninstalling via the details two-click removes the badge and the files; `grid-launcher.db` never contains the fixture token |
| `downloads` | `downloads.spec.ts` | this group's mock server runs with `--throttle-ms 100` (chunked slow streaming — see `mock-romm/server.mjs`'s `e2e_throttle`) against the ~300KB "Big Arcade Game" fixture (rom 301), giving a real in-flight download to interact with: a second install queues behind the first; cancelling the active download shows `Cancelled`; retrying it reaches `Completed`; dismissing removes the row |
| `emulators` | `emulators.spec.ts` | auto-fill from autoprofile match on path basename; name and args persist; row order preserved on edit; duplicate name rejection; two-click delete; per-platform defaults persist to `config.toml` |
| `launch` | `launch.spec.ts` | pre-seeded with one installed game and three emulator stubs; play the game (argv recorded); instant-exit stub error; broken path error; unmapped RetroArch error with the verbatim message. Each mutation through the emulators UI is confirmed written to `config.toml` before proceeding. |
| `emulator-catalog` | `emulator-catalog.spec.ts` | open the catalog tab, install a github-provider stub (`PCSX2`, an AppImage asset) and a direct-provider stub (`Redream`, scraped from an HTML download page and extracted from tar.gz) against `mock-forge.mjs`; both drawer rows reach `Completed`, land under `Emulators/<name>-<tag>/`, and the catalog marks them installed/disabled; PCSX2 is set as the PS2 default and launches the pre-seeded game (argv file assertion) |

The embedded WebDriver provider keeps one app process alive for a whole
`wdio run` and cannot restart it, so the runner starts one `wdio run` per spec
file. That is why "relaunch the app" is a two-spec pair sharing one data
directory rather than one spec, and why `wdio.conf.ts` reads its per-stage
settings (`E2E_SPEC`, `E2E_DATA_DIR`, `E2E_MOCK_URL`) from the environment.

### Residual manual checklist

These behaviors remain manual and require a desktop session + live RomM server
(the E2E suite automates the remaining coverage):

- **Gamepad hardware**: d-pad and left stick move focus, held stick repeats at a
  comfortable rate.
- **AppImage smoke test**: launch the AppImage from the desktop, connect to a live
  RomM server, confirm the token prompt never echoes the value.
- **Multi-file install**: download an archive containing multiple game files and
  confirm they land under one safe-title directory.
- **Basic-auth mode**: connect via `http://user:password@server:port/romm` and
  confirm both credentials enter the keyring and the password never reaches
  `config.toml` or logs.

## Persisted state

- `config.toml` — app configuration (config directory determined by `directories` crate).
- `grid-launcher.db` — SQLite registry of installed games (same config directory).

## New crates (milestone 2)

Added for the install pipeline: `zip`, `tar`, `flate2`, `liblzma`, `bzip2`,
`sevenz-rust2`, `rusqlite` (with bundled SQLite).

## Milestone 3 dependencies

`libc` (unix-only target dependency for session stop signals); `emulator-autoprofiles.json`
at the repo root is embedded into grid-core at compile time with `include_str!`.

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

Milestone 1's exit gate: now automated by the E2E suite (`connect` + `connect-restore` stage groups); residual items above.

## Manual test checklist — Milestone 2

Core install pipeline exit gate: now automated by the E2E suite (`install` + `downloads` stage groups); residual items above.

## Manual test checklist — Milestone 3

Emulated launch core exit gate: now automated by the E2E suite (`emulators` + `launch` stage groups); residual items above.
