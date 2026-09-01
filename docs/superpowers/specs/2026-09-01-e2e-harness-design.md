# Rust rewrite milestone 3.5 — E2E test harness (design)

**Status:** approved design, pre-implementation
**Builds on:** milestones 1–3 (branch `rust-launch`, unmerged); this milestone
branches from it as `rust-e2e` and the two merge to main together once the
suite passes over the milestone 3 flows.
**Research basis:** WebdriverIO `@wdio/tauri-service` docs, Tauri 2 WebDriver
docs, keyring-crate CI patterns (doc-research 2026-09-01). Nobara fc44 ships
no GTK3 `WebKitWebDriver` (`webkit2gtk-driver` does not exist in its repos),
which rules out the `official`/`tauri-driver` provider on this machine and
decides the embedded-driver approach.

## User journey (what this milestone changes for people)

The user stops being the per-milestone test gate. From this milestone on,
every UI flow the manual checklists covered runs as a scripted suite that the
agent executes locally (and CI runs on every push); the user does one formal
testing pass at the end of the rewrite phase. A milestone is not "done" until
its flows are in this suite and green.

## Goal

One command — `rewrite/scripts/e2e.sh` — builds the app in a test
configuration, stands up an isolated environment (virtual display, private
D-Bus session with an unlocked test keyring, mock RomM server, throwaway data
directory, stub emulators), drives the real Tauri app through WebDriver, and
exits nonzero on any failed flow.

## Scope

In scope:

- `e2e` cargo feature in `app` (src-tauri) gating `tauri-plugin-wdio-webdriver`
  and its capability; `withGlobalTauri` enabled only in the e2e config path.
- `GRID_LAUNCHER_DATA_DIR` env override for config/cache/db path resolution.
- `rewrite/e2e/` WebdriverIO project: wdio config (embedded driver provider),
  spec files for every milestone 1–3 flow, Node mock RomM server, fixtures.
- `rewrite/scripts/e2e.sh` runner (build → env → run → teardown) and a
  GitHub Actions job running the suite.
- Stable `data-testid` attributes added to the Svelte components where
  selectors would otherwise be brittle.

Out of scope: visual regression/screenshot testing; Windows/macOS E2E;
gamepad-event simulation (the nav event path is covered indirectly via
keyboard, which shares `handleNav`); performance testing; testing the
AppImage bundle itself (the suite drives the plain binary; the AppImage
remains covered by the end-phase formal pass).

## Global constraints

- **The release build must never contain the automation server.** The wdio
  plugin, its permission/capability entry, and `withGlobalTauri` exist only
  under the `e2e` cargo feature + a separate `tauri.e2e.conf.json` merged
  config used by the e2e build. CI's secret-hygiene stage gains a check that
  the default-feature build graph does not include `tauri-plugin-wdio-webdriver`
  (`cargo tree -p app -e features` grep). This rule has the same standing as
  the secret rules.
- Milestone 1 secret rules unchanged. The fixture token is an obviously fake
  literal (`FAKE-E2E-TOKEN-not-real`); it may appear only under `rewrite/e2e/`
  fixtures and specs, and the hygiene script's allowlist is extended
  accordingly.
- E2E runs never touch the real user environment: all app state under a
  per-run temp `GRID_LAUNCHER_DATA_DIR`; keyring on a private
  `dbus-run-session` bus; mock server on localhost with an OS-assigned port.
- Existing suites (cargo, vitest, svelte-check, clippy, fmt, hygiene, Python)
  stay green; the e2e suite is additive.

## Architecture

### App-side changes (small, feature-gated where visible)

1. **Path override** (`grid-core/src/config.rs` + the two `ProjectDirs` call
   sites in `app/src-tauri/src/lib.rs`): a helper
   `data_dir() -> PathBuf` returning `$GRID_LAUNCHER_DATA_DIR` when set (and
   non-empty), else the existing `ProjectDirs` locations. Config path becomes
   `<data_dir>/config.toml`, registry `<data_dir>/grid-launcher.db`, covers
   `<data_dir>/covers` when the override is set (unified layout under the
   override; the default locations are unchanged). Always compiled (not
   feature-gated): it is useful for portable installs and harmless — the
   variable simply relocates state, no security surface.
2. **`e2e` feature** in `app/src-tauri/Cargo.toml`:
   `e2e = ["dep:tauri-plugin-wdio-webdriver"]`; `lib.rs` registers the plugin
   under `#[cfg(feature = "e2e")]`. The wdio permission lives in
   `capabilities/e2e.json` which is only referenced by `tauri.e2e.conf.json`
   (a merge config passed via `--config`); `withGlobalTauri: true` lives
   there too. The default `tauri.conf.json` is untouched.
3. **`data-testid` attributes** on: connect form fields/submit, platform nav
   buttons, game cards (id-suffixed), installed badge, details overlay
   buttons (install/uninstall/play/stop/close), downloads footer/drawer rows
   and action buttons, emulators panel (rows, form fields, save/delete,
   defaults selects), library-path banner. No behavior changes.

### E2E project (`rewrite/e2e/`)

```
e2e/
  package.json          @wdio/cli, @wdio/tauri-service, @wdio/mocha-framework, expect
  wdio.conf.ts          embedded driverProvider; application = built e2e binary;
                        env injection (GRID_LAUNCHER_DATA_DIR, WEBKIT_DISABLE_DMABUF_RENDERER)
  mock-romm/server.mjs  Node http server: /api/users/me, /api/platforms,
                        /api/roms (paginated), /api/roms/{id}, content downloads
                        (fixture zip; supports file_ids), 401 on wrong token
  fixtures/             platforms.json, roms.json, rom-detail templates,
                        game.zip (one tiny rom file), multi-file fixtures
  stubs/                stub emulator scripts (long-runner, instant-exit)
  specs/
    connect.spec.ts     token connect, bad-token error, quit/restore session
    library.spec.ts     platforms load, grid renders, covers requested
    install.spec.ts     install → progress → badge; uninstall; library-path banner
    downloads.spec.ts   drawer rows, cancel/retry/dismiss, queued second install
    launch.spec.ts      add stub emulator (manual form), set default, play,
                        playing badge, stop, early-exit warning
    emulators.spec.ts   add/edit/delete, auto-fill from path, defaults select
```

Specs run serially in one wdio runner (single app instance per spec file;
the service restarts the app between spec files, giving clean state — each
spec file provisions its own temp data dir via a per-file hook).

### Runner (`rewrite/scripts/e2e.sh`)

1. Build: `cargo build -p app --features e2e` and `npm run build` (frontend)
   via `tauri build --debug --no-bundle --config tauri.e2e.conf.json`
   (exact invocation pinned at implementation; the product is a plain binary
   with the plugin compiled in).
2. `cd e2e && npm ci` (first run) then
   `dbus-run-session -- bash -c 'echo test | gnome-keyring-daemon --unlock; xvfb-run -a npx wdio run wdio.conf.ts'`.
3. Teardown removes the temp data dirs; exit code propagates.
4. Preflight: verifies `xvfb-run`, `dbus-run-session`, `gnome-keyring-daemon`
   exist and prints the dnf install line if not.

### CI

A new job in `.github/workflows/rust-rewrite.yml` (paths-filtered like the
rest): ubuntu-latest, installs xvfb/gnome-keyring/dbus plus the existing
webkit deps, builds the e2e binary, runs `scripts/e2e.sh`. It is allowed to
be slower than the unit jobs; it is required for merge of rewrite changes.

## Flow coverage contract (the M1–M3 manual checklists, executable)

| Manual checklist item | Spec |
| --- | --- |
| M1: connect with token, no echo | connect.spec (value never in DOM/logs assertion) |
| M1: browse platforms/covers | library.spec |
| M1: session restore on relaunch | connect.spec (service app restart) |
| M1: config.toml has no secrets | connect.spec (reads temp config.toml, asserts no token substring) |
| M2: set library path, persists | install.spec |
| M2: single-file install end-to-end + badge | install.spec |
| M2: cancel/retry/queue | downloads.spec |
| M2: relaunch keeps badges | install.spec (app restart) |
| M2: uninstall removes files+badge | install.spec (asserts temp lib dir) |
| M2: no secrets in db | install.spec |
| M3: add emulator + auto-fill + default | emulators.spec |
| M3: play/playing/stop | launch.spec (stub) |
| M3: exit clears badge (~2.5s) | launch.spec |
| M3: missing-exe verbatim error | launch.spec |
| M3: early-exit warning | launch.spec (instant-exit stub) |
| M3: RetroArch no-core verbatim error | emulators.spec or launch.spec |

Gamepad d-pad/stick and multi-file-install remain manual-only (documented in
README as the residual manual checklist for the end-phase pass, alongside
AppImage smoke).

## Error handling / flakiness policy

- Every wait is condition-based (wdio `waitUntil`/`waitForExist` with
  explicit timeouts sized to the operation: 15 s app start, 10 s install of
  the tiny fixture, 5 s UI transitions); no bare sleeps except a single
  poll-interval-tolerant wait (≤4 s) where the 2.5 s reaper is the subject.
- The mock server logs requests to a per-run file; on failure the runner
  prints the last 50 lines plus the app's stderr for diagnosis.
- A spec retry count of 1 (wdio `specFileRetries`) absorbs display/daemon
  startup flakes; a genuine assertion failure failing twice fails the run.

## Testing (of the harness itself)

- mock-romm gets a Node test (node:test) hitting its endpoints directly.
- The path-override helper gets Rust unit tests (set/unset/empty variable).
- The hygiene addition (no wdio plugin in default build graph) is checked in
  the script itself and covered by running it in CI.

## Exit gate

`rewrite/scripts/e2e.sh` green on this machine over the milestone 3 build
(all spec files), CI job green, all existing suites green. Then `rust-e2e`
(containing `rust-launch`) merges to main; the user's next involvement is the
end-phase formal pass.
