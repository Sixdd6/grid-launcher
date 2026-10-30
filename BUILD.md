# Building GRID Launcher

The repository is a Cargo workspace with a Svelte frontend:

- `Cargo.toml` — workspace root; members are `crates/grid-core` and `app/src-tauri`.
- `crates/grid-core` — UI-agnostic core. Never depends on Tauri.
- `app/` — Tauri 2 shell (`app/src-tauri`) plus the Svelte 5 frontend (`app/src`).
  The Tauri package is named `app`.
- `e2e/` — WebdriverIO end-to-end harness, its mock RomM server and mock forge.
- `scripts/` — `e2e.sh` (end-to-end runner) and `check_secret_hygiene.sh` (secret rules).
- `.github/workflows/build.yml` — the one pipeline: gates, end-to-end, release artifacts.
- `emulator-autoprofiles.json`, `retroarch-core-list.json`, `romm-platform-cores.json` —
  data files compiled into `grid-core` with `include_str!`.
- `openapi.json` — the RomM server API contract.
- `SPEC.md` — product behavior. `ARCHITECTURE.md` — module map.

## Prerequisites

- Rust stable, with `rustfmt` and `clippy`.
- Node 24.
- On Linux, the Tauri system libraries (Debian/Ubuntu names, as installed in
  `build.yml`):

      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

- For the end-to-end suite, additionally:

      xvfb gnome-keyring libsecret-tools dbus-x11 at-spi2-core sqlite3

  `libsecret-tools` supplies `secret-tool`, which the runner uses to clear a stage's
  keyring item; without it that step is a silent no-op.
- For a local AppImage bundle: `zsync`.
- On Windows:
  - Visual Studio Build Tools with the "Desktop development with C++" workload (MSVC
    and the Windows SDK).
  - Rust stable on the MSVC toolchain (`stable-x86_64-pc-windows-msvc`), with `rustfmt`
    and `clippy`.
  - Node 24.
  - Git for Windows. Its Git Bash runs the `.sh` scripts and the gate.
  - WebView2 ships with Windows 11.

  The end-to-end suite and the AppImage bundle need Linux. On a Windows machine, run them
  in WSL2 (from a clone inside the WSL file system, not `/mnt/c`) or leave them to CI.

Install the frontend dependencies once:

    cd app && npm ci

## Build container

`scripts/ctr.sh` runs Linux builds, the gate, the end-to-end suite and the AppImage bundle
in a rootless Podman container, capped at 4 CPUs and 8 GB and run at low priority, so the
desktop stays usable. The only host requirement is Podman. There are two images, each
matching the CI job it stands in for: Ubuntu 24.04 (`localhost/grid-launcher-build:24.04`)
for the gate, e2e, `run` and `shell`, like the check and e2e jobs; Debian 13
(`:debian13`) for `appimage`, like the AppImage job, so a local AppImage bundles the same
WebKitGTK as a released one (2.54 today, against Ubuntu 24.04's 2.52.6).

The AppImage is built on Debian 13 because it is the distribution with the oldest glibc
(2.41) that ships WebKitGTK 2.54, the first WebKitGTK that renders on the GPU with NVIDIA
on Wayland. With 2.50 (Ubuntu 22.04, the base until 2026-10-30) and 2.52 (Ubuntu 24.04
and 26.04) WebKitGTK refused GPU rendering there and painted every frame on the CPU, which
made scrolling a large platform stutter. The cost: the AppImage needs glibc 2.41 or newer
(Debian 13, Ubuntu 25.04+, Fedora 42+, Arch and other rolling releases), so it no longer
starts on Ubuntu 22.04/24.04, distributions built on them, or Debian 12.

    scripts/ctr.sh build             # rebuild both images (newer base, no cache, apt upgrade)
    scripts/ctr.sh gate              # the gate below, in its order
    scripts/ctr.sh e2e [groups...]   # scripts/e2e.sh; E2E_* variables pass through
    scripts/ctr.sh appimage 0.9.7    # the release job's AppImage and .zsync, in ctr-out/
    scripts/ctr.sh run <cmd...>      # any command, cwd = the repo
    scripts/ctr.sh shell             # interactive shell
    scripts/ctr.sh clean [--all]     # drop this checkout's volumes (--all: caches too)

`CTR_CPUS` and `CTR_MEMORY` (e.g. `2`, `4g`) can lower the cap, never raise it. Only one
grid-launcher container runs at a time; another call waits for the lock. Only the repo is
mounted. `target/`, `app/node_modules` and `e2e/node_modules` are Podman volumes per
checkout (the Debian 13 image has its own `target/` volume), so a container build never mixes
with a host build in the same tree. `appimage` builds, then repacks with the same
checksum-pinned `appimagetool` and update information as the release job, so its output
is named and laid out like a released AppImage. The cargo
and npm caches are shared volumes. Commit from the host; the container only builds and
tests.

## Develop

    cd app && npx tauri dev

That runs `npm run dev` (Vite on :5173) and the Tauri shell against it. A dev build
never checks for its own updates.

## Gate

The commands below are exactly what `.github/workflows/build.yml`'s `check` job runs,
in its order. All of them must pass before work is considered done.

    scripts/check_secret_hygiene.sh
    cargo fmt --check
    cd app && npm ci && npx svelte-check && npm run build
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p app --all-targets --features e2e -- -D warnings
    cargo test --workspace
    cd app && npm test

On Windows, run the same commands, in the same order, from Git Bash at the repository
root. `scripts/check_secret_hygiene.sh` calls `cargo tree`, so Rust must be on the Git
Bash `PATH`.

A second job, `check-windows`, runs the frontend build, both clippy commands and
`cargo test --workspace` on `windows-latest`.

CI runs only when a release is published or when the workflow is started by hand
(`workflow_dispatch`). Pushes and pull requests trigger nothing, so run the gate locally
before every push. The Windows runner's `TEMP` is a short 8.3 path (`RUNNER~1`); run
Windows tests with a short `TEMP` too, or path comparisons that pass locally fail there.

## End-to-end tests

The suite runs on Linux only (it needs Xvfb, D-Bus, gnome-keyring and `secret-tool`).
On Windows, run it in WSL2 or leave it to CI.

`scripts/e2e.sh` drives a real, locally built Tauri binary with WebdriverIO against a
mock RomM server. CI runs it on every release and manual dispatch, and both build jobs wait
for it.

    scripts/e2e.sh                   # build, then run every stage group
    scripts/e2e.sh connect           # run only the named stage groups
    scripts/e2e.sh library install   # any number of names
    E2E_SKIP_BUILD=1 scripts/e2e.sh  # skip the Rust AND frontend builds
    E2E_KEEP=1 scripts/e2e.sh        # keep the temp run directory

`E2E_SKIP_BUILD=1` is safe only when nothing under `app/src` or the Rust crates has
changed, and it requires a build stamp written by a previous `e2e.sh` build.

Exit codes: 0 pass, 1 a stage group failed, 2 a prerequisite is missing or the binary
cannot be trusted to be an e2e build.

When naming groups, list `python-import` first. It asserts the Connect form, and a group
that connected earlier leaves a RomM credential in the run's private keyring.

A failed stage group is reset (fresh data directory, fresh mock server) and rerun once
before it counts as failed. A failing group does not stop the run — later groups still
execute and the script exits nonzero at the end, so one pass shows every group's result.

Nothing touches your machine outside a temp directory: every stage gets its own
`GRID_LAUNCHER_DATA_DIR`, and the run happens inside a private D-Bus session with its own
gnome-keyring, `XDG_DATA_HOME` and `XDG_RUNTIME_DIR`. No `tauri-driver` is needed — the
app embeds its own WebDriver server behind the `e2e` cargo feature. `e2e/node_modules`
installs itself on the first run.

Stage groups are defined in `scripts/e2e.sh` (`STAGE_GROUPS`); their specs live in
`e2e/specs/`, fixtures in `e2e/fixtures*/`, seeds in `e2e/seed/`, and the mock servers in
`e2e/mock-romm/`.

### Performance probe

`large-platform-perf` measures how a large platform loads, scrolls and hovers in the
real webview. It asserts nothing and never runs in a default pass (`OPT_IN_GROUPS`):
name it to run it. Time a release build, because a debug build parses the ROM list
several times slower than what ships:

    E2E_RELEASE=1 scripts/ctr.sh e2e large-platform-perf

`E2E_RELEASE=1` builds and runs `target/release/app` with its own build stamp, so it
does not disturb the debug e2e build. To include the network, point the probe at a real
RomM server (read-only; use a test account):

    E2E_RELEASE=1 E2E_LIVE_URL=http://host:port E2E_LIVE_PLATFORM=<id> E2E_LIVE_TOKEN="$TOKEN" scripts/ctr.sh e2e large-platform-perf

`ctr.sh` passes `E2E_*` variables into the container by name, so the token never
appears on a command line inside it. Results are the `PERF` lines in the output:

- `first open` / `reselect` — click to first card, first cover and ten covers.
- `list_games` — the backend call alone: fetch, parse and IPC.
- `cold scroll` — scrolling onto rows whose covers are still loading.
- `scroll 60px/frame`, `scroll 400px/frame`, `jump to middle` — scrolling.
- `hover flick` / `browse` / `rest` — pointer browsing, with the number of background
  art changes. WebDriver pointer moves do not reach the WebKitGTK page, so the spec
  drives hover from inside the page.
- `A/B scroll` — the same scroll with one costly style turned off at a time (cover
  shadows, blurs, the background art, compositing layers), to find what a frame pays
  for.

Measure at 4K too: the e2e display is a 3840×2160 screen, and `E2E_WINDOW` sets the
probe's window size (default `1600x1000`):

    E2E_WINDOW=3840x2160 E2E_RELEASE=1 scripts/ctr.sh e2e large-platform-perf

Frame times are `requestAnimationFrame` intervals: about 16 ms is 60 fps. The container
renders in software under Xvfb, so compare runs with each other, not with a desktop.

## Local bundles

    cd app && npx tauri build --bundles appimage    # Linux
    cd app && npx tauri build --bundles nsis        # Windows

Output lands under `target/release/bundle/` (the workspace shares one `target/` at the
repository root).

If bundling fails with `strip: ... unknown type [0x13] section '.relr.dyn'`, the AppImage
tooling's bundled `strip` predates your system libraries' RELR relocations (seen on
Fedora/Nobara). Work around it with `NO_STRIP=1 npx tauri build`.

GRID renders on the GPU by default. WebKitGTK reaches the GPU on Wayland only through its
DMABUF renderer, and on some NVIDIA/Wayland stacks that renderer cannot allocate GBM
buffers ("Failed to create GBM buffer ... Invalid argument"), which leaves the window
blank white. Start GRID with `GRID_LAUNCHER_HW_ACCEL=never` there (it sets
`WEBKIT_DISABLE_DMABUF_RENDERER=1` and renders on the CPU), then turn Settings ›
Appearance › Hardware acceleration off so it stays off. `GRID_LAUNCHER_HW_ACCEL=always`
forces the GPU on whatever the setting says. The log's `webview: hardware acceleration
policy` line says what WebKitGTK actually did: it keeps `Never` when it decides it cannot
use the GPU.

WebKitGTK 2.52.6 (Ubuntu 24.04's) can abort the web process in a JavaScriptCore
garbage-collector thread (`JSC::MarkedBlock::aboutToMarkSlow`) soon after a large
platform opens, which leaves an empty window. The app sets `JSC_useDFGJIT=0` at startup
on Linux, which stops it; export the variable yourself to override it.

## Release

Version numbers come from the tag, not from the tree. `app/src-tauri/tauri.conf.json`
keeps `0.9.0-dev`; each build job passes the real version with
`npx tauri build --config '{"version":"<VERSION>"}'`.

1. Tag `vX.Y.Z` (or `vX.Y.Z-pre`, any semver pre-release) and publish a GitHub release
   for it. `build.yml` triggers on `release: [published]` only, which covers both a
   direct publish and a draft published later.
2. The `version` job strips the leading `v` and rejects a tag that is not semver.
3. `check`, `check-windows` and `e2e` must pass; both build jobs `need` them, so a tag can never
   ship artifacts from an untested tree.
4. `build-linux` builds the AppImage, extracts its AppDir, and repacks it with
   `appimagetool -u "gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync"`,
   because Tauri's bundler embeds no update information. It attaches
   `grid-launcher-<version>-x86_64.AppImage` and its `.zsync`.
5. `build-windows` builds the NSIS installer and attaches
   `grid-launcher-<version>-windows-x86_64-setup.exe`.

A `workflow_dispatch` run is a dry run: it builds version `0.0.0-dev`, uploads both
artifacts to the run, and attaches nothing to any release. A version containing `dev`
also suppresses the app's own update check.

## Secret handling

Credentials live only in the OS keyring and in redacting in-memory types. They never
appear in config files, logs, IPC payloads, or fixtures. `scripts/check_secret_hygiene.sh`
enforces the rule mechanically: `expose_secret()` is allowed only at a fixed list of call
sites, and committed fixtures must contain nothing that looks like a real bearer token.
