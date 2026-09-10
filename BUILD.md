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
- Node 22.
- On Linux, the Tauri system libraries (Debian/Ubuntu names, as installed in
  `build.yml`):

      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev

- For the end-to-end suite, additionally:

      xvfb gnome-keyring libsecret-tools dbus-x11 at-spi2-core sqlite3

  `libsecret-tools` supplies `secret-tool`, which the runner uses to clear a stage's
  keyring item; without it that step is a silent no-op.
- For a local AppImage bundle: `zsync`.

Install the frontend dependencies once:

    cd app && npm ci

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

A second job, `check-windows`, compiles the Windows code paths with
`cargo check --workspace --all-targets` on `windows-latest`. It runs no tests; it exists
so a Windows-only compile error is caught on the pull request rather than on a tag.

## End-to-end tests

`scripts/e2e.sh` drives a real, locally built Tauri binary with WebdriverIO against a
mock RomM server. CI runs it on pushes to `main` and on manual dispatch.

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

## Local bundles

    cd app && npx tauri build --bundles appimage    # Linux
    cd app && npx tauri build --bundles nsis        # Windows

Output lands under `target/release/bundle/` (the workspace shares one `target/` at the
repository root).

If bundling fails with `strip: ... unknown type [0x13] section '.relr.dyn'`, the AppImage
tooling's bundled `strip` predates your system libraries' RELR relocations (seen on
Fedora/Nobara). Work around it with `NO_STRIP=1 npx tauri build`.

On some NVIDIA/Wayland stacks WebKitGTK's DMABUF renderer cannot allocate GBM buffers
("Failed to create GBM buffer ... Invalid argument"), which leaves the window blank
white. The app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` at startup on Linux; export the
variable yourself to override it.

## Release

Version numbers come from the tag, not from the tree. `app/src-tauri/tauri.conf.json`
keeps `0.9.0-dev`; each build job passes the real version with
`npx tauri build --config '{"version":"<VERSION>"}'`.

1. Tag `vX.Y.Z` (or `vX.Y.Z-pre`, any semver pre-release) and publish a GitHub release
   for it. `build.yml` triggers on `release: [published]` only, which covers both a
   direct publish and a draft published later.
2. The `version` job strips the leading `v` and rejects a tag that is not semver.
3. `check` and `check-windows` must pass; both build jobs `need` them, so a tag can never
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
