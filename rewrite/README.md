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
    cargo test --workspace              # Rust — 487 tests
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
| `downloads` | `downloads.spec.ts` | this group's mock server runs with `--throttle-ms 100` (chunked slow streaming — see `mock-romm/server.mjs`'s `e2e_throttle`) against the ~2MB "Big Arcade Game" fixture (rom 301), giving a real in-flight download to interact with: the three segments, their counts and the verbatim legend render before anything runs; a first install sits in Active and a second queues behind it in Queued; a base row carries no kind badge; every row has a `download-graph-<id>` svg with a network and a disk path (structure only — sampling is once per wall-clock second, so no spec asserts a sample count); the footer strip reads `⬇ Big Arcade Game · <pct> · <rate>/s` with its own sparkline and opens the view; cancelling the active download shows `Cancelled` in Completed; retrying it reaches `Completed`; dismissing removes the row |
| `emulators` | `emulators.spec.ts` | auto-fill from autoprofile match on path basename; name and args persist; row order preserved on edit; duplicate name rejection; two-click delete; per-platform defaults persist to `config.toml` |
| `launch` | `launch.spec.ts` | pre-seeded with one installed game and three emulator stubs; play the game (argv recorded); instant-exit stub error; broken path error; unmapped RetroArch error with the verbatim message. Each mutation through the emulators UI is confirmed written to `config.toml` before proceeding. |
| `emulator-catalog` | `emulator-catalog.spec.ts` | open the catalog tab, install a github-provider stub (`PCSX2`, an AppImage asset) and a direct-provider stub (`Redream`, scraped from an HTML download page and extracted from tar.gz) against `mock-forge.mjs`; both drawer rows reach `Completed`, land under `Emulators/<name>-<tag>/`, and the catalog marks them installed/disabled; post-install autoconfig creates `portable.ini` next to the installed PCSX2 and writes the managed keys into `inis/PCSX2.ini` (with no `[Achievements]`/`Bios` keys, since no RA credentials are configured); PCSX2 is set as the PS2 default and launches the pre-seeded game (argv file assertion) |
| `cloud-saves` | `cloud-saves.spec.ts` | this group's mock (`--fixtures-dir fixtures-cloud-saves`) seeds server save records and exposes a live `GET /__e2e__/requests` introspection endpoint, since the mock is a separate process from the spec; manual upload from the details panel POSTs one `overwrite=true` multipart request with the local file; launching with auto-download-on-launch restores the cloud save to disk before the "TestEmu" stub emulator (which writes fresh save content only once it receives the Stop button's SIGTERM) ever runs; exiting fires the auto-upload (delay zeroed via Settings › Cloud saves) with that fresh content; uploading against four seeded records in one slot prunes exactly one over the default retention limit of 3; a native-platform game's save-state panel info is unsupported with the verbatim block reason (asserted via a direct `cloud_panel_info` invoke, since the UI never renders that combination — see the spec's own comment). xemu's raw-disk save sync has no E2E coverage here (no xemu binary in CI); the Task 16 wiremock integration test covers it instead. |
| `images` | `images-a/-b.spec.ts` | pre-seeded with one `grid-launcher.db` row (rom 102) written in the pre-milestone-7 v1 schema, so `Registry::open` must migrate it to v2 on first open; part a connects, opens rom 101's details (a real large cover, two of three `merged_screenshots` entries — the third is a foreign host the server-resolver filters out — and its description), installs rom 101, confirms a loaded cover on the Library grid, then flips the mock into "offline" mode (`POST /__e2e__/offline`, which destroys the socket for every `/api/`/`/assets/` request rather than erroring) as its last step; part b's fresh launch finds the server unreachable (R2: Library section, "Not connected" chip) yet still renders rom 101's already-cached cover with no client, and its details show Play/no-Install; Retry (after flipping the mock back online) reconnects and its replenish job backfills the migrated rom 102 row's image columns and fetches its cover, which then appears on the Library grid |
| `ps3-install` | `ps3-install.spec.ts` | a PlayStation 3 install with NO emulator configured, so `ps3_roots_from_config` takes its library fallback: rom 401's archive (`BLUS30336/PS3_GAME/{USRDIR/EBOOT.BIN,PARAM.SFO}`) reaches a `Completed` row and is routed whole into `<library>/PlayStation 3/.vfs/dev_hdd0/game/BLUS30336/`, the staging directory and the archive are both gone afterwards, and `grid-launcher.db` records `ps3_game_id = 'BLUS30336'` (read with the `sqlite3` CLI). The seed's empty `xdg-config/` directory — exported as `E2E_XDG_CONFIG_HOME`, used as both `XDG_CONFIG_HOME` and `RPCS3_CONFIG_DIR` — is what keeps the RPCS3 VFS probe off a developer's real RPCS3 install |
| `content` | `content.spec.ts` | update/DLC content, where a game and its extra content share one content path and are told apart only by `file_ids` (the mock resolves that pair back to a fixture file). PS4 (rom 501) is user-driven: base install completes, `Install Update` appears, clicking it opens a `PS4 Game (update)` row that merges the update archive's `CUSA12345/` tree into the base install's extraction directory. Xbox 360 (rom 601) is automatic: `queue_xbox360_content` admits the `Xbox Game (update)` row from inside the base install's finalize step with no click, and its STFS package lands at `<xenia dir>/content/0000000000000000/415608C3/000B0000/tu00000001`. The seeded Xenia stub is named `xenia_edge` (matching the Linux-capable `Xenia Edge (Xbox 360)` profile rather than the Windows-only Canary one) with a `portable.txt` beside it, which is what puts the content root next to the executable |
| `native` | `native.spec.ts` | a Windows-platform ("native") game: the primary button reads `Install App`; installing rom 701 lays out `<library>/Windows/My Game/{game/MyGame/mygame.exe,prefix}`; Game Settings lists the extracted executable and saves `--fullscreen`; Play launches through the seeded `wine` stub — first on the app's `PATH` via `E2E_STUB_BIN`, with `default_compat_tool = "wine"` — whose `wine-argv.log` carries the executable path and the saved parameter, and Stop clears the playing chip. Rom 702's ~300KB archive carries a per-file `e2e_throttle` in the fixture (so only that one download is slow), giving the Details `Cancel` button a real in-flight install to cancel to `Cancelled` |
| `firmware` | `firmware.spec.ts` | the two background firmware triggers. Per game: installing rom 801 (PlayStation) fires the finalize hook's firmware pass against the seeded DuckStation default, whose profile routes `scph5501.bin` to `<stubs>/duckstation/bios/`. Hand-added RPCS3: the seed writes the stub file but no config entry, so adding `RPCS3` through the Emulators form is what fires `spawn_ps3_firmware` — it admits its own `PS3 Firmware`/`Firmware` drawer row, completes, writes `<stubs>/rpcs3/PS3UPDAT.PUP`, and the RPCS3 card then shows the downloaded-firmware note and `Install PS3 Firmware`, which produces the verbatim success toast and spawns the stub with `--installfw <pup>` (asserted from the stub's own argv log). The fixture's PlayStation 3 platform carries a placeholder rom purely so its `rom_count` is nonzero: `RommClient::platforms` drops every zero-count platform, and `ps3_platform_id` reads the id out of that filtered list |
| `updates` | `updates.spec.ts` | seeded rows (`e2e/seed/updates-seed.mjs`) against `fixtures-updates` and a mock forge route standing in for `api.github.com/repos/Sixdd6/grid-launcher/releases/latest`: badges appear on rom 801 (SNES, out of date by timestamp) and rom 802 (Windows, out of date by file-name tag) only — not on rom 803 (identical to the server) or rom 804 (absent from server rom detail); non-native Update on rom 801 re-downloads and re-extracts, landing beside the seeded install and clearing its badge; the native Update on rom 802 two-click-confirms, then MERGES the new archive over the installed tree, preserving the seeded `saves/slot1.sav`; an absent server entry (rom 804) shows no Update button at all; the self-update badge appears on the top bar with the mock forge's tag `v9.9.9-e2e` (`GRID_LAUNCHER_E2E_UPDATE_CHECK=1` lifts the dev-build gate for this group only), opens Settings › Updates, and Dismiss hides the badge while the Updates entry stays; the Settings rail's Connection, Updates, Cloud saves, RetroAchievements and Appearance panes each render their own line |

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
- **RetroArch core picker**: with a real RetroArch install, confirm Emulators ›
  Platform defaults shows a Core select only on rows whose emulator is RetroArch, that
  it lists only cores present in the RetroArch `cores/` directory, and that changing it
  writes `[retroarch_cores]` in `config.toml`.
- **RetroArch platform gating**: with a real RetroArch install missing a platform's
  core, confirm that platform's emulator select does NOT offer RetroArch, and that
  installing the core makes it appear after the pane refreshes.
- **Explicit "(none)"**: set a platform to (none) on Emulators › Platform defaults,
  switch views and back, it stays (none) and launching a game there reports no
  emulator.
- **Theme override**: with the OS in dark mode, set Settings › Appearance › Theme to
  Light, confirm the shell repaints immediately and stays light after a relaunch;
  set it back to Follow system and confirm it tracks an OS theme change live.
- **Background art**: hover a Library card for more than half a second and confirm the
  blurred art behind the content cross-fades to that cover; drag the fade slider and
  confirm the art responds while dragging and the value survives a relaunch.
- **Server menu**: open the server name menu and confirm "Open RomM in browser" opens
  the configured server. With a basic-auth server URL, confirm the menu item does
  nothing rather than opening a URL carrying the password.
- **Recently played**: launch an installed game, quit it, and confirm the Library rail's
  Recent count includes it and the "Recently played" sort puts it first; confirm the
  stamp survives updating that game (an update must not reset it) and a relaunch of the
  app.
- **Platform firmware chip**: on the Server view, select a platform the server holds
  firmware for and confirm the chip counts the files. With a default emulator set,
  press Install and confirm the firmware lands in that emulator's firmware directory;
  with the platform set to (none), confirm the chip reads "no default emulator" and
  offers no button.
- **Details video**: on a RomM server whose game carries a `path_video`, open Details ›
  Media and play it. Confirm it plays from the local cache (the file appears under the
  covers directory) and that the network request came from the app, not the webview.
- **YouTube trailer**: with a game whose `youtube_video_id` is set, open the trailer in the
  fullscreen viewer and confirm it shows a poster with a "Watch on YouTube" button, and that
  the button opens the trailer in the system browser (not embedded in the app — Linux never
  sends the `Referer` header an embed needs, so YouTube answers error 153 for every embed).
- **Related row**: open a game whose IGDB metadata lists similar games and confirm only
  titles the server actually holds appear, and that clicking one is not offered (the row is
  informational until collections land).
- **Card sizes at width**: with the window maximised on a wide display, confirm the grid
  fills to at most 1920px and stays centred, and that Small / Medium / Large change the
  column count rather than stretching the covers.
- **Download sparklines**: install a game large enough to run for a minute against a
  live server. In the Downloads view confirm the row's graph grows from the right, one
  point per second, in the primary colour during the download and in teal once the
  install phase writes to disk; the caption under it shows the rate and an ETA that
  counts down; the footer strip draws the same line at 120×18 and stops updating (but
  keeps its last shape) when the row completes.
- **Completed history**: with more than 50 finished rows (repeat a small install and
  dismiss nothing), confirm the Completed segment holds exactly 50 and the oldest rows
  disappear first while any active or queued row stays.
- **Light theme graphs**: switch Settings › Appearance to Light and confirm both series
  are legible against the row surface.
- **Edit sheet**: on Emulators › Installed press Edit on one row, then Edit on another
  and confirm the sheet's fields switch to the second entry; press Cancel and confirm the
  row highlight clears. Press Ctrl+F from any Emulators pane and confirm the catalog pane
  comes forward with its search box focused.
- **Background art off**: untick Settings › Appearance › Background art and confirm the
  art disappears and `background_fade = 0` reaches `config.toml`; tick it again and
  confirm the previous fade value returns.

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

## Manual test checklist — Milestone 5

Emulator autoconfig exit gate: now automated by the E2E suite (`emulator-catalog` stage group's post-install `portable.ini`/`PCSX2.ini` assertions, on top of the milestone-4 catalog install coverage) plus `cargo test -p grid-core` and `--features e2e`; residual items above.

## Manual test checklist — Milestone 6

Cloud save/state sync exit gate: now automated by the E2E suite (`cloud-saves` stage group) plus `cargo test --workspace`; xemu's raw-disk save sync stays covered by the Task 16 wiremock integration test instead (no xemu binary in CI); residual items above.

## Manual test checklist — Milestone 7

Covers/images exit gate: now automated by the E2E suite (`images` stage group) plus `cargo test --workspace`; milestone-specific residual items, neither reproducible against the mock: a real RomM server with LaunchBox/ScreenScraper screenshot metadata, to exercise foreign-host filtering and the type-token rules against real payloads instead of fixtures; and a cache directory grown past 512 MiB, to observe the startup sweep's log line. Residual items above also apply.

## Manual test checklist — Milestone 8

Install specials exit gate: now automated by the E2E suite (`ps3-install`, `content`, `native`,
`firmware` stage groups) plus `cargo test --workspace`; milestone-specific residual items, none
reproducible against the mock:

- **A real RomM server with PS4/Xbox 360 content categories and firmware**: confirm
  `files[].category` classification and update/DLC availability against real server payloads,
  not fixture data.
- **A real Proton install through `umu-run`**: confirm a managed compat-tool install actually
  launches a Windows game via `umu-run`/Proton on a real Steam/Proton environment.
- **An RPCS3 `--installfw` dialog**: confirm the spawned RPCS3 process shows its real firmware
  installation UI and completes against a real `PS3UPDAT.PUP`.
- **RAR archives from a real server**: confirm the bundled `unrar` crate extracts a real-world
  RAR-compressed download end to end.

Residual items above also apply.

## Manual test checklist — Milestone 9

Identity/updates exit gate: now automated by the E2E suite (`updates` stage group) plus
`cargo test --workspace`; milestone-specific residual items, none reproducible against the mock:

- **Window title shows the version**: confirm the title bar reads `GRID Launcher <version>` on a
  real desktop window manager, not just the webview's own title element.
- **An installed game updated on the server shows the badge after reconnect**: against a real
  RomM server, upload a newer file for a ROM you already have installed, then reconnect (or
  Retry) and confirm the Library badge appears with no restart needed.
- **Update on a Windows game keeps saves**: run the native merge Update against a real
  Proton/Wine save directory (not the fixture's single file) and confirm nothing outside the new
  archive's paths is touched.
- **The banner appears on a release build only**: confirm a real release build (no `-dev`
  pre-release) checks `releases/latest` once at startup
  and shows the banner when a newer tag exists, while a `cargo tauri dev` source build never
  checks at all.

Residual items above also apply.
