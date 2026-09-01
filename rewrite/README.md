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
    cargo test --workspace              # Rust — 312 tests
    cd app && npm test                  # frontend — 53 tests
    npx svelte-check                    # SvelteKit type check
    scripts/check_secret_hygiene.sh     # secret rules (unchanged)
    python -m unittest discover tests/  # Python reference suite — ~1624 tests
    scripts/e2e.sh                      # end-to-end suite (see below)

## E2E tests

`scripts/e2e.sh` drives the real Tauri binary with WebdriverIO. It builds an
e2e-only build of the app, starts a mock RomM server, and runs the specs in
`e2e/specs/` against them.

    scripts/e2e.sh                  # build, then run every stage
    scripts/e2e.sh connect          # run one stage group
    E2E_SKIP_BUILD=1 scripts/e2e.sh # reuse the existing binary
    E2E_KEEP=1 scripts/e2e.sh       # keep the temp run directory

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

    sudo dnf install -y xorg-x11-server-Xvfb dbus-daemon gnome-keyring nodejs npm

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

The embedded WebDriver provider keeps one app process alive for a whole
`wdio run` and cannot restart it, so the runner starts one `wdio run` per spec
file. That is why "relaunch the app" is a two-spec pair sharing one data
directory rather than one spec, and why `wdio.conf.ts` reads its per-stage
settings (`E2E_SPEC`, `E2E_DATA_DIR`, `E2E_MOCK_URL`) from the environment.

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

## Manual test checklist — Milestone 3

Emulated launch core exit gate. Desktop session and live RomM required.

1. Open Emulators from the footer; add a real emulator by path; name and args
   auto-fill from its profile; save; relaunch app — entry persists in
   config.toml.
2. Set it as default for a platform with an installed game.
3. Play the game from the details overlay; the emulator starts with the right
   ROM; the overlay shows Playing and the session appears.
4. Quit the emulator normally; within ~2.5 s the badge clears.
5. Play again, press Stop; the emulator terminates and the badge clears.
6. Point the entry at a nonexistent path and Play: the exact
   "Emulator executable not found:" message shows inline.
7. A RetroArch platform with no core mapping shows the exact "No RetroArch
   core is configured…" message; adding `retroarch_cores` in config.toml
   fixes it.
8. Break an entry's args with an unclosed quote and confirm launch still
   proceeds via the fallback splitter (or shows "Invalid launch arguments"
   when truly unparseable).
9. config.toml and grid-launcher.db still contain no secrets.
