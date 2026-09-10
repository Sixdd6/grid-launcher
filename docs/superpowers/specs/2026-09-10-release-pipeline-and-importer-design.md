# Release pipeline and Python-config importer — design

Date: 2026-09-10. Status: approved in chat (no token import; e2e runs on push to main).

## Goal

Ship the first Rust release from GitHub Actions: an AppImage with embedded update
information and a Windows NSIS installer, built from a release tag. Retire the Python
build workflows. On first launch, carry an existing Python user's configuration and
installed library into the Rust app without any action on their part, except tokens.

Out of scope: TV mode, the Discover tab, in-app self-update beyond the existing
check-only notice, an MSI target, macOS, code signing.

## Part 1 — Workflow

`.github/workflows/rust-rewrite.yml` is renamed `.github/workflows/build.yml` with the
workflow name `Build`. `appimage-linux.yml`, `pyinstaller-linux.yml` and
`pyinstaller-windows.yml` are deleted. `build.sh` and `appimage/` stay in the tree as the
Python reference; nothing runs them.

### Triggers

| Job | `push` to `main` | `pull_request` | `release: published` | `workflow_dispatch` |
| --- | --- | --- | --- | --- |
| `version` (new) | no | no | yes | yes |
| `check` (existing Linux gate) | yes | yes | yes | yes |
| `check-windows` (new) | yes | yes | yes | yes |
| `e2e` (existing) | yes | no | no | yes |
| `build-linux` (new) | no | no | yes | yes |
| `build-windows` (new) | no | no | yes | yes |

Two rulings from the whole-branch review changed this table from the original design.
**The release event is `types: [published]`, not `created`** — a release drafted first and
published later fires only `published`, so `created` would build nothing for the flow that
writes the notes before shipping; listing both would build a direct publish twice. **The
two check jobs run on `release` too, and `build-linux`/`build-windows` declare
`needs: [version, check, check-windows]`**, so a tag can never ship artifacts from a tree
CI has not tested. `e2e` still does not run on `release`. Every `if:` in the workflow
tests `github.event_name == 'release'` only, never the release type.

`check-windows` runs on `windows-latest`: Rust stable, Node 22, `npm ci`, `npm run build`
in `app`, then `cargo check --workspace --all-targets` from `rewrite`. It compiles the
Windows code paths that have never been built; it runs no tests.

### Version

One `version` job (`ubuntu-latest`, seconds) with `outputs.version`; both build jobs
`needs` it and read `${{ needs.version.outputs.version }}`. It was a duplicated step in
both build jobs in the original design — the semver rules now live in exactly one place:

- On `release`: `VERSION="${GITHUB_REF_NAME#v}"`. The job fails unless `VERSION` matches
  `^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$`. The tag itself must start with `v`.
- On `workflow_dispatch`: `VERSION=0.0.0-dev`. Dispatch builds are dry runs: artifacts are
  uploaded to the run, never attached to a release, and the app inside never checks for
  updates (a version containing `dev` suppresses the check).

The source keeps `"version": "0.9.0-dev"` in `tauri.conf.json` and `Cargo.toml`. The build
passes the version with `npx tauri build --config '{"version":"<VERSION>"}'`, which is what
`package_info().version` reports at runtime. Tauri validates semver, and NSIS accepts
pre-release identifiers. The first release tag must sort above the last Python tag
`v0.8.0-beta4`.

### `build-linux` (`ubuntu-22.04`)

1. Checkout, apt deps as in `check`, Rust stable, `Swatinem/rust-cache`, Node 22, `npm ci`.
2. `npx tauri build --bundles appimage --config '{"version":"<VERSION>"}'` from `app`.
   Output: `rewrite/target/release/bundle/appimage/GRID Launcher_<VERSION>_amd64.AppImage`.
3. Repack for update information. Tauri's bundler embeds none, and AppImageUpdate reads it
   from the ELF section appimagetool writes:
   - `./"GRID Launcher_<VERSION>_amd64.AppImage" --appimage-extract` → `squashfs-root/`
   - download `appimagetool-x86_64.AppImage` (continuous) as today
   - `./appimagetool --appimage-extract-and-run -u "gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync" squashfs-root dist/grid-launcher-<VERSION>-x86_64.AppImage`
   - move the generated `.zsync` beside it in `dist/` (appimagetool may write it to the
     working directory; same guard as the Python workflow).
4. `actions/upload-artifact` with both files. On `release` only, attach both with
   `softprops/action-gh-release@v2`.

Asset names are unchanged from the Python releases. Consequence: a Python AppImage that
still carries the embedded update string will update itself to the Rust build through
AppImageUpdate. Part 3 exists so that switch costs the user nothing but their tokens.

### `build-windows` (`windows-latest`)

1. Checkout, Rust stable, `rust-cache`, Node 22, `npm ci`.
2. `npx tauri build --bundles nsis --config '{"version":"<VERSION>"}'`.
   Output: `rewrite/target/release/bundle/nsis/GRID Launcher_<VERSION>_x64-setup.exe`.
3. Rename to `grid-launcher-<VERSION>-windows-x86_64-setup.exe`.
4. Upload artifact; on `release`, attach.

NSIS settings in `tauri.conf.json`: `bundle.windows.nsis.installMode = "currentUser"` and
the default WebView2 `downloadBootstrapper`. No signing.

## Part 2 — Identity

`tauri.conf.json`: `productName` `"GRID Launcher"`, `identifier`
`"io.github.sixdd6.gridlauncher"`, `bundle.targets` `["appimage", "nsis"]` (each job narrows
with `--bundles`). `version` stays `0.9.0-dev`. The two plan documents that mention the old
identifier are history and are not edited. The Rust config directory
(`ProjectDirs("io.github","Sixdd6","grid-launcher")`) does not depend on the identifier, so
existing dev-machine state survives except the window-state file.

## Part 3 — Importer

### Trigger

In the Tauri setup hook, before any service loads the config:

```
if !Config::default_path().exists() && python_config_path().exists() { import }
```

`python_config_path()` is `<home>/.grid-launcher/config.json` on every platform (doc 02,
persistence root). `GRID_LAUNCHER_DATA_DIR` (the data-dir override) moves only the Rust
side; the Python path is fixed. The check is by file presence, so the importer runs at most
once per profile: the import writes `config.toml` whether or not every row converted.

### Module

`grid-core/src/import_python.rs`:

```rust
pub struct ImportReport { pub emulators: usize, pub games: usize, pub skipped_games: usize }
pub fn import(python_json: &Path, config_path: &Path, registry: &Registry, now: i64)
    -> Result<ImportReport, ImportError>;
pub fn plan(python_json: &str) -> Result<(Config, Vec<InstalledGame>, usize), ImportError>; // pure
```

`plan` is the pure conversion (tested); `import` reads the file, calls `plan`, upserts the
rows, saves the config, and returns the report. Errors: file unreadable or not a JSON object
→ `ImportError::Unreadable` / `ImportError::Malformed`; the caller logs one warning and
starts fresh with no file written. A registry or config write error → `ImportError::Write`
after which the caller still starts (the config may exist or not; either way the next run
is consistent).

### Mapping (Python key → Rust field)

Every value passes through the same normalisers doc 02 specifies for Python (trim; blank
emulator name drops the entry; blank title or platform drops the game; save-strategy alias
table; lenient booleans; the upload delay clamped to `[0, 60]`).

| Python | Rust |
| --- | --- |
| `server_url`, `username`, `library_path`, `launch_args` | same names |
| `debug_prints` | `debug_prints` |
| `theme` (`system`/`dark`/`light`, else `system`) | `theme` |
| `emulators[]` (name, path, args, save_strategy, ignore_files, ignore_extensions, save_paths, state_paths, source_id, source_provider, source_owner, source_repo, source_release_tag) | `emulators[]` same names; `source_installed_tag` blank |
| `default_emulators` | `default_emulators` |
| `default_retroarch_cores` | `retroarch_cores` |
| `default_compat_tool` | `default_compat_tool` |
| `compat_tool_installs{id: {name, compat_tool_type, install_path}}` | `compat_tool_installs[]` with `source_id = id`, `name`, `path = install_path`, `release_tag` blank; `compat_tool_type` dropped |
| the four `auto_cloud_save_*` keys | same names |
| `retroachievements_username` | same |
| `native_manual_save_paths` (`<title>__manual` → list) | `native_manual_save_paths` unchanged keys |
| `cloud_sync_state` (object) | `cloud_sync_state` as a TOML table via `serde_json::Value` → `toml::Value`; an unconvertible subtree is dropped, the rest kept |
| `installed_games[]` | one `InstalledGame` each: `rom_id` parsed as `i64` else `None`; `filesize_bytes` parsed else `0`; `installed_at = now`; `last_played_at = 0`; `cover_small_path`/`cover_large_path` blank (covers refetch from the server); `screenshot_urls`, `genres`, `regions`, `rating`, `description`, `rom_file_name`, `archive_path`, `extracted_path`, `extracted_dir`, `multi_file_game_dir`, `native_*`, `included_dlc`, `ps3_*`, `ps4_*`, `ra_id`, `server_updated_at` copied; `languages`, `tags`, `revision`, `companies`, `first_release_date` blank (Python does not store them) |

Skipped on purpose: `api_token`, `retroachievements_api_key`, `retroachievements_token`
(the user enters tokens again; the Python keyring entries under service `GRIDLauncher` are
never read), `first_run_completed`, `window_geometry`, `window_state`,
`emulator_source_installs` (the entry-level `source_*` fields already carry provenance),
the three `tv_*` keys and `tv_mode_last_active`, `cached_cover_path`, `cover_url`,
`local_path`. Unknown keys are ignored, never copied into `Config::extra`.

A game whose `rom_id` is absent is still imported; the registry keys on title and platform.
`skipped_games` counts rows dropped for a blank title or platform.

### Secrets

The importer never reads, logs, or writes a token. `serde_json` deserialises into a typed
struct that has no field for the three secret keys, so their values are never held. The
report and every log line carry counts only.

### Surfacing

`AppState` gains `python_import: Option<ImportReport>` (set once at startup). Command
`python_import_notice` returns it, and the frontend, on mount, shows one toast:
"Imported N emulators and M games from the previous version. Enter your RomM token to
reconnect." with the RetroAchievements sentence appended only when a username was imported.
Same late-mount pattern as the app-update notice (doc 10 D-10-k), no event.

Each count is pluralised (`app/src/lib/pythonImport.ts`): `0` renders as "no emulators" /
"no games", `1` as "1 emulator" / "1 game", anything else as "N emulators" / "N games".
The original fixed string is otherwise unchanged. `skipped_games` is never shown.

The toast host is `App.svelte`, above the phase branch — not `Shell.svelte`. An imported
profile has no credential, so the app comes up on `Connect`, which is exactly where this
notice has to be readable (review H1). It is pushed with an explicit 20 s duration rather
than the routine `TOAST_DURATION_MS` of 4 s: it is a one-time instruction to go and fetch
two tokens (review L4).

`RestoreOutcome::NoSession` carries the config's `server_url` and `username` (blank when
there is no config), threaded through `restore_session` and `applyRestore` so `Connect`
starts with both fields prefilled — the toast promises that only the tokens are missing
(review M1). `libraryPath` stays blank: an imported path is already stored, and `Connect`
writes only a non-blank one.

### The Python config path

`python_import::python_config_path()` is `<home>/.grid-launcher/config.json` unless
`GRID_LAUNCHER_PYTHON_CONFIG` is set and non-empty, in which case that path is used — in
every build, not just `e2e`. The original design compiled the import out of `e2e` builds,
which left the whole startup path without end-to-end coverage (review M3). `scripts/e2e.sh`
now exports a non-existent path for every stage, so no throwaway profile can read a
developer's real `~/.grid-launcher/config.json`, and points the `python-import` stage group
at a fixture written with that stage's mock RomM URL.

That stage group is listed FIRST and clears the RomM keyring item before it starts
(`secret-tool`, best effort). A run's private gnome-keyring is shared by every stage and
its item is keyed by a fixed service/account, so a credential an earlier group saved would
let the app reconnect and never render the Connect screen the stage asserts on.

### Tests

grid-core: `plan` from a fixture that covers every row of the mapping table, including a
blank-name emulator, a blank-platform game, a non-numeric `rom_id`, a string boolean, a
delay of `900`, an unknown top-level key, and the three secret keys set to non-empty values
(assert they appear nowhere in the output config's TOML text). `import` against a tempdir:
writes config and rows; second call is not reached because the caller's presence check
fails (tested at the app layer); malformed JSON writes nothing; `GRID_LAUNCHER_DATA_DIR`
is honoured for the Rust path only, and `GRID_LAUNCHER_PYTHON_CONFIG` overrides the Python
path (set, empty, and unset). App layer: a startup test with a Python file present
and no Rust config yields a notice; with a Rust config present yields none. Frontend:
`App` in phase `none` renders a toast pushed into the store; `Connect` prefills the server
URL from the restored session and leaves the library path blank; `pushToast` honours an
explicit duration. E2E: the `python-import` stage group starts the app on an empty data dir
with a fixture Python config and asserts both the notice and the prefilled form.

## Docs

- `docs/porting/02-config-and-secrets.md`: new "Rust port deviations — importer" section
  with the mapping table and the skipped list.
- `docs/porting/10-identity-updates.md`, External surfaces: the release asset names, the
  NSIS installer, and that the update string is embedded by a repack step.
- Memory: the CI note ("manual-only until parity") is closed.

## Risks

- The Windows build has never compiled. `check-windows` on every push exposes that before
  the first release; the first release may need a fix pass.
- Repacking Tauri's AppImage assumes `--appimage-extract` reproduces a valid AppDir
  (`AppRun`, `.desktop`, icon at the root). Tauri's bundler writes all three; the job
  lists `squashfs-root` before repacking so a failure is diagnosable from the log.
- A Python user's covers are blank until the library view refetches them.
