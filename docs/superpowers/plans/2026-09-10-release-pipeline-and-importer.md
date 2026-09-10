# Release Pipeline and Python-Config Importer — Implementation Plan (2026-09-10)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. One implementer at a time; the conflict map below says which tasks may never run concurrently.

**Goal:** Ship the first Rust release from GitHub Actions — an AppImage carrying embedded update information and a Windows NSIS installer, built from a release tag — retire the three Python build workflows, and carry an existing Python user's configuration and installed library into the Rust app on first launch, tokens excepted.

**Architecture:** The pure conversion (Python `config.json` text → `Config` + `Vec<InstalledGame>`) lives in a new `crates/grid-core/src/import_python.rs`, with `plan` (pure, fixture-tested) split from `import` (file read, registry upsert, config save). `app/src-tauri/src/python_import.rs` owns the startup presence check and stores the result in `AppState.python_import`; the frontend pulls it once on mount through a `python_import_notice` command and shows one toast, the same late-mount pattern as the self-update notice (doc 10 D-10-k). The release pipeline is one workflow, `.github/workflows/build.yml`, whose build jobs pass the version to Tauri with `--config '{"version":"…"}'` and repack the AppImage with `appimagetool -u` so AppImageUpdate finds update information Tauri's bundler does not embed.

**Tech Stack:** Rust (serde, serde_json, toml, rusqlite, tempfile, thiserror, tracing), Tauri 2, Svelte 5 + TypeScript, vitest, svelte-check, GitHub Actions, appimagetool, NSIS.

**Spec:** `docs/superpowers/specs/2026-09-10-release-pipeline-and-importer-design.md` — the binding authority. Read it alongside this plan.

**Reference:** The Python app at the repo root is the behaviour oracle. `docs/porting/*.md` are the ported specs; `docs/porting/02-config-and-secrets.md` lines 126-228 hold the Python schema tables this importer converts from.

## Global Constraints

- **Token secrecy (hard requirement):** tokens/credentials never in files, logs, errors, IPC, console output, or test fixtures' outputs. The importer deserialises into a struct with no field for `api_token`, `retroachievements_api_key`, `retroachievements_token`; their values are therefore never held, never logged and never written. Every import log line and the `ImportReport` carry counts only.
- **No destructive git:** subagents never run `git checkout` / `git restore` / `git reset` / `git stash` on tracked files.
- **Committing:** commit from the repo root with `git commit --only -- <paths>`. Commit after every completed, verified task. Subjects start with `rewrite: ` for code, `docs: ` for documentation-only tasks, `ci: ` for workflow-only tasks.
- **Docs travel with behaviour:** every behaviour change updates `docs/porting` in the same task.
- **The Python app is the behaviour oracle.** When this plan and the Python code disagree, read the Python source named in the step and adjust the step, not the intent.
- **Commit messages end with:**

  ```
  Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
  ```

- **Rust gate, run from `rewrite/` at the end of every Rust task:**

  ```bash
  cd rewrite && cargo test -p grid-core -p app
  cargo clippy --workspace --all-targets -- -D warnings
  cargo clippy -p app --all-targets --features e2e -- -D warnings
  cargo fmt --all --check
  ```

- **Frontend gate, added by tasks that touch `rewrite/app/src`:**

  ```bash
  cd rewrite/app && npx vitest run && npx svelte-check
  ```

  `npx svelte-check` has a baseline of 3 warnings (Details.svelte ×2, DownloadsFooter.svelte ×1). The count must not rise.
- **Secret-hygiene gate, run from the repo root by Tasks 2, 3 and 4:** `bash rewrite/scripts/check_secret_hygiene.sh`.
- **TDD:** every step marked "test first" writes the failing test, runs it to see it fail, then implements.
- All `rewrite/` paths below are relative to the repo root unless a step says otherwise. `docs/` paths are relative to the repo root.

## File structure

| File | Responsibility | Task |
| --- | --- | --- |
| `rewrite/app/src-tauri/tauri.conf.json` | Product identity, bundle targets, NSIS settings | 1 |
| `rewrite/crates/grid-core/src/import_python.rs` (new) | The whole importer: lenient Python types, normalisers, `plan`, `import`, `ImportReport`, `ImportError` | 2, 3 |
| `rewrite/crates/grid-core/src/lib.rs` | `pub mod import_python;` | 2 |
| `rewrite/app/src-tauri/src/python_import.rs` (new) | Startup presence check, Python config path, logging, `startup_import` | 4 |
| `rewrite/app/src-tauri/src/commands.rs` | `AppState.python_import` field | 4 |
| `rewrite/app/src-tauri/src/commands/updates.rs` | `python_import_notice` command | 4 |
| `rewrite/app/src-tauri/src/lib.rs` | Module declaration, startup call, command registration | 4 |
| `rewrite/app/src/lib/api.ts` | `PythonImportReport` type + `pythonImportNotice` binding | 4 |
| `rewrite/app/src/lib/pythonImport.ts` (new) | Pure toast-text builder | 4 |
| `rewrite/app/src/lib/stores/pythonImport.svelte.ts` (new) | Once-per-process pull + toast push | 4 |
| `rewrite/app/src/App.svelte` | Mount effect for the store | 4 |
| `.github/workflows/build.yml` (renamed from `rust-rewrite.yml`) | The single CI + release pipeline | 5 |
| `.github/workflows/appimage-linux.yml`, `pyinstaller-linux.yml`, `pyinstaller-windows.yml` | Deleted | 5 |
| `docs/porting/02-config-and-secrets.md` | "Rust port deviations — importer" section | 6 |
| `docs/porting/10-identity-updates.md` | External surfaces: release asset names, NSIS installer, repack step | 6 |

## Conflict map

| Tasks | Shared files | Rule |
| --- | --- | --- |
| 2, 3 | `crates/grid-core/src/import_python.rs` | Strictly serial: Task 3 extends the file Task 2 creates. |
| 3, 4 | `import_python.rs` (`import`, `ImportReport`) → app layer | Serial: Task 4 consumes Task 3's `import`. |
| 1, 5 | none | May run concurrently with 2/3/4. |
| 6 | docs only | Runs last, after 1-5, so it documents what shipped. |

Recommended order: **1 → 2 → 3 → 4 → 5 → 6**. Tasks 1 and 5 may run in parallel with 2-4 if two implementers are available.

## Ambiguities resolved (read before Task 2)

These are places the spec's mapping table and the actual Rust types disagree. The resolution below is binding for this plan.

1. **`theme` maps to `config.ui.theme`, not a top-level `theme`.** `Config` (`crates/grid-core/src/config.rs:151-262`) has no top-level `theme` field; the appearance theme lives in `UiSettings.theme` (`config.rs:218-225`), defaulting to `"system"`. The spec's row `theme → theme` therefore means `ui.theme`.
2. **`installed_at` is stamped by `import`, not by `plan`.** The spec's signature for `plan` takes no `now`, so `plan` leaves `installed_at: 0` and `import` overwrites it with `now` on every row before `Registry::upsert`.
3. **The import runs in `run()`, right after `Registry::open`, not inside the `setup` hook.** The spec says "before any service loads the config". In `rewrite/app/src-tauri/src/lib.rs` the registry is opened at line ~78 and `AppState` is constructed at line ~90, both before `setup`. Running there is strictly earlier than the setup hook and lets `AppState.python_import` be a plain field instead of a lock. The only earlier config read is `Config::load` for the logging filter (lib.rs:47), which writes nothing.
4. **`ImportReport` gains a `retroachievements: bool`.** The spec's toast appends a RetroAchievements sentence "only when a username was imported", but its `ImportReport` has no field to carry that. The field is `pub retroachievements: bool`, set to `!config.retroachievements_username.is_empty()`.
5. **The RetroAchievements sentence text.** The spec fixes the first two sentences but not this one. It is: `Enter your RetroAchievements token as well.`
6. **A non-object row in `installed_games` counts as skipped.** Python's normaliser silently drops non-dict rows; here a non-object row deserialises to a blank-title game and is counted in `skipped_games`. This is a deliberate, documented difference — the count is a diagnostic, not a contract.

---

### Task 1: Product identity and NSIS settings

**Files:**
- Modify: `rewrite/app/src-tauri/tauri.conf.json` — `productName`, `identifier`, `bundle.targets`, new `bundle.windows.nsis` block.

**Interfaces:**
- Produces: the AppImage that Task 5's `build-linux` repacks is named `GRID Launcher_<VERSION>_amd64.AppImage` (derived from `productName`), and the NSIS output is `GRID Launcher_<VERSION>_x64-setup.exe`. Task 5's paths depend on `productName` being exactly `GRID Launcher`.

**Background:** the file today reads `"productName": "GRID Launcher (Rust preview)"`, `"identifier": "io.github.sixdd6.gridlauncher2"`, `"targets": ["appimage"]`. `version` stays `"0.9.0-dev"` — the build overrides it per job. The Rust config directory comes from `ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")` (`crates/grid-core/src/config.rs:298-302`) and does **not** depend on the identifier, so existing dev-machine state survives the identifier change; only `tauri-plugin-window-state`'s `.window-state.json` moves, which is expected and harmless.

- [ ] **Step 1: Edit `rewrite/app/src-tauri/tauri.conf.json`**

Change the three existing keys and add the `windows` block inside `bundle`. The resulting file:

```json
{
  "$schema": "../node_modules/@tauri-apps/cli/config.schema.json",
  "productName": "GRID Launcher",
  "version": "0.9.0-dev",
  "identifier": "io.github.sixdd6.gridlauncher",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "GRID Launcher",
        "width": 800,
        "height": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "capabilities": ["default"],
      "assetProtocol": {
        "enable": true,
        "scope": ["$CACHE/grid-launcher/covers/**/*"]
      },
      "csp": {
        "default-src": "'self'",
        "img-src": "'self' asset: http://asset.localhost https://img.youtube.com",
        "media-src": "'self' http://127.0.0.1:*",
        "connect-src": "ipc: http://ipc.localhost",
        "style-src": "'unsafe-inline' 'self'"
      }
    }
  },
  "bundle": {
    "active": true,
    "targets": ["appimage", "nsis"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "webviewInstallMode": {
        "type": "downloadBootstrapper"
      },
      "nsis": {
        "installMode": "currentUser"
      }
    },
    "android": {
      "debugApplicationIdSuffix": ".debug"
    }
  }
}
```

`downloadBootstrapper` is Tauri's default; it is written out so the release contract is visible in the file rather than implied. No signing keys are configured — the spec puts code signing out of scope.

- [ ] **Step 2: Verify the file is valid JSON and carries the four values**

Run, from the repo root:

```bash
node -e '
const c = JSON.parse(require("fs").readFileSync("rewrite/app/src-tauri/tauri.conf.json", "utf8"));
const checks = [
  ["productName", c.productName === "GRID Launcher"],
  ["identifier", c.identifier === "io.github.sixdd6.gridlauncher"],
  ["version", c.version === "0.9.0-dev"],
  ["targets", JSON.stringify(c.bundle.targets) === JSON.stringify(["appimage","nsis"])],
  ["installMode", c.bundle.windows.nsis.installMode === "currentUser"],
  ["webview", c.bundle.windows.webviewInstallMode.type === "downloadBootstrapper"],
];
const bad = checks.filter(([, ok]) => !ok).map(([k]) => k);
if (bad.length) { console.error("WRONG: " + bad.join(", ")); process.exit(1); }
console.log("tauri.conf.json OK");
'
```

Expected: `tauri.conf.json OK`. A parse error or a `WRONG: …` line means the edit is not finished. A full `npx tauri build` is **not** required for this task.

- [ ] **Step 3: Verify Tauri itself accepts the config**

Run:

```bash
cd rewrite/app && npx tauri info
```

Expected: the report prints and ends without an error; the `tauri.conf.json` section shows no schema complaint. If `npx tauri info` cannot reach the network for its version checks it still parses the config — only a parse/schema error is a failure here.

- [ ] **Step 4: Commit**

```bash
git commit --only -- rewrite/app/src-tauri/tauri.conf.json -m "$(cat <<'MSG'
rewrite: name the release build GRID Launcher and add the NSIS target

productName drops the "(Rust preview)" suffix, the identifier drops its
"2" suffix, appimage and nsis are both bundle targets, and NSIS installs
per user with the downloadBootstrapper WebView2 mode.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

### Task 2: `import_python::plan` — the pure conversion

**Files:**
- Create: `rewrite/crates/grid-core/src/import_python.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs` — add `pub mod import_python;` in alphabetical order (after `pub mod images;`, before `pub mod launch;`).
- Test: inside `import_python.rs`, a `#[cfg(test)] mod tests` block (the crate's convention — see `crates/grid-core/src/config.rs` and `autoconfig/entry.rs`).

**Interfaces:**
- Produces, for Task 3 and Task 4:

  ```rust
  pub struct ImportReport {
      pub emulators: usize,
      pub games: usize,
      pub skipped_games: usize,
      pub retroachievements: bool,
  }
  pub enum ImportError { Unreadable, Malformed, Write(String) }
  pub fn plan(python_json: &str)
      -> Result<(crate::config::Config, Vec<crate::library::registry::InstalledGame>, usize), ImportError>;
  ```

  `plan`'s third tuple element is `skipped_games`.
- Consumes: `crate::autoconfig::entry::normalize_save_strategy(&str) -> String` (`crates/grid-core/src/autoconfig/entry.rs:87-95`, the ported alias table); `crate::config::{Config, EmulatorEntry, CompatToolInstall}`; `crate::library::registry::InstalledGame`.

**Field-name facts (verified, do not guess):**
- `EmulatorEntry` (`config.rs:7-63`): `name, path, args, source_id, source_provider, source_owner, source_repo, source_release_tag, source_installed_tag, save_strategy, ignore_files, ignore_extensions, save_paths, state_paths`.
- `CompatToolInstall` (`config.rs:70-80`): `name, path, source_id, release_tag`.
- `Config` (`config.rs:151-262`): the fields used here are `server_url, username, library_path, emulators, default_emulators, retroarch_cores, launch_args, retroachievements_username, auto_cloud_save_download_on_launch, auto_cloud_save_upload_on_exit, auto_cloud_save_skip_download_if_local_newer, auto_cloud_save_upload_delay_seconds, cloud_sync_state, native_manual_save_paths, default_compat_tool, compat_tool_installs, debug_prints, ui`. `Config::default()` supplies everything else, including `schema_version: 1` and an empty `extra`.
- `InstalledGame` (`library/registry.rs:246-311`): all the fields named in the mapping, plus `fanart_urls` and `images_version`, which the Python config has no source for and which `..Default::default()` leaves blank/zero.

**Python oracle:** `grid_launcher/core/config.py:8-212` (`normalize_emulators`, `normalize_default_emulators`, `normalize_default_retroarch_cores`, `normalize_installed_games`, `normalize_compat_tool_installs`); `grid-launcher.py:2185-2221` (`_config_bool`, `_config_int`, the `[0, 60]` delay clamp); `grid_launcher/ui/theme.py:140-146` (`normalized_theme_choice`); `grid_launcher/emulator/profiles.py:141-156` (save-strategy aliases).

- [ ] **Step 1: Write the fixture**

Create the fixture as a `const` inside the test module (no separate file — the crate keeps fixtures inline). It covers every mapping row, a blank-name emulator, a blank-platform game, a blank-title game, a non-numeric `rom_id`, a non-numeric `filesize_bytes`, string booleans, a delay of `900`, an unknown top-level key, and all three secret keys set to non-empty values.

```rust
    /// A Python `~/.grid-launcher/config.json` exercising every row of the
    /// mapping table in `docs/superpowers/specs/2026-09-10-release-pipeline-and-importer-design.md`.
    /// The three `SECRET-*` values must never appear in anything `plan`
    /// produces — `secrets_never_reach_the_imported_config` asserts it.
    const PYTHON_CONFIG: &str = r#"{
  "server_url": "  https://romm.example.test  ",
  "api_token": "SECRET-ROMM-TOKEN-DO-NOT-IMPORT",
  "username": " ash ",
  "library_path": "/games",
  "first_run_completed": true,
  "launch_args": "--fullscreen",
  "debug_prints": "off",
  "theme": "DARK",
  "window_geometry": "AdnQywADAAAAAAAA",
  "window_state": "maximized",
  "emulators": [
    {
      "name": "  RetroArch  ",
      "path": "/opt/retroarch/retroarch",
      "args": "  -L %core% %rom%  ",
      "save_strategy": "Single-File",
      "ignore_files": "notes.txt",
      "ignore_extensions": ".log",
      "save_paths": "saves",
      "state_paths": "states",
      "source_id": "libretro/RetroArch",
      "source_provider": "github",
      "source_owner": "libretro",
      "source_repo": "RetroArch",
      "source_release_tag": "latest"
    },
    { "name": "   ", "path": "/opt/ghost" },
    { "name": "PCSX2", "path": "/opt/pcsx2/pcsx2" }
  ],
  "default_emulators": { " Nintendo 64 ": "RetroArch", "PlayStation 2": "PCSX2" },
  "default_retroarch_cores": { " n64 ": "  mupen64plus_next  ", "empty": "   " },
  "installed_games": [
    {
      "title": " Chrono Trigger ",
      "platform": " SNES ",
      "rom_id": "4321",
      "ra_id": "10024",
      "server_updated_at": "2026-01-02T03:04:05Z",
      "rom_file_name": "Chrono Trigger.sfc",
      "archive_path": "/games/SNES/Chrono Trigger.zip",
      "extracted_path": "",
      "extracted_dir": "",
      "multi_file_game_dir": "",
      "description": "A time-travel RPG.",
      "rating": "9.5",
      "genres": "RPG",
      "regions": "USA",
      "filesize_bytes": "4194304",
      "screenshot_urls": "https://img.example.test/a.png",
      "cover_url": "https://img.example.test/cover.png",
      "cached_cover_path": "/home/ash/.grid-launcher/imagecache/ct.png",
      "local_path": "/games/SNES/Chrono Trigger.sfc"
    },
    {
      "title": "Portal 2",
      "platform": "Windows",
      "rom_id": "not-a-number",
      "filesize_bytes": "huge",
      "native_executable_path": "/games/Windows/Portal 2/portal2.exe",
      "native_launch_parameters": "-novid",
      "native_compat_tool": "GE-Proton9-27",
      "native_wineprefix": "/prefixes/portal2",
      "native_game_dir": "/games/Windows/Portal 2",
      "included_dlc": "Peer Review"
    },
    {
      "title": "Ratchet & Clank",
      "platform": "PlayStation 3",
      "rom_id": "88",
      "ps3_game_id": "bces00141",
      "ps3_iso_path": "/games/PS3/rc.iso",
      "ps3_trophy_paths": "/trophy/NPWR00001",
      "ps4_game_id": "cusa00001",
      "ps4_content": "update",
      "extracted_dir": "/games/PS3/Ratchet",
      "extracted_path": "/games/PS3/Ratchet/PS3_GAME",
      "multi_file_game_dir": "/games/PS3/Ratchet"
    },
    { "title": "Demon's Souls", "platform": "   ", "rom_id": "77" },
    { "title": "   ", "platform": "PS3" }
  ],
  "emulator_source_installs": { "retroarch": { "tag": "v1.19.1" } },
  "compat_tool_installs": {
    "GE-Proton9-27": {
      "name": "GE-Proton9-27",
      "compat_tool_type": "proton",
      "install_path": "/home/ash/.local/share/grid-launcher/compat-tools/GE-Proton9-27"
    },
    "blank": { "name": "  ", "compat_tool_type": "proton", "install_path": "/nowhere" }
  },
  "default_compat_tool": "GE-Proton9-27",
  "auto_cloud_save_download_on_launch": "yes",
  "auto_cloud_save_upload_on_exit": false,
  "auto_cloud_save_skip_download_if_local_newer": "nonsense",
  "auto_cloud_save_upload_delay_seconds": 900,
  "cloud_sync_state": {
    "rom:4321": {
      "last_uploaded_at": 1757000000,
      "last_hash": "abc123",
      "dirty": false,
      "note": null
    }
  },
  "native_manual_save_paths": { "portal 2__manual": ["/home/ash/saves/portal2", "   "] },
  "retroachievements_username": " ashley ",
  "retroachievements_api_key": "SECRET-RA-KEY-DO-NOT-IMPORT",
  "retroachievements_token": "SECRET-RA-TOKEN-DO-NOT-IMPORT",
  "tv_mode_home_view": "home",
  "tv_guide_button_exclusion_list": ["rpcs3"],
  "tv_guide_button_default_opt_outs": [],
  "tv_mode_last_active": true,
  "an_unknown_future_key": { "kept": "no" }
}"#;
```

- [ ] **Step 2 (test first): Write the failing tests**

Append to the same `#[cfg(test)] mod tests` block, under the fixture:

```rust
    use super::*;

    fn planned() -> (Config, Vec<InstalledGame>, usize) {
        plan(PYTHON_CONFIG).expect("the fixture is a valid JSON object")
    }

    #[test]
    fn scalars_are_trimmed_and_theme_lands_in_the_ui_table() {
        let (config, _, _) = planned();
        assert_eq!(config.server_url, "https://romm.example.test");
        assert_eq!(config.username, "ash");
        assert_eq!(config.library_path, "/games");
        assert_eq!(config.launch_args, "--fullscreen");
        assert_eq!(config.retroachievements_username, "ashley");
        assert_eq!(config.default_compat_tool, "GE-Proton9-27");
        // `theme` has no top-level home in `Config`; it is `ui.theme`.
        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.schema_version, Config::default().schema_version);
    }

    #[test]
    fn an_unrecognized_theme_collapses_to_system() {
        let (config, _, _) = plan(r#"{"theme": "solarized"}"#).unwrap();
        assert_eq!(config.ui.theme, "system");
    }

    #[test]
    fn booleans_are_lenient_and_unknown_words_keep_the_default() {
        let (config, _, _) = planned();
        assert!(!config.debug_prints); // "off"
        assert!(config.auto_cloud_save_download_on_launch); // "yes"
        assert!(!config.auto_cloud_save_upload_on_exit); // real JSON false
        // "nonsense" is neither on nor off, so the default (true) stands.
        assert!(config.auto_cloud_save_skip_download_if_local_newer);
    }

    #[test]
    fn the_upload_delay_is_clamped_to_sixty() {
        let (config, _, _) = planned();
        assert_eq!(config.auto_cloud_save_upload_delay_seconds, 60);
        let (low, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": -5}"#).unwrap();
        assert_eq!(low.auto_cloud_save_upload_delay_seconds, 0);
        let (text, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": "12"}"#).unwrap();
        assert_eq!(text.auto_cloud_save_upload_delay_seconds, 12);
        let (bad, _, _) = plan(r#"{"auto_cloud_save_upload_delay_seconds": true}"#).unwrap();
        assert_eq!(bad.auto_cloud_save_upload_delay_seconds, 3);
    }

    #[test]
    fn emulators_drop_blank_names_normalize_and_sort() {
        let (config, _, _) = planned();
        let names: Vec<&str> = config.emulators.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["PCSX2", "RetroArch"]);

        let retroarch = &config.emulators[1];
        assert_eq!(retroarch.path, "/opt/retroarch/retroarch");
        assert_eq!(retroarch.args, "-L %core% %rom%");
        assert_eq!(retroarch.save_strategy, "single_file");
        assert_eq!(retroarch.ignore_files, "notes.txt");
        assert_eq!(retroarch.ignore_extensions, ".log");
        assert_eq!(retroarch.save_paths, "saves");
        assert_eq!(retroarch.state_paths, "states");
        assert_eq!(retroarch.source_id, "libretro/RetroArch");
        assert_eq!(retroarch.source_provider, "github");
        assert_eq!(retroarch.source_owner, "libretro");
        assert_eq!(retroarch.source_repo, "RetroArch");
        assert_eq!(retroarch.source_release_tag, "latest");
        // Nothing on disk vouches for the installed tag, so it starts blank.
        assert_eq!(retroarch.source_installed_tag, "");

        let pcsx2 = &config.emulators[0];
        assert_eq!(pcsx2.args, "%rom%"); // missing args default, config.py:36-37
        assert_eq!(pcsx2.save_strategy, "auto");
    }

    #[test]
    fn maps_are_trimmed_and_blank_cores_are_dropped() {
        let (config, _, _) = planned();
        assert_eq!(config.default_emulators.get("Nintendo 64").map(String::as_str), Some("RetroArch"));
        assert_eq!(config.default_emulators.get("PlayStation 2").map(String::as_str), Some("PCSX2"));
        assert_eq!(config.retroarch_cores.get("n64").map(String::as_str), Some("mupen64plus_next"));
        assert!(!config.retroarch_cores.contains_key("empty"));
    }

    #[test]
    fn compat_tool_installs_become_a_list_keyed_by_source_id() {
        let (config, _, _) = planned();
        assert_eq!(config.compat_tool_installs.len(), 1);
        let tool = &config.compat_tool_installs[0];
        assert_eq!(tool.name, "GE-Proton9-27");
        assert_eq!(tool.source_id, "GE-Proton9-27");
        assert_eq!(
            tool.path,
            "/home/ash/.local/share/grid-launcher/compat-tools/GE-Proton9-27"
        );
        assert_eq!(tool.release_tag, "");
    }

    #[test]
    fn cloud_sync_state_becomes_a_toml_table_without_nulls() {
        let (config, _, _) = planned();
        let entry = config.cloud_sync_state["rom:4321"].as_table().expect("a table");
        assert_eq!(entry["last_uploaded_at"].as_integer(), Some(1_757_000_000));
        assert_eq!(entry["last_hash"].as_str(), Some("abc123"));
        assert_eq!(entry["dirty"].as_bool(), Some(false));
        // TOML has no null, so the unconvertible key is dropped and the rest kept.
        assert!(!entry.contains_key("note"));
    }

    #[test]
    fn manual_save_paths_keep_their_keys_and_drop_blank_entries() {
        let (config, _, _) = planned();
        assert_eq!(
            config.native_manual_save_paths.get("portal 2__manual"),
            Some(&vec!["/home/ash/saves/portal2".to_string()])
        );
    }

    #[test]
    fn unknown_keys_are_ignored_rather_than_carried_into_extra() {
        let (config, _, _) = planned();
        assert!(config.extra.is_empty());
    }

    #[test]
    fn games_convert_and_blank_identity_rows_are_counted_as_skipped() {
        let (_, games, skipped) = planned();
        assert_eq!(skipped, 2);
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, vec!["Chrono Trigger", "Portal 2", "Ratchet & Clank"]);

        let ct = &games[0];
        assert_eq!(ct.platform, "SNES");
        assert_eq!(ct.rom_id, Some(4321));
        assert_eq!(ct.filesize_bytes, 4_194_304);
        assert_eq!(ct.ra_id, "10024");
        assert_eq!(ct.server_updated_at, "2026-01-02T03:04:05Z");
        assert_eq!(ct.rom_file_name, "Chrono Trigger.sfc");
        assert_eq!(ct.archive_path, "/games/SNES/Chrono Trigger.zip");
        assert_eq!(ct.description, "A time-travel RPG.");
        assert_eq!(ct.rating, "9.5");
        assert_eq!(ct.genres, "RPG");
        assert_eq!(ct.regions, "USA");
        assert_eq!(ct.screenshot_urls, "https://img.example.test/a.png");
        // `plan` is pure: `import` stamps `installed_at`.
        assert_eq!(ct.installed_at, 0);
        assert_eq!(ct.last_played_at, 0);
        // Covers refetch from the server, so no Python cache path is carried over.
        assert_eq!(ct.cover_small_path, "");
        assert_eq!(ct.cover_large_path, "");
        // Python stores none of these.
        assert_eq!(ct.languages, "");
        assert_eq!(ct.tags, "");
        assert_eq!(ct.revision, "");
        assert_eq!(ct.companies, "");
        assert_eq!(ct.first_release_date, "");
        assert_eq!(ct.fanart_urls, "");

        let portal = &games[1];
        assert_eq!(portal.rom_id, None); // "not-a-number"
        assert_eq!(portal.filesize_bytes, 0); // "huge"
        assert_eq!(portal.native_executable_path, "/games/Windows/Portal 2/portal2.exe");
        assert_eq!(portal.native_launch_parameters, "-novid");
        assert_eq!(portal.native_compat_tool, "GE-Proton9-27");
        assert_eq!(portal.native_wineprefix, "/prefixes/portal2");
        assert_eq!(portal.native_game_dir, "/games/Windows/Portal 2");
        assert_eq!(portal.included_dlc, "Peer Review");

        let ratchet = &games[2];
        assert_eq!(ratchet.ps3_game_id, "BCES00141"); // upper-cased, config.py:178
        assert_eq!(ratchet.ps4_game_id, "CUSA00001"); // config.py:180
        assert_eq!(ratchet.ps3_iso_path, "/games/PS3/rc.iso");
        assert_eq!(ratchet.ps3_trophy_paths, "/trophy/NPWR00001");
        assert_eq!(ratchet.ps4_content, "update");
        assert_eq!(ratchet.extracted_dir, "/games/PS3/Ratchet");
        assert_eq!(ratchet.extracted_path, "/games/PS3/Ratchet/PS3_GAME");
        assert_eq!(ratchet.multi_file_game_dir, "/games/PS3/Ratchet");
    }

    /// The hard requirement: no token value and no secret key name survives
    /// into anything the importer writes.
    #[test]
    fn secrets_never_reach_the_imported_config() {
        let (config, games, _) = planned();
        let text = toml::to_string_pretty(&config).expect("the config serializes");
        for needle in [
            "SECRET-ROMM-TOKEN-DO-NOT-IMPORT",
            "SECRET-RA-KEY-DO-NOT-IMPORT",
            "SECRET-RA-TOKEN-DO-NOT-IMPORT",
            "api_token",
            "retroachievements_api_key",
            "retroachievements_token",
        ] {
            assert!(!text.contains(needle), "{needle} leaked into the config");
        }
        let rows = format!("{games:?}");
        for needle in [
            "SECRET-ROMM-TOKEN-DO-NOT-IMPORT",
            "SECRET-RA-KEY-DO-NOT-IMPORT",
            "SECRET-RA-TOKEN-DO-NOT-IMPORT",
        ] {
            assert!(!rows.contains(needle), "{needle} leaked into a registry row");
        }
    }

    #[test]
    fn a_non_object_document_is_malformed() {
        assert!(matches!(plan("[1, 2, 3]"), Err(ImportError::Malformed)));
        assert!(matches!(plan("not json at all"), Err(ImportError::Malformed)));
        assert!(matches!(plan("null"), Err(ImportError::Malformed)));
    }

    #[test]
    fn an_empty_object_yields_the_defaults() {
        let (config, games, skipped) = plan("{}").unwrap();
        assert_eq!(config, Config::default());
        assert!(games.is_empty());
        assert_eq!(skipped, 0);
    }

    #[test]
    fn mistyped_containers_are_ignored_the_way_python_ignores_them() {
        // `normalize_emulators`/`normalize_installed_games` return [] for a
        // non-list, and the map normalizers return {} for a non-dict.
        let (config, games, skipped) = plan(
            r#"{"emulators": "nope", "installed_games": 7, "default_emulators": [], "cloud_sync_state": 3}"#,
        )
        .unwrap();
        assert!(config.emulators.is_empty());
        assert!(config.default_emulators.is_empty());
        assert!(config.cloud_sync_state.is_empty());
        assert!(games.is_empty());
        assert_eq!(skipped, 0);
    }
```

- [ ] **Step 3: Run the tests and see them fail**

Run: `cd rewrite && cargo test -p grid-core import_python`
Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module 'import_python'` (or `cannot find function 'plan'`), because the module does not exist yet.

- [ ] **Step 4: Write the module**

Create `rewrite/crates/grid-core/src/import_python.rs` with exactly this content above the test module:

```rust
//! One-shot import of a Python-era `~/.grid-launcher/config.json` into the
//! Rust `config.toml` and installed-games registry.
//!
//! Runs at most once per profile: the caller checks that no Rust config
//! exists and that a Python one does (`app/src-tauri/src/python_import.rs`),
//! and a successful import always writes `config.toml`, so the second
//! start never reaches this module.
//!
//! # Secrets
//!
//! This module holds no token, ever. [`PythonConfig`] has no field for
//! `api_token`, `retroachievements_api_key` or `retroachievements_token`,
//! so `serde_json` drops those values while parsing and nothing here can
//! log, copy or write them. [`ImportReport`] carries counts only, and
//! [`ImportError`] carries no path and no file content.
//!
//! The conversion mirrors the reference's own load-time normalizers —
//! `grid_launcher/core/config.py:8-212`, `grid-launcher.py:2185-2221`,
//! `grid_launcher/ui/theme.py:140-146` — because those are what produced
//! the file being read.

use crate::autoconfig::entry::normalize_save_strategy;
use crate::config::{CompatToolInstall, Config, EmulatorEntry};
use crate::library::registry::{InstalledGame, Registry};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::path::Path;

/// What an import did, in counts only. Serialized straight to the frontend
/// by `python_import_notice`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ImportReport {
    /// Emulator entries written to `config.toml`.
    pub emulators: usize,
    /// Registry rows written.
    pub games: usize,
    /// Rows dropped because their title or platform was blank. A row that
    /// was not a JSON object at all counts here too — it deserializes to a
    /// blank-title row. The count is a diagnostic, not a contract.
    pub skipped_games: usize,
    /// Whether a RetroAchievements username came across, so the toast can
    /// tell the user that token needs re-entering as well.
    pub retroachievements: bool,
}

/// Why an import did not happen. No variant carries a path or any file
/// content: the caller logs this text verbatim.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the previous version's config file could not be read")]
    Unreadable,
    #[error("the previous version's config file is not a JSON object")]
    Malformed,
    #[error("the imported data could not be written: {0}")]
    Write(String),
}

// ---------------------------------------------------------------------------
// Lenient input types
//
// The reference guards every field with `isinstance` and falls back to a
// default instead of failing the load (config.py:30-58). These wrappers do
// the same, so one mistyped key in a hand-edited file cannot cost the user
// their whole library.
// ---------------------------------------------------------------------------

/// A JSON value that must be a string. Anything else reads as `""`, exactly
/// like the reference's `x.strip() if isinstance(x, str) else ""`. Trimmed
/// on the way in, because every reference normalizer trims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LenientString(String);

impl LenientString {
    fn as_str(&self) -> &str {
        &self.0
    }

    fn into_string(self) -> String {
        self.0
    }
}

impl<'de> Deserialize<'de> for LenientString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            serde_json::Value::String(text) => text.trim().to_string(),
            _ => String::new(),
        }))
    }
}

/// `_config_bool` (`grid-launcher.py:2185-2196`): a real JSON boolean wins;
/// the strings `1/true/yes/on` and `0/false/no/off` are accepted
/// case-insensitively; anything else means "no opinion" and the caller's
/// default stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LenientBool(Option<bool>);

impl LenientBool {
    fn or(self, default: bool) -> bool {
        self.0.unwrap_or(default)
    }
}

impl<'de> Deserialize<'de> for LenientBool {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            serde_json::Value::Bool(flag) => Some(flag),
            serde_json::Value::String(text) => match text.trim().to_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        }))
    }
}

/// `_config_int` (`grid-launcher.py:2198-2209`): an integer wins, a numeric
/// string is parsed, a boolean is explicitly rejected, everything else
/// means "no opinion".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LenientInt(Option<i64>);

impl LenientInt {
    fn or(self, default: i64) -> i64 {
        self.0.unwrap_or(default)
    }
}

impl<'de> Deserialize<'de> for LenientInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            // Checked before `Number` is even possible; serde_json never
            // parses `true` as a number, but the reference rejects booleans
            // explicitly and this keeps the two readable side by side.
            serde_json::Value::Bool(_) => None,
            serde_json::Value::Number(number) => number.as_i64(),
            serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        }))
    }
}

/// A value of a shape the reference guards with `isinstance`: anything that
/// does not deserialize as `T` falls back to `T::default()` instead of
/// failing the whole document (`if not isinstance(value, list): return []`).
#[derive(Debug, Clone, PartialEq)]
struct Lenient<T>(T);

impl<T: Default> Default for Lenient<T> {
    fn default() -> Self {
        Self(T::default())
    }
}

impl<'de, T: serde::de::DeserializeOwned + Default> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(serde_json::from_value(value).unwrap_or_default()))
    }
}

type JsonMap = serde_json::Map<String, serde_json::Value>;

/// The Python config document, restricted to the keys this importer maps.
///
/// The three secret keys are absent BY DESIGN, so their values are dropped
/// during parsing and never exist in this process. So are
/// `first_run_completed`, `window_geometry`, `window_state`,
/// `emulator_source_installs`, the TV-mode keys and `tv_mode_last_active`;
/// unknown keys are ignored rather than copied into `Config::extra`.
#[derive(Debug, Default, Deserialize)]
struct PythonConfig {
    #[serde(default)]
    server_url: LenientString,
    #[serde(default)]
    username: LenientString,
    #[serde(default)]
    library_path: LenientString,
    #[serde(default)]
    launch_args: LenientString,
    #[serde(default)]
    debug_prints: LenientBool,
    #[serde(default)]
    theme: LenientString,
    #[serde(default)]
    emulators: Lenient<Vec<Lenient<PythonEmulator>>>,
    #[serde(default)]
    default_emulators: Lenient<JsonMap>,
    #[serde(default)]
    default_retroarch_cores: Lenient<JsonMap>,
    #[serde(default)]
    installed_games: Lenient<Vec<Lenient<PythonGame>>>,
    #[serde(default)]
    compat_tool_installs: Lenient<JsonMap>,
    #[serde(default)]
    default_compat_tool: LenientString,
    #[serde(default)]
    auto_cloud_save_download_on_launch: LenientBool,
    #[serde(default)]
    auto_cloud_save_upload_on_exit: LenientBool,
    #[serde(default)]
    auto_cloud_save_skip_download_if_local_newer: LenientBool,
    #[serde(default)]
    auto_cloud_save_upload_delay_seconds: LenientInt,
    #[serde(default)]
    cloud_sync_state: Lenient<JsonMap>,
    #[serde(default)]
    native_manual_save_paths: Lenient<JsonMap>,
    #[serde(default)]
    retroachievements_username: LenientString,
}

/// One `emulators[]` row (`config.py:8-81`).
#[derive(Debug, Default, Deserialize)]
struct PythonEmulator {
    #[serde(default)]
    name: LenientString,
    #[serde(default)]
    path: LenientString,
    #[serde(default)]
    args: LenientString,
    #[serde(default)]
    save_strategy: LenientString,
    #[serde(default)]
    ignore_files: LenientString,
    #[serde(default)]
    ignore_extensions: LenientString,
    #[serde(default)]
    save_paths: LenientString,
    #[serde(default)]
    state_paths: LenientString,
    #[serde(default)]
    source_id: LenientString,
    #[serde(default)]
    source_provider: LenientString,
    #[serde(default)]
    source_owner: LenientString,
    #[serde(default)]
    source_repo: LenientString,
    #[serde(default)]
    source_release_tag: LenientString,
}

/// One `installed_games[]` row (`config.py:106-190`). `cover_url`,
/// `cached_cover_path` and `local_path` are deliberately absent: covers
/// refetch from the server and `local_path` has no Rust counterpart.
#[derive(Debug, Default, Deserialize)]
struct PythonGame {
    #[serde(default)]
    title: LenientString,
    #[serde(default)]
    platform: LenientString,
    #[serde(default)]
    rom_id: LenientString,
    #[serde(default)]
    ra_id: LenientString,
    #[serde(default)]
    server_updated_at: LenientString,
    #[serde(default)]
    rom_file_name: LenientString,
    #[serde(default)]
    archive_path: LenientString,
    #[serde(default)]
    extracted_path: LenientString,
    #[serde(default)]
    extracted_dir: LenientString,
    #[serde(default)]
    multi_file_game_dir: LenientString,
    #[serde(default)]
    description: LenientString,
    #[serde(default)]
    rating: LenientString,
    #[serde(default)]
    genres: LenientString,
    #[serde(default)]
    regions: LenientString,
    #[serde(default)]
    filesize_bytes: LenientString,
    #[serde(default)]
    screenshot_urls: LenientString,
    #[serde(default)]
    native_executable_path: LenientString,
    #[serde(default)]
    native_launch_parameters: LenientString,
    #[serde(default)]
    native_compat_tool: LenientString,
    #[serde(default)]
    native_wineprefix: LenientString,
    #[serde(default)]
    native_game_dir: LenientString,
    #[serde(default)]
    included_dlc: LenientString,
    #[serde(default)]
    ps3_trophy_paths: LenientString,
    #[serde(default)]
    ps3_game_id: LenientString,
    #[serde(default)]
    ps3_iso_path: LenientString,
    #[serde(default)]
    ps4_game_id: LenientString,
    #[serde(default)]
    ps4_content: LenientString,
}

/// One `compat_tool_installs` value (`config.py:193-212`).
/// `compat_tool_type` is read and then dropped: the Rust
/// [`CompatToolInstall`] has no such field.
#[derive(Debug, Default, Deserialize)]
struct PythonCompatTool {
    #[serde(default)]
    name: LenientString,
    #[serde(default)]
    install_path: LenientString,
}

// ---------------------------------------------------------------------------
// Normalizers
// ---------------------------------------------------------------------------

/// `normalized_theme_choice` (`ui/theme.py:140-146`): `system`, `dark` or
/// `light`, case-insensitively; anything else collapses to `system`.
fn normalize_theme(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    match lowered.as_str() {
        "system" | "dark" | "light" => lowered,
        _ => "system".to_string(),
    }
}

/// `normalize_default_emulators` (`config.py:81-90`) and
/// `normalize_default_retroarch_cores` (`config.py:92-102`). Keys are
/// trimmed and blank keys dropped; a non-string value drops the pair. When
/// `require_value` is set, a blank value drops the pair too — the cores map
/// requires one, the default-emulators map does not.
fn string_map(raw: &JsonMap, require_value: bool) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(text) = value.as_str() else { continue };
        let text = text.trim();
        if require_value && text.is_empty() {
            continue;
        }
        out.insert(key.to_string(), text.to_string());
    }
    out
}

/// `native_manual_save_paths`: `"<title>__manual"` -> a list of directories.
/// Keys keep their exact shape (the Rust cloud code uses the same key), and
/// blank or non-string entries in a list are dropped.
fn path_list_map(raw: &JsonMap) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(items) = value.as_array() else { continue };
        let paths: Vec<String> = items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect();
        out.insert(key.to_string(), paths);
    }
    out
}

/// A `serde_json` value as a TOML value, or `None` when TOML cannot hold it
/// (`null`, and a number that is neither an `i64` nor an `f64`). Containers
/// keep every convertible child and drop the rest, so one bad leaf costs a
/// leaf rather than the whole `cloud_sync_state` subtree.
fn json_to_toml(value: &serde_json::Value) -> Option<toml::Value> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(flag) => Some(toml::Value::Boolean(*flag)),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map(toml::Value::Integer)
            .or_else(|| number.as_f64().map(toml::Value::Float)),
        serde_json::Value::String(text) => Some(toml::Value::String(text.clone())),
        serde_json::Value::Array(items) => {
            Some(toml::Value::Array(items.iter().filter_map(json_to_toml).collect()))
        }
        serde_json::Value::Object(map) => {
            let mut table = toml::value::Table::new();
            for (key, child) in map {
                if let Some(child) = json_to_toml(child) {
                    table.insert(key.clone(), child);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

/// `cloud_sync_state` as the untyped TOML table `Config` stores.
fn sync_state_table(raw: &JsonMap) -> toml::value::Table {
    let mut table = toml::value::Table::new();
    for (key, value) in raw {
        if let Some(value) = json_to_toml(value) {
            table.insert(key.clone(), value);
        }
    }
    table
}

// ---------------------------------------------------------------------------
// The conversion
// ---------------------------------------------------------------------------

/// The pure conversion: Python `config.json` text in, a Rust [`Config`], the
/// registry rows, and the number of rows dropped for a blank title or
/// platform out.
///
/// `installed_at` is left at `0` on every row — this function has no clock.
/// [`import`] stamps it.
pub fn plan(
    python_json: &str,
) -> Result<(Config, Vec<InstalledGame>, usize), ImportError> {
    let document: serde_json::Value =
        serde_json::from_str(python_json).map_err(|_| ImportError::Malformed)?;
    if !document.is_object() {
        return Err(ImportError::Malformed);
    }
    // Every field is `Lenient`/defaulted, so an object cannot fail here;
    // the mapping keeps the compiler honest rather than the runtime.
    let raw: PythonConfig =
        serde_json::from_value(document).map_err(|_| ImportError::Malformed)?;

    let mut emulators: Vec<EmulatorEntry> = Vec::new();
    for entry in raw.emulators.0 {
        let entry = entry.0;
        let name = entry.name.into_string();
        if name.is_empty() {
            continue;
        }
        let args = entry.args.into_string();
        emulators.push(EmulatorEntry {
            name,
            path: entry.path.into_string(),
            // config.py:36-37, :61 — a missing or all-whitespace value is
            // the placeholder, not an empty argument list.
            args: if args.is_empty() { "%rom%".to_string() } else { args },
            save_strategy: normalize_save_strategy(entry.save_strategy.as_str()),
            ignore_files: entry.ignore_files.into_string(),
            ignore_extensions: entry.ignore_extensions.into_string(),
            save_paths: entry.save_paths.into_string(),
            state_paths: entry.state_paths.into_string(),
            source_id: entry.source_id.into_string(),
            source_provider: entry.source_provider.into_string(),
            source_owner: entry.source_owner.into_string(),
            source_repo: entry.source_repo.into_string(),
            source_release_tag: entry.source_release_tag.into_string(),
            // Nothing on disk vouches for which release is actually
            // installed, so the update check re-resolves it.
            source_installed_tag: String::new(),
        });
    }
    // config.py:79 sorts by the case-folded name.
    emulators.sort_by_key(|entry| entry.name.to_lowercase());

    let mut compat_tool_installs: Vec<CompatToolInstall> = Vec::new();
    for (source_id, value) in &raw.compat_tool_installs.0 {
        let tool: PythonCompatTool =
            serde_json::from_value(value.clone()).unwrap_or_default();
        let name = tool.name.into_string();
        if name.is_empty() {
            continue; // config.py:205-206
        }
        compat_tool_installs.push(CompatToolInstall {
            name,
            path: tool.install_path.into_string(),
            source_id: source_id.trim().to_string(),
            // Python records no release tag for a compat tool.
            release_tag: String::new(),
        });
    }

    let mut games: Vec<InstalledGame> = Vec::new();
    let mut skipped_games = 0usize;
    for row in raw.installed_games.0 {
        let row = row.0;
        let title = row.title.into_string();
        let platform = row.platform.into_string();
        if title.is_empty() || platform.is_empty() {
            skipped_games += 1;
            continue;
        }
        games.push(InstalledGame {
            title,
            platform,
            // A row with no parsable rom id is still imported: the registry
            // keys on title and platform, and `installed_match` accepts a
            // `None` rom id as an identity match.
            rom_id: row.rom_id.as_str().parse::<i64>().ok(),
            rom_file_name: row.rom_file_name.into_string(),
            archive_path: row.archive_path.into_string(),
            extracted_path: row.extracted_path.into_string(),
            extracted_dir: row.extracted_dir.into_string(),
            multi_file_game_dir: row.multi_file_game_dir.into_string(),
            description: row.description.into_string(),
            rating: row.rating.into_string(),
            genres: row.genres.into_string(),
            regions: row.regions.into_string(),
            filesize_bytes: row.filesize_bytes.as_str().parse::<i64>().unwrap_or(0),
            server_updated_at: row.server_updated_at.into_string(),
            screenshot_urls: row.screenshot_urls.into_string(),
            native_executable_path: row.native_executable_path.into_string(),
            native_launch_parameters: row.native_launch_parameters.into_string(),
            native_compat_tool: row.native_compat_tool.into_string(),
            native_wineprefix: row.native_wineprefix.into_string(),
            native_game_dir: row.native_game_dir.into_string(),
            included_dlc: row.included_dlc.into_string(),
            ps3_trophy_paths: row.ps3_trophy_paths.into_string(),
            ps3_game_id: row.ps3_game_id.as_str().to_uppercase(),
            ps3_iso_path: row.ps3_iso_path.into_string(),
            ps4_game_id: row.ps4_game_id.as_str().to_uppercase(),
            ps4_content: row.ps4_content.into_string(),
            ra_id: row.ra_id.into_string(),
            // `..Default::default()` covers, in order: `languages`, `tags`,
            // `revision`, `companies`, `first_release_date` (Python stores
            // none of them), `cover_small_path`/`cover_large_path` and
            // `fanart_urls` (images refetch from the server),
            // `installed_at` (stamped by `import`), `last_played_at`, and
            // `images_version` (stamped by `Registry::upsert`).
            ..Default::default()
        });
    }

    let defaults = Config::default();
    let config = Config {
        server_url: raw.server_url.into_string(),
        username: raw.username.into_string(),
        library_path: raw.library_path.into_string(),
        emulators,
        default_emulators: string_map(&raw.default_emulators.0, false),
        retroarch_cores: string_map(&raw.default_retroarch_cores.0, true),
        launch_args: raw.launch_args.into_string(),
        retroachievements_username: raw.retroachievements_username.into_string(),
        auto_cloud_save_download_on_launch: raw
            .auto_cloud_save_download_on_launch
            .or(defaults.auto_cloud_save_download_on_launch),
        auto_cloud_save_upload_on_exit: raw
            .auto_cloud_save_upload_on_exit
            .or(defaults.auto_cloud_save_upload_on_exit),
        auto_cloud_save_skip_download_if_local_newer: raw
            .auto_cloud_save_skip_download_if_local_newer
            .or(defaults.auto_cloud_save_skip_download_if_local_newer),
        // `max(0, min(value, 60))` (grid-launcher.py:2221).
        auto_cloud_save_upload_delay_seconds: raw
            .auto_cloud_save_upload_delay_seconds
            .or(defaults.auto_cloud_save_upload_delay_seconds as i64)
            .clamp(0, 60) as u64,
        cloud_sync_state: sync_state_table(&raw.cloud_sync_state.0),
        native_manual_save_paths: path_list_map(&raw.native_manual_save_paths.0),
        default_compat_tool: raw.default_compat_tool.into_string(),
        compat_tool_installs,
        debug_prints: raw.debug_prints.or(defaults.debug_prints),
        ui: crate::config::UiSettings {
            theme: normalize_theme(raw.theme.as_str()),
            ..Default::default()
        },
        ..Config::default()
    };

    Ok((config, games, skipped_games))
}
```

Note the two imports that Task 3 needs and this task does not yet use: `Registry` and `Path`. Leave them out for now and add them in Task 3 — an unused import fails `cargo clippy -- -D warnings`. Delete these two lines from the `use` block above for this task:

```rust
use crate::library::registry::{InstalledGame, Registry};
use std::path::Path;
```

and write instead:

```rust
use crate::library::registry::InstalledGame;
```

- [ ] **Step 5: Register the module**

In `rewrite/crates/grid-core/src/lib.rs`, add the declaration in alphabetical order:

```rust
pub mod images;
pub mod import_python;
pub mod launch;
```

- [ ] **Step 6: Run the tests and see them pass**

Run: `cd rewrite && cargo test -p grid-core import_python`
Expected: PASS, 15 tests.

- [ ] **Step 7: Run the gates**

```bash
cd rewrite && cargo test -p grid-core -p app
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo fmt --all --check
cd /home/six/Documents/Programming/grid-launcher && bash rewrite/scripts/check_secret_hygiene.sh
```

Expected: all pass. `cargo fmt --all --check` prints nothing.

- [ ] **Step 8: Commit**

```bash
git commit --only -- rewrite/crates/grid-core/src/import_python.rs rewrite/crates/grid-core/src/lib.rs -m "$(cat <<'MSG'
rewrite: convert a Python config.json into a Config and registry rows

import_python::plan is the pure half of the importer: lenient Python
input types matching the reference's isinstance guards, the theme,
save-strategy, boolean and delay normalizers, and the full mapping to
Config and InstalledGame. The typed input struct has no field for
api_token, retroachievements_api_key or retroachievements_token, so no
token value is ever held; a fixture test asserts none reaches the
serialized config.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

### Task 3: `import_python::import` — file read, registry upsert, config save

**Files:**
- Modify: `rewrite/crates/grid-core/src/import_python.rs` — add `import`, extend the `use` block, add tests.

**Interfaces:**
- Consumes (from Task 2): `plan(&str) -> Result<(Config, Vec<InstalledGame>, usize), ImportError>`, `ImportReport { emulators, games, skipped_games, retroachievements }`, `ImportError::{Unreadable, Malformed, Write}`, and the `PYTHON_CONFIG` test fixture.
- Consumes (from grid-core): `Registry::open(&Path) -> Result<Registry, LibraryError>`, `Registry::upsert(&InstalledGame) -> Result<(), LibraryError>`, `Registry::all() -> Result<Vec<InstalledGame>, LibraryError>` (`library/registry.rs:389-470`); `Config::save(&Path) -> Result<(), ConfigError>` and `Config::load(&Path)` (`config.rs:311-340`).
- Produces, for Task 4:

  ```rust
  pub fn import(
      python_json: &Path,
      config_path: &Path,
      registry: &Registry,
      now: i64,
  ) -> Result<ImportReport, ImportError>;
  ```

- [ ] **Step 1 (test first): Write the failing tests**

Append to `mod tests` in `rewrite/crates/grid-core/src/import_python.rs`:

```rust
    use std::path::PathBuf;

    /// A tempdir holding a Python config, a Rust config path and an open
    /// registry — the three things `import` touches.
    struct Scratch {
        _dir: tempfile::TempDir,
        python: PathBuf,
        config: PathBuf,
        registry: Registry,
    }

    fn scratch(python_json: &str) -> Scratch {
        let dir = tempfile::tempdir().expect("a tempdir");
        let python = dir.path().join("config.json");
        std::fs::write(&python, python_json).expect("the fixture writes");
        let registry = Registry::open(&dir.path().join("grid-launcher.db")).expect("a registry");
        Scratch {
            python,
            config: dir.path().join("config.toml"),
            registry,
            _dir: dir,
        }
    }

    #[test]
    fn import_writes_the_config_and_the_rows() {
        let s = scratch(PYTHON_CONFIG);
        let report = import(&s.python, &s.config, &s.registry, 1_757_500_000).unwrap();

        assert_eq!(report.emulators, 2);
        assert_eq!(report.games, 3);
        assert_eq!(report.skipped_games, 2);
        assert!(report.retroachievements);

        let saved = Config::load(&s.config).expect("the config loads back");
        assert_eq!(saved.server_url, "https://romm.example.test");
        assert_eq!(saved.emulators.len(), 2);
        assert_eq!(saved.ui.theme, "dark");

        let mut rows = s.registry.all().expect("the registry reads back");
        rows.sort_by(|a, b| a.title.cmp(&b.title));
        let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, vec!["Chrono Trigger", "Portal 2", "Ratchet & Clank"]);
        // `import` supplies the clock `plan` does not have.
        assert!(rows.iter().all(|r| r.installed_at == 1_757_500_000));
        assert!(rows.iter().all(|r| r.last_played_at == 0));
    }

    #[test]
    fn no_token_reaches_the_written_config_file() {
        let s = scratch(PYTHON_CONFIG);
        import(&s.python, &s.config, &s.registry, 1).unwrap();
        let text = std::fs::read_to_string(&s.config).expect("the config file reads");
        for needle in [
            "SECRET-ROMM-TOKEN-DO-NOT-IMPORT",
            "SECRET-RA-KEY-DO-NOT-IMPORT",
            "SECRET-RA-TOKEN-DO-NOT-IMPORT",
            "api_token",
            "retroachievements_api_key",
            "retroachievements_token",
        ] {
            assert!(!text.contains(needle), "{needle} leaked into config.toml");
        }
    }

    #[test]
    fn malformed_json_writes_nothing() {
        let s = scratch("{ this is not json");
        let error = import(&s.python, &s.config, &s.registry, 1).unwrap_err();
        assert!(matches!(error, ImportError::Malformed));
        assert!(!s.config.exists());
        assert!(s.registry.all().unwrap().is_empty());
    }

    #[test]
    fn an_absent_python_file_is_unreadable() {
        let s = scratch("{}");
        std::fs::remove_file(&s.python).unwrap();
        let error = import(&s.python, &s.config, &s.registry, 1).unwrap_err();
        assert!(matches!(error, ImportError::Unreadable));
        assert!(!s.config.exists());
    }

    #[test]
    fn an_empty_python_config_still_writes_a_rust_config() {
        // The caller's presence check keys on the Rust file existing, so a
        // config with nothing worth importing must still leave one behind
        // or the import would run again on every start.
        let s = scratch("{}");
        let report = import(&s.python, &s.config, &s.registry, 1).unwrap();
        assert_eq!(report, ImportReport::default());
        assert!(s.config.exists());
    }

    #[test]
    fn error_text_names_no_path_and_no_file_content() {
        let s = scratch("{ this is not json");
        let error = import(&s.python, &s.config, &s.registry, 1).unwrap_err();
        let text = error.to_string();
        assert!(!text.contains("config.json"));
        assert!(!text.contains("this is not json"));
    }

    /// `GRID_LAUNCHER_DATA_DIR` moves the Rust side only: the Python path is
    /// `~/.grid-launcher/config.json` on every platform and the override
    /// never touches it. Here that is `Config::default_path()` following the
    /// override while `import`'s `python_json` argument does not.
    #[test]
    fn the_data_dir_override_moves_only_the_rust_config() {
        let _lock = crate::test_env::lock();
        let dir = tempfile::tempdir().expect("a tempdir");
        let _guard = crate::test_env::EnvGuard::set(&[(
            "GRID_LAUNCHER_DATA_DIR",
            Some(dir.path().to_str().unwrap()),
        )]);
        assert_eq!(Config::default_path(), dir.path().join("config.toml"));

        let python = dir.path().join("python-home").join("config.json");
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(&python, PYTHON_CONFIG).unwrap();
        let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
        import(&python, &Config::default_path(), &registry, 7).unwrap();

        assert!(dir.path().join("config.toml").exists());
    }
```

- [ ] **Step 2: Run the tests and see them fail**

Run: `cd rewrite && cargo test -p grid-core import_python`
Expected: FAIL with `cannot find function 'import' in this scope` and `cannot find type 'Registry' in this scope`.

- [ ] **Step 3: Write `import` and extend the imports**

In `rewrite/crates/grid-core/src/import_python.rs`, change the `use` block so it reads:

```rust
use crate::autoconfig::entry::normalize_save_strategy;
use crate::config::{CompatToolInstall, Config, EmulatorEntry};
use crate::library::registry::{InstalledGame, Registry};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::path::Path;
```

Then append, after `plan`:

```rust
/// Reads the Python config at `python_json`, converts it with [`plan`],
/// writes the registry rows and saves the Rust config at `config_path`.
///
/// `now` is stamped onto every row's `installed_at`: the Python config
/// records no install time, and a row with `installed_at == 0` would sort
/// to the bottom of every recency view forever.
///
/// The config is saved LAST. The caller's "no Rust config yet" check is what
/// makes this a one-shot, so the file that ends the import must not exist
/// before the rows it describes do.
///
/// A [`ImportError::Write`] leaves the caller free to start anyway: whether
/// or not `config.toml` landed, the next start is consistent — either the
/// import is done, or it is retried from an unchanged Python file.
pub fn import(
    python_json: &Path,
    config_path: &Path,
    registry: &Registry,
    now: i64,
) -> Result<ImportReport, ImportError> {
    let text = std::fs::read_to_string(python_json).map_err(|_| ImportError::Unreadable)?;
    let (config, games, skipped_games) = plan(&text)?;

    let report = ImportReport {
        emulators: config.emulators.len(),
        games: games.len(),
        skipped_games,
        retroachievements: !config.retroachievements_username.is_empty(),
    };

    for game in &games {
        let mut row = game.clone();
        row.installed_at = now;
        registry
            .upsert(&row)
            .map_err(|e| ImportError::Write(e.to_string()))?;
    }
    config
        .save(config_path)
        .map_err(|e| ImportError::Write(e.to_string()))?;

    Ok(report)
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cd rewrite && cargo test -p grid-core import_python`
Expected: PASS, 22 tests.

- [ ] **Step 5: Run the gates**

```bash
cd rewrite && cargo test -p grid-core -p app
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo fmt --all --check
cd /home/six/Documents/Programming/grid-launcher && bash rewrite/scripts/check_secret_hygiene.sh
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git commit --only -- rewrite/crates/grid-core/src/import_python.rs -m "$(cat <<'MSG'
rewrite: write the imported config and registry rows

import_python::import reads the Python config.json, runs plan, stamps
installed_at, upserts every row and saves config.toml last, so the
caller's one-shot presence check can never see a config that describes
rows which are not there. Errors name no path and carry no file content.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

### Task 4: App wiring — startup check, state, command, toast

**Files:**
- Create: `rewrite/app/src-tauri/src/python_import.rs`
- Modify: `rewrite/app/src-tauri/src/lib.rs` — `mod python_import;`, the startup call, and the command registration.
- Modify: `rewrite/app/src-tauri/src/commands.rs:44-71` — add the `python_import` field to `AppState`.
- Modify: `rewrite/app/src-tauri/src/commands/updates.rs` — add the `python_import_notice` command.
- Create: `rewrite/app/src/lib/pythonImport.ts`
- Create: `rewrite/app/src/lib/pythonImport.test.ts`
- Create: `rewrite/app/src/lib/stores/pythonImport.svelte.ts`
- Create: `rewrite/app/src/lib/stores/pythonImport.test.ts`
- Modify: `rewrite/app/src/lib/api.ts` — the `PythonImportReport` type and the `pythonImportNotice` binding.
- Modify: `rewrite/app/src/App.svelte` — one more mount effect.

**Interfaces:**
- Consumes (from Task 3): `grid_core::import_python::{import, ImportReport, ImportError}`; `ImportReport` fields `emulators`, `games`, `skipped_games`, `retroachievements`, all `Serialize`.
- Consumes (from grid-core): `grid_core::autoconfig::paths::home_dir() -> Option<PathBuf>` (`crates/grid-core/src/autoconfig/paths.rs:19-26`); `grid_core::config::Config::default_path()`; `grid_core::library::registry::Registry`.
- Produces:

  ```rust
  // app/src-tauri/src/python_import.rs
  pub fn python_config_path() -> Option<PathBuf>;
  pub fn should_import(rust_config: &Path, python_config: &Path) -> bool;
  pub fn startup_import(rust_config: &Path, python_config: &Path, registry: &Registry, now: i64)
      -> Option<ImportReport>;
  ```

  ```ts
  // app/src/lib/pythonImport.ts
  export function importToastText(report: PythonImportReport): string;
  ```

- [ ] **Step 1 (test first, Rust): Write `python_import.rs` with failing tests only**

Create `rewrite/app/src-tauri/src/python_import.rs` containing just the test module, so the tests fail on missing functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::config::Config;

    const MINIMAL: &str = r#"{
      "server_url": "https://romm.example.test",
      "api_token": "SECRET-DO-NOT-IMPORT",
      "retroachievements_username": "ashley",
      "emulators": [{ "name": "RetroArch", "path": "/opt/retroarch" }],
      "installed_games": [{ "title": "Chrono Trigger", "platform": "SNES", "rom_id": "4321" }]
    }"#;

    struct Scratch {
        _dir: tempfile::TempDir,
        rust_config: std::path::PathBuf,
        python_config: std::path::PathBuf,
        registry: Registry,
    }

    fn scratch() -> Scratch {
        let dir = tempfile::tempdir().expect("a tempdir");
        let python_config = dir.path().join("python-config.json");
        std::fs::write(&python_config, MINIMAL).expect("the fixture writes");
        let registry = Registry::open(&dir.path().join("grid-launcher.db")).expect("a registry");
        Scratch {
            rust_config: dir.path().join("config.toml"),
            python_config,
            registry,
            _dir: dir,
        }
    }

    #[test]
    fn a_python_file_and_no_rust_config_means_import() {
        let s = scratch();
        assert!(should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn an_existing_rust_config_means_no_import() {
        let s = scratch();
        Config::default().save(&s.rust_config).unwrap();
        assert!(!should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn no_python_file_means_no_import() {
        let s = scratch();
        std::fs::remove_file(&s.python_config).unwrap();
        assert!(!should_import(&s.rust_config, &s.python_config));
    }

    #[test]
    fn startup_import_reports_counts_and_leaves_a_rust_config() {
        let s = scratch();
        let report = startup_import(&s.rust_config, &s.python_config, &s.registry, 42)
            .expect("a fresh profile with a Python config imports");
        assert_eq!(report.emulators, 1);
        assert_eq!(report.games, 1);
        assert_eq!(report.skipped_games, 0);
        assert!(report.retroachievements);
        assert!(s.rust_config.exists());
        assert_eq!(s.registry.all().unwrap().len(), 1);
    }

    #[test]
    fn startup_import_is_a_no_op_once_a_rust_config_exists() {
        let s = scratch();
        Config::default().save(&s.rust_config).unwrap();
        assert!(startup_import(&s.rust_config, &s.python_config, &s.registry, 42).is_none());
        assert!(s.registry.all().unwrap().is_empty());
    }

    #[test]
    fn a_broken_python_config_yields_no_notice_and_no_rust_config() {
        let s = scratch();
        std::fs::write(&s.python_config, "{ not json").unwrap();
        assert!(startup_import(&s.rust_config, &s.python_config, &s.registry, 42).is_none());
        assert!(!s.rust_config.exists());
    }

    #[test]
    fn the_python_path_is_the_fixed_dot_directory() {
        let path = python_config_path().expect("a home directory");
        assert!(path.ends_with(".grid-launcher/config.json"));
    }
}
```

`tempfile` must be a dev-dependency of the `app` crate. Check `rewrite/app/src-tauri/Cargo.toml` for a `[dev-dependencies]` `tempfile` entry; if it is absent, add `tempfile = "3.27.0"` there (the version grid-core already pins).

- [ ] **Step 2: Run the tests and see them fail**

Run: `cd rewrite && cargo test -p app python_import`
Expected: FAIL — the module is not declared (`cargo test` will not even see it). Add `mod python_import;` to `rewrite/app/src-tauri/src/lib.rs` alongside the other `mod` lines (alphabetical: after `mod media_server;`), re-run, and expect `cannot find function 'should_import' in this scope`.

- [ ] **Step 3: Write the module body**

Insert above the test module in `rewrite/app/src-tauri/src/python_import.rs`:

```rust
//! The one-shot import of a Python-era configuration, run at startup.
//!
//! The trigger is file presence, not a stored flag: no Rust `config.toml`
//! and a Python `~/.grid-launcher/config.json`. A successful import always
//! writes `config.toml`, so this runs at most once per profile even when
//! some rows were skipped.
//!
//! Every log line here carries counts only — never a path, a key or a value
//! from the file being read.

use grid_core::import_python::{self, ImportReport};
use grid_core::library::registry::Registry;
use std::path::{Path, PathBuf};

/// `~/.grid-launcher/config.json`, the reference's persistence root on every
/// platform (doc 02, "Persistence root"; grid-launcher.py:2386-2393).
///
/// `GRID_LAUNCHER_DATA_DIR` deliberately does NOT apply: it redirects the
/// Rust side's own state, and the Python app never read it.
pub fn python_config_path() -> Option<PathBuf> {
    grid_core::autoconfig::paths::home_dir()
        .map(|home| home.join(".grid-launcher").join("config.json"))
}

/// Whether the importer should run. Pure, so the startup decision is
/// testable without a Tauri app.
pub fn should_import(rust_config: &Path, python_config: &Path) -> bool {
    !rust_config.exists() && python_config.exists()
}

/// Runs the import when [`should_import`] says to, and returns the report
/// for `AppState.python_import`.
///
/// Returns `None` for every non-event — nothing to import, or an import that
/// failed. A failure is logged once and the app starts normally: a fresh
/// profile is a working profile.
pub fn startup_import(
    rust_config: &Path,
    python_config: &Path,
    registry: &Registry,
    now: i64,
) -> Option<ImportReport> {
    if !should_import(rust_config, python_config) {
        return None;
    }
    match import_python::import(python_config, rust_config, registry, now) {
        Ok(report) => {
            tracing::info!(
                "imported the previous version's settings: {} emulators, {} games, {} rows skipped",
                report.emulators,
                report.games,
                report.skipped_games
            );
            Some(report)
        }
        Err(e) => {
            // `ImportError`'s Display carries no path and no file content.
            tracing::warn!("the previous version's settings were not imported: {e}");
            None
        }
    }
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cd rewrite && cargo test -p app python_import`
Expected: PASS, 7 tests.

- [ ] **Step 5: Add the `AppState` field**

In `rewrite/app/src-tauri/src/commands.rs`, add to the `AppState` struct (after `media_server`), and add the import at the top of the file:

```rust
use grid_core::import_python::ImportReport;
```

```rust
    /// What the one-shot Python-config import did at startup, or `None`
    /// when there was nothing to import. Counts only — never a value out of
    /// the imported file. Pulled once by the frontend through
    /// `commands::updates::python_import_notice`.
    pub python_import: Option<ImportReport>,
```

- [ ] **Step 6: Add the command**

In `rewrite/app/src-tauri/src/commands/updates.rs`, next to `app_update_notice`:

```rust
/// The startup import's report, or `null` when nothing was imported. The
/// frontend pulls this once on mount, the same late-mount pattern as
/// `app_update_notice` — there is no event, because the import is finished
/// before the webview exists.
#[tauri::command]
pub fn python_import_notice(state: State<'_, AppState>) -> Option<ImportReport> {
    state.python_import
}
```

Add `use grid_core::import_python::ImportReport;` to that file's imports.

- [ ] **Step 7: Wire startup and register the command**

In `rewrite/app/src-tauri/src/lib.rs`, immediately after the `let registry = Registry::open(&db_path)…` binding and before `let install = …`, insert:

```rust
    // Carry a Python user's settings and library across on the first start
    // (spec 2026-09-10, Part 3). Before any service reads the config, and
    // after the registry is open because the import writes rows into it.
    // Counts only reach the log; no token is ever read.
    let python_import = match (&registry, python_import::python_config_path()) {
        (Ok(registry), Some(python_config)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            python_import::startup_import(&config_path, &python_config, registry, now)
        }
        _ => None,
    };
```

`config_path` is already bound above (`let config_path = Config::default_path();`). Add `python_import,` to the `AppState { … }` literal, and add the command to the `invoke_handler` list beside `commands::updates::app_update_notice`:

```rust
            commands::updates::python_import_notice,
```

- [ ] **Step 8: Run the Rust gate**

```bash
cd rewrite && cargo test -p grid-core -p app
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo fmt --all --check
cd /home/six/Documents/Programming/grid-launcher && bash rewrite/scripts/check_secret_hygiene.sh
```

Expected: all pass.

- [ ] **Step 9 (test first, frontend): Write the toast-text test**

Create `rewrite/app/src/lib/pythonImport.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { importToastText } from './pythonImport';

describe('importToastText', () => {
  it('reports the counts and asks for the RomM token', () => {
    expect(
      importToastText({ emulators: 2, games: 14, skipped_games: 0, retroachievements: false }),
    ).toBe('Imported 2 emulators and 14 games from the previous version. Enter your RomM token to reconnect.');
  });

  it('appends the RetroAchievements sentence only when a username came across', () => {
    expect(
      importToastText({ emulators: 1, games: 1, skipped_games: 0, retroachievements: true }),
    ).toBe(
      'Imported 1 emulator and 1 game from the previous version. Enter your RomM token to reconnect. Enter your RetroAchievements token as well.',
    );
  });

  it('says none rather than 0', () => {
    expect(
      importToastText({ emulators: 0, games: 0, skipped_games: 3, retroachievements: false }),
    ).toBe('Imported no emulators and no games from the previous version. Enter your RomM token to reconnect.');
  });
});
```

- [ ] **Step 10: Run it and see it fail**

Run: `cd rewrite/app && npx vitest run src/lib/pythonImport.test.ts`
Expected: FAIL — `Failed to resolve import "./pythonImport"`.

- [ ] **Step 11: Write the helper and the API binding**

Create `rewrite/app/src/lib/pythonImport.ts`:

```ts
// The one line the user sees after their Python settings are carried over.
// Pure, so the wording is pinned by vitest rather than by reading a toast.
import type { PythonImportReport } from './api';

function count(n: number, singular: string, plural: string): string {
  if (n === 0) return `no ${plural}`;
  return `${n} ${n === 1 ? singular : plural}`;
}

/**
 * The import toast. `skipped_games` is deliberately not shown: it is a
 * diagnostic for the log, and a user who never saw those rows in the old app
 * would not recognise the number.
 */
export function importToastText(report: PythonImportReport): string {
  const sentences = [
    `Imported ${count(report.emulators, 'emulator', 'emulators')} and ` +
      `${count(report.games, 'game', 'games')} from the previous version.`,
    'Enter your RomM token to reconnect.',
  ];
  if (report.retroachievements) sentences.push('Enter your RetroAchievements token as well.');
  return sentences.join(' ');
}
```

In `rewrite/app/src/lib/api.ts`, add the type next to the `AppUpdateStatus` block:

```ts
/** `python_import_notice`'s payload: what the one-shot Python-config import
 *  did at startup, or `null` when nothing was imported. Counts only. */
export type PythonImportReport = {
  emulators: number;
  games: number;
  skipped_games: number;
  retroachievements: boolean;
};
```

and the binding inside the `api` object, next to `appUpdateNotice`:

```ts
  pythonImportNotice: () => invoke<PythonImportReport | null>('python_import_notice'),
```

- [ ] **Step 12: Run the test and see it pass**

Run: `cd rewrite/app && npx vitest run src/lib/pythonImport.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 13 (test first): Write the store test**

Create `rewrite/app/src/lib/stores/pythonImport.test.ts`, following `stores/appUpdate.test.ts`'s module-mock pattern:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

describe('initPythonImport', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('./toasts.svelte');
  });

  it('shows one toast for a report and never a second one', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: {
        pythonImportNotice: () =>
          Promise.resolve({ emulators: 2, games: 14, skipped_games: 1, retroachievements: false }),
      },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await initPythonImport();
    await initPythonImport();

    expect(shown).toEqual([
      'Imported 2 emulators and 14 games from the previous version. Enter your RomM token to reconnect.',
    ]);
  });

  it('shows nothing when there was no import', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({ api: { pythonImportNotice: () => Promise.resolve(null) } }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await initPythonImport();

    expect(shown).toEqual([]);
  });

  it('swallows a failed pull', async () => {
    const shown: string[] = [];
    vi.doMock('../api', () => ({
      api: { pythonImportNotice: () => Promise.reject(new Error('no backend')) },
    }));
    vi.doMock('./toasts.svelte', () => ({ pushToast: (text: string) => shown.push(text) }));

    const { initPythonImport } = await import('./pythonImport.svelte');
    await expect(initPythonImport()).resolves.toBeUndefined();
    expect(shown).toEqual([]);
  });
});
```

- [ ] **Step 14: Run it and see it fail**

Run: `cd rewrite/app && npx vitest run src/lib/stores/pythonImport.test.ts`
Expected: FAIL — `Failed to resolve import "./pythonImport.svelte"`.

- [ ] **Step 15: Write the store**

Create `rewrite/app/src/lib/stores/pythonImport.svelte.ts`:

```ts
// The startup Python-config import's one toast. Module-scoped so the toast
// survives a Shell remount without repeating: the import itself already
// happened once, before the webview existed.
import { api } from '../api';
import { importToastText } from '../pythonImport';
import { pushToast } from './toasts.svelte';

let shown = false;

/**
 * Pulls `python_import_notice` once and, when there was an import, shows one
 * toast. There is no event to listen for — the import finishes inside the
 * Rust `run()` before the window is created — so this is a pull only, unlike
 * `initAppUpdate`.
 *
 * Returns nothing to unsubscribe from; `App.svelte` calls it from an effect
 * like the other startup stores and ignores the result.
 */
export async function initPythonImport(): Promise<void> {
  if (shown) return;
  try {
    const report = await api.pythonImportNotice();
    if (report === null) return;
    shown = true;
    pushToast(importToastText(report));
  } catch {
    // A failed read is never surfaced: the import either happened or did
    // not, and the user finds their library either way.
  }
}
```

- [ ] **Step 16: Run the test and see it pass**

Run: `cd rewrite/app && npx vitest run src/lib/stores/pythonImport.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 17: Mount the store**

In `rewrite/app/src/App.svelte`, add the import beside `initAppUpdate`:

```ts
  import { initPythonImport } from './lib/stores/pythonImport.svelte';
```

and one effect, directly after the `initAppUpdate` effect:

```svelte
  // The import ran before the window existed, so its notice is a pull with
  // nothing to unsubscribe from — but it belongs with the other pre-shell
  // effects so the toast appears at the first paint, not after a connect.
  $effect(() => {
    initPythonImport();
  });
```

- [ ] **Step 18: Run the frontend gate**

```bash
cd rewrite/app && npx vitest run && npx svelte-check
```

Expected: vitest all green; `npx svelte-check` reports the 3 baseline warnings and no more.

- [ ] **Step 19: Run the Rust gate again**

```bash
cd rewrite && cargo test -p grid-core -p app
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p app --all-targets --features e2e -- -D warnings
cargo fmt --all --check
```

Expected: all pass.

- [ ] **Step 20: Commit**

```bash
git commit --only -- \
  rewrite/app/src-tauri/src/python_import.rs \
  rewrite/app/src-tauri/src/lib.rs \
  rewrite/app/src-tauri/src/commands.rs \
  rewrite/app/src-tauri/src/commands/updates.rs \
  rewrite/app/src-tauri/Cargo.toml \
  rewrite/app/src/lib/api.ts \
  rewrite/app/src/lib/pythonImport.ts \
  rewrite/app/src/lib/pythonImport.test.ts \
  rewrite/app/src/lib/stores/pythonImport.svelte.ts \
  rewrite/app/src/lib/stores/pythonImport.test.ts \
  rewrite/app/src/App.svelte \
  -m "$(cat <<'MSG'
rewrite: import the previous version's settings on the first start

Startup runs the Python-config import once per profile, keyed on there
being no config.toml and a ~/.grid-launcher/config.json. The report
lands in AppState, python_import_notice hands it to the frontend, and
App.svelte shows one toast telling the user which tokens to re-enter.
Counts only cross the IPC boundary and reach the log.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

### Task 5: One build workflow, three Python workflows deleted

**Files:**
- Create: `.github/workflows/build.yml` (a rename of `rust-rewrite.yml` — use `git mv` so the history follows).
- Delete: `.github/workflows/rust-rewrite.yml`, `.github/workflows/appimage-linux.yml`, `.github/workflows/pyinstaller-linux.yml`, `.github/workflows/pyinstaller-windows.yml`.

**Interfaces:**
- Consumes (from Task 1): `productName` is exactly `GRID Launcher`, so Tauri's bundler writes `GRID Launcher_<VERSION>_amd64.AppImage` and `GRID Launcher_<VERSION>_x64-setup.exe`. **Task 1 must land before this task's build jobs can be trusted.**
- Produces: release assets `grid-launcher-<VERSION>-x86_64.AppImage`, `grid-launcher-<VERSION>-x86_64.AppImage.zsync` and `grid-launcher-<VERSION>-windows-x86_64-setup.exe` — the names Task 6 documents.

**Triggers (the spec's table, as implemented by the job-level `if:` conditions below):**

| Job | `push` to `main` | `pull_request` | `release: created` | `workflow_dispatch` |
| --- | --- | --- | --- | --- |
| `check` | yes | yes | no | yes |
| `check-windows` | yes | yes | no | yes |
| `e2e` | yes | no | no | yes |
| `build-linux` | no | no | yes | yes |
| `build-windows` | no | no | yes | yes |

`build.sh` and `appimage/` stay in the tree as the Python reference. Nothing runs them any more; do not delete them.

- [ ] **Step 1: Rename the workflow**

```bash
cd /home/six/Documents/Programming/grid-launcher
git mv .github/workflows/rust-rewrite.yml .github/workflows/build.yml
```

- [ ] **Step 2: Write the complete `build.yml`**

Replace the whole file with:

```yaml
name: Build

# One pipeline for the Rust app: gates on every push and pull request, the
# end-to-end suite on pushes to main, and the release artifacts when a
# release is created. A workflow_dispatch run does everything as a dry run —
# it builds version 0.0.0-dev, uploads to the run, and attaches nothing.
on:
  push:
    branches: [main]
  pull_request:
  release:
    types: [created]
  workflow_dispatch:

jobs:
  check:
    name: Check (Linux)
    if: github.event_name != 'release'
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: rewrite
    steps:
      - uses: actions/checkout@v4
      - name: System dependencies (Tauri on Linux)
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Secret hygiene
        run: scripts/check_secret_hygiene.sh
      - name: Format
        run: cargo fmt --check
      - name: Frontend install and build
        run: cd app && npm ci && npx svelte-check && npm run build
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
      - name: Tests
        run: cargo test --workspace
      - name: Frontend tests
        run: cd app && npm test

  check-windows:
    name: Check (Windows)
    # Compiles the Windows code paths. No tests: this job exists so a
    # Windows-only compile error is caught on the pull request that
    # introduces it rather than on the release tag.
    if: github.event_name != 'release'
    runs-on: windows-latest
    defaults:
      run:
        shell: bash
        working-directory: rewrite
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Frontend install and build
        run: cd app && npm ci && npm run build
      - name: Cargo check
        run: cargo check --workspace --all-targets

  e2e:
    name: End-to-end
    if: github.event_name == 'push' || github.event_name == 'workflow_dispatch'
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: rewrite
    steps:
      - uses: actions/checkout@v4
      - name: System dependencies (Tauri on Linux + E2E harness)
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev xvfb gnome-keyring dbus-x11 at-spi2-core sqlite3
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: |
            rewrite/app/package-lock.json
            rewrite/e2e/package-lock.json
      - name: End-to-end tests
        run: scripts/e2e.sh

  build-linux:
    name: AppImage
    if: github.event_name == 'release' || github.event_name == 'workflow_dispatch'
    runs-on: ubuntu-22.04
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Version
        id: version
        shell: bash
        run: |
          set -euo pipefail
          if [ "${GITHUB_EVENT_NAME}" = "release" ]; then
            VERSION="${GITHUB_REF_NAME#v}"
            if [ "$VERSION" = "${GITHUB_REF_NAME}" ]; then
              echo "release tag must start with v (got ${GITHUB_REF_NAME})" >&2
              exit 1
            fi
            if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
              echo "version '$VERSION' is not semver" >&2
              exit 1
            fi
          else
            # A dispatch build is a dry run. A version containing "dev" also
            # suppresses the app's own update check (app_update::is_dev_build).
            VERSION=0.0.0-dev
          fi
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"
          echo "Building version: $VERSION"

      - name: System dependencies (Tauri on Linux + appimagetool)
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev zsync

      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: rewrite/app/package-lock.json

      - name: Install frontend dependencies
        working-directory: rewrite/app
        run: npm ci

      - name: Build the AppImage
        working-directory: rewrite/app
        env:
          VERSION: ${{ steps.version.outputs.version }}
        run: npx tauri build --bundles appimage --config "{\"version\":\"$VERSION\"}"

      - name: Repack with update information
        working-directory: rewrite/app/src-tauri/target/release/bundle/appimage
        env:
          VERSION: ${{ steps.version.outputs.version }}
        run: |
          set -euo pipefail
          SRC="GRID Launcher_${VERSION}_amd64.AppImage"
          ls -l
          chmod +x "$SRC"
          # Tauri's bundler embeds no update information, so unpack its
          # AppDir and let appimagetool write the ELF section AppImageUpdate
          # reads. The listing below makes a missing AppRun/.desktop/icon
          # diagnosable straight from the log.
          "./$SRC" --appimage-extract > /dev/null
          ls -l squashfs-root
          wget -q -O appimagetool \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
          chmod +x appimagetool
          mkdir -p "$GITHUB_WORKSPACE/dist"
          OUTPUT_NAME="grid-launcher-${VERSION}-x86_64.AppImage"
          UPDATE_INFO="gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync"
          ./appimagetool --appimage-extract-and-run -u "$UPDATE_INFO" \
            squashfs-root "$GITHUB_WORKSPACE/dist/$OUTPUT_NAME"
          # appimagetool may write the .zsync into the working directory;
          # the upload steps expect it in dist/ next to the AppImage.
          if [ -f "$OUTPUT_NAME.zsync" ]; then mv "$OUTPUT_NAME.zsync" "$GITHUB_WORKSPACE/dist/"; fi
          ls -l "$GITHUB_WORKSPACE/dist"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: grid-launcher-linux-appimage
          path: |
            dist/grid-launcher-*-x86_64.AppImage
            dist/grid-launcher-*-x86_64.AppImage.zsync

      - name: Attach to release
        if: github.event_name == 'release'
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/grid-launcher-*-x86_64.AppImage
            dist/grid-launcher-*-x86_64.AppImage.zsync

  build-windows:
    name: Windows installer
    if: github.event_name == 'release' || github.event_name == 'workflow_dispatch'
    runs-on: windows-latest
    permissions:
      contents: write
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v4

      - name: Version
        id: version
        run: |
          set -euo pipefail
          if [ "${GITHUB_EVENT_NAME}" = "release" ]; then
            VERSION="${GITHUB_REF_NAME#v}"
            if [ "$VERSION" = "${GITHUB_REF_NAME}" ]; then
              echo "release tag must start with v (got ${GITHUB_REF_NAME})" >&2
              exit 1
            fi
            if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
              echo "version '$VERSION' is not semver" >&2
              exit 1
            fi
          else
            VERSION=0.0.0-dev
          fi
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"
          echo "Building version: $VERSION"

      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: rewrite/app/package-lock.json

      - name: Install frontend dependencies
        working-directory: rewrite/app
        run: npm ci

      - name: Build the NSIS installer
        working-directory: rewrite/app
        env:
          VERSION: ${{ steps.version.outputs.version }}
        run: npx tauri build --bundles nsis --config "{\"version\":\"$VERSION\"}"

      - name: Rename the installer
        env:
          VERSION: ${{ steps.version.outputs.version }}
        run: |
          set -euo pipefail
          mkdir -p dist
          mv "rewrite/app/src-tauri/target/release/bundle/nsis/GRID Launcher_${VERSION}_x64-setup.exe" \
             "dist/grid-launcher-${VERSION}-windows-x86_64-setup.exe"
          ls -l dist

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: grid-launcher-windows-setup
          path: dist/grid-launcher-*-windows-x86_64-setup.exe

      - name: Attach to release
        if: github.event_name == 'release'
        uses: softprops/action-gh-release@v2
        with:
          files: dist/grid-launcher-*-windows-x86_64-setup.exe
```

- [ ] **Step 3: Delete the three Python workflows**

```bash
cd /home/six/Documents/Programming/grid-launcher
git rm .github/workflows/appimage-linux.yml \
       .github/workflows/pyinstaller-linux.yml \
       .github/workflows/pyinstaller-windows.yml
```

`git rm` stages the deletions; it is not a checkout/restore/reset and is allowed.

- [ ] **Step 4: Verify the YAML parses and the tree holds one workflow**

Run, from the repo root:

```bash
if command -v actionlint > /dev/null; then
  actionlint .github/workflows/build.yml
else
  python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1])); print("build.yml parses")' .github/workflows/build.yml
fi
ls .github/workflows
```

Expected: `actionlint` prints nothing (or the fallback prints `build.yml parses`), and `ls` shows exactly `build.yml`.

- [ ] **Step 5: Verify the job names, triggers and asset names**

Run, from the repo root:

```bash
python3 - <<'PY'
import yaml
doc = yaml.safe_load(open('.github/workflows/build.yml'))
assert doc['name'] == 'Build', doc['name']
# PyYAML parses the bare key `on` as the boolean True.
triggers = doc[True]
assert set(triggers) == {'push', 'pull_request', 'release', 'workflow_dispatch'}, triggers
assert triggers['push']['branches'] == ['main']
assert triggers['release']['types'] == ['created']
jobs = doc['jobs']
assert set(jobs) == {'check', 'check-windows', 'e2e', 'build-linux', 'build-windows'}, set(jobs)
assert jobs['check']['if'] == "github.event_name != 'release'"
assert jobs['check-windows']['runs-on'] == 'windows-latest'
assert jobs['build-linux']['runs-on'] == 'ubuntu-22.04'
assert jobs['build-windows']['runs-on'] == 'windows-latest'
text = open('.github/workflows/build.yml').read()
for needle in [
    'gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync',
    'grid-launcher-${VERSION}-x86_64.AppImage',
    'grid-launcher-${VERSION}-windows-x86_64-setup.exe',
    'GRID Launcher_${VERSION}_amd64.AppImage',
    'GRID Launcher_${VERSION}_x64-setup.exe',
]:
    assert needle in text, needle
print('build.yml OK')
PY
```

Expected: `build.yml OK`.

- [ ] **Step 6: Verify the Python build reference still exists**

Run: `ls build.sh appimage/`
Expected: both listed. They are the Python reference and must survive.

- [ ] **Step 7: Commit**

```bash
git commit --only -- .github/workflows -m "$(cat <<'MSG'
ci: one Build workflow for gates and release artifacts

rust-rewrite.yml becomes build.yml: check and a new check-windows on
every push and pull request, e2e on pushes to main, and build-linux and
build-windows on a created release or a dispatch dry run. The AppImage
is repacked with appimagetool -u so AppImageUpdate finds the update
string Tauri's bundler does not embed. The three PyInstaller/AppImage
workflows are deleted; build.sh and appimage/ stay as the Python
reference.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

### Task 6: Porting docs

**Files:**
- Modify: `docs/porting/02-config-and-secrets.md` — a new `## Rust port deviations — importer` section at the end of the file, after the existing `## Rust port deviations (milestone 7)` section.
- Modify: `docs/porting/10-identity-updates.md` — the `## External surfaces` section (lines ~36-56).

- [ ] **Step 1: Append the importer section to doc 02**

Append to the end of `docs/porting/02-config-and-secrets.md`:

```markdown
## Rust port deviations — importer

The rewrite reads a Python-era `~/.grid-launcher/config.json` exactly once per
profile and converts it into `config.toml` plus registry rows
(`rewrite/crates/grid-core/src/import_python.rs`, spec
`docs/superpowers/specs/2026-09-10-release-pipeline-and-importer-design.md`
Part 3). The trigger is file presence, checked in
`rewrite/app/src-tauri/src/python_import.rs` before any service reads the
config: no `config.toml` and a `~/.grid-launcher/config.json`. `config.toml`
is written whether or not every row converted, so the import never repeats.
`GRID_LAUNCHER_DATA_DIR` moves the Rust config only — the Python path is
fixed on every platform.

Every value passes through the same normalizers Python applied when it wrote
the file: trim; a blank emulator name drops the entry; a blank title or
platform drops the game; the save-strategy alias table
(`grid_launcher/emulator/profiles.py:141-156`); lenient booleans
(`grid-launcher.py:2185-2196`); the upload delay clamped to `[0, 60]`
(`grid-launcher.py:2221`).

### Mapping

| Python key | Rust destination |
| --- | --- |
| `server_url`, `username`, `library_path`, `launch_args` | `Config` fields of the same name |
| `debug_prints` | `Config.debug_prints` |
| `theme` (`system`/`dark`/`light`, else `system`) | `Config.ui.theme` — the Rust config has no top-level `theme` |
| `emulators[]` (`name`, `path`, `args`, `save_strategy`, `ignore_files`, `ignore_extensions`, `save_paths`, `state_paths`, `source_id`, `source_provider`, `source_owner`, `source_repo`, `source_release_tag`) | `Config.emulators[]`, same names; `source_installed_tag` blank; sorted by case-folded name |
| `default_emulators` | `Config.default_emulators` |
| `default_retroarch_cores` | `Config.retroarch_cores` |
| `default_compat_tool` | `Config.default_compat_tool` |
| `compat_tool_installs{id: {name, compat_tool_type, install_path}}` | `Config.compat_tool_installs[]` with `source_id = id`, `name`, `path = install_path`, `release_tag` blank; `compat_tool_type` dropped |
| the four `auto_cloud_save_*` keys | `Config` fields of the same name |
| `retroachievements_username` | `Config.retroachievements_username` |
| `native_manual_save_paths` (`<title>__manual` -> list) | `Config.native_manual_save_paths`, keys unchanged |
| `cloud_sync_state` | `Config.cloud_sync_state` as a TOML table; an unconvertible leaf (a JSON `null`) is dropped and the rest of the subtree kept |
| `installed_games[]` | one `InstalledGame` each: `rom_id` parsed as `i64` else `None`; `filesize_bytes` parsed else `0`; `installed_at = now`; `last_played_at = 0`; `cover_small_path`/`cover_large_path`/`fanart_urls` blank (images refetch from the server); `screenshot_urls`, `genres`, `regions`, `rating`, `description`, `rom_file_name`, `archive_path`, `extracted_path`, `extracted_dir`, `multi_file_game_dir`, `native_*`, `included_dlc`, `ps3_*`, `ps4_*`, `ra_id`, `server_updated_at` copied; `languages`, `tags`, `revision`, `companies`, `first_release_date` blank (Python stores none of them) |

A game with no `rom_id` is still imported: the registry keys on title and
platform, and `installed_match` accepts a `None` rom id.

### Skipped on purpose

`api_token`, `retroachievements_api_key`, `retroachievements_token` — the user
enters tokens again, and the Python keyring entries under service
`GRIDLauncher` are never read. `first_run_completed`, `window_geometry`,
`window_state` (see the top-level table above), `emulator_source_installs`
(the entry-level `source_*` fields already carry provenance), the three `tv_*`
keys and `tv_mode_last_active`, `cached_cover_path`, `cover_url`,
`local_path`. Unknown keys are ignored — never copied into `Config::extra`.

### Secrets

The importer deserializes into a struct that has **no field** for the three
secret keys, so `serde_json` drops those values while parsing and no token is
ever held, logged or written. The `ImportReport` and every log line carry
counts only. `plan`'s fixture test asserts that none of the three values, and
none of the three key names, appears in the serialized config.

### Surfacing

`AppState.python_import: Option<ImportReport>` is set once at startup; the
`python_import_notice` command returns it; the frontend pulls it on mount
(`app/src/lib/stores/pythonImport.svelte.ts`) and shows one toast — "Imported
N emulators and M games from the previous version. Enter your RomM token to
reconnect.", with "Enter your RetroAchievements token as well." appended only
when a username was imported. Same late-mount pattern as the app-update
notice (doc 10 D-10-k), with no event: the import finishes before the window
is created.
```

- [ ] **Step 2: Extend doc 10's External surfaces**

In `docs/porting/10-identity-updates.md`, replace the **AppImage update metadata** bullet in `## External surfaces` with:

```markdown
- **AppImage update metadata** — an `update_info` string embedded in the AppImage by
  `appimagetool`, of the form
  `gh-releases-zsync|Sixdd6|grid-launcher|latest|grid-launcher-*-x86_64.AppImage.zsync`
  (build.sh:228 for the Python reference). A companion `.zsync` file is produced beside it.
  This is consumed by external tooling (AppImageUpdate); **the application itself never
  reads it** — see Behavior, "App version flow".

  **rewrite**: Tauri's AppImage bundler embeds no update information, so
  `.github/workflows/build.yml`'s `build-linux` job repacks: it runs
  `--appimage-extract` on the bundler's output and rebuilds the AppDir with
  `appimagetool -u "<the same update string>"`. The string and the asset names are
  unchanged from the Python releases, so a Python AppImage updates itself to the Rust
  build; the Python-config importer (doc 02, "Rust port deviations — importer") is what
  makes that switch cost the user nothing but their tokens.

- **Release assets** (rewrite) — a created GitHub release attaches exactly three files,
  built by `.github/workflows/build.yml`:

  | Asset | Job | Built from |
  | --- | --- | --- |
  | `grid-launcher-<VERSION>-x86_64.AppImage` | `build-linux` (`ubuntu-22.04`) | `npx tauri build --bundles appimage`, then the `appimagetool -u` repack |
  | `grid-launcher-<VERSION>-x86_64.AppImage.zsync` | `build-linux` | written by the same `appimagetool` run |
  | `grid-launcher-<VERSION>-windows-x86_64-setup.exe` | `build-windows` (`windows-latest`) | `npx tauri build --bundles nsis`, renamed from `GRID Launcher_<VERSION>_x64-setup.exe` |

  The NSIS installer installs per user (`bundle.windows.nsis.installMode = "currentUser"`)
  and pulls WebView2 with the default `downloadBootstrapper`. Nothing is code-signed.

- **The version a release build reports** (rewrite) — `tauri.conf.json` keeps
  `"version": "0.9.0-dev"` in the source tree. Each build job passes the real version with
  `npx tauri build --config '{"version":"<VERSION>"}'`, and that is what
  `app.package_info().version` returns at runtime. On a `release` event `VERSION` is the tag
  with its leading `v` stripped, and the job fails unless it matches
  `^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$`. A `workflow_dispatch` run is a dry run at
  `0.0.0-dev`: it attaches nothing to any release, and the `dev` in the version suppresses
  the app's own update check (`app_update::is_dev_build`).
```

- [ ] **Step 3: Verify the docs say what shipped**

Run, from the repo root:

```bash
python3 - <<'PY'
doc02 = open('docs/porting/02-config-and-secrets.md').read()
doc10 = open('docs/porting/10-identity-updates.md').read()
for needle in [
    '## Rust port deviations — importer',
    'Config.ui.theme',
    'api_token', 'retroachievements_api_key', 'retroachievements_token',
    'python_import_notice',
]:
    assert needle in doc02, f'doc 02 is missing: {needle}'
for needle in [
    'grid-launcher-<VERSION>-x86_64.AppImage.zsync',
    'grid-launcher-<VERSION>-windows-x86_64-setup.exe',
    'appimagetool -u',
    'currentUser',
    '0.0.0-dev',
]:
    assert needle in doc10, f'doc 10 is missing: {needle}'
print('docs OK')
PY
```

Expected: `docs OK`.

- [ ] **Step 4: Verify every claim the docs make is true of the tree**

Run, from the repo root:

```bash
grep -n "gh-releases-zsync|Sixdd6|grid-launcher|latest" .github/workflows/build.yml
grep -n '"version": "0.9.0-dev"' rewrite/app/src-tauri/tauri.conf.json
grep -n 'installMode' rewrite/app/src-tauri/tauri.conf.json
grep -rn "pub fn python_import_notice" rewrite/app/src-tauri/src/commands/updates.rs
```

Expected: every command prints a match. A miss means the doc describes something Tasks 1-5 did not do — fix the doc, not the code.

- [ ] **Step 5: Close the CI memory note**

The memory note "CI disabled until parity — rust-rewrite workflow manual-only; at parity retire Python builds and promote it" is now satisfied: `rust-rewrite.yml` is `build.yml`, it runs on push, pull request and release, and the three Python workflows are gone. Tell the user in the final report so they can close the note; do not edit memory files from inside this plan.

- [ ] **Step 6: Commit**

```bash
git commit --only -- docs/porting/02-config-and-secrets.md docs/porting/10-identity-updates.md -m "$(cat <<'MSG'
docs: record the importer mapping and the release surfaces

Doc 02 gains a "Rust port deviations — importer" section with the full
Python-to-Rust mapping, the skipped-key list and the secret rule. Doc 10
gains the three release asset names, the appimagetool repack that embeds
the update string, the per-user NSIS installer, and how the build passes
the version to Tauri.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
MSG
)"
```

---

## Self-review

**1. Spec coverage.**

| Spec section | Task |
| --- | --- |
| Part 1, rename + delete three workflows | 5, Steps 1 and 3 |
| Part 1, triggers table | 5, Step 2 (job-level `if:`), verified in Step 5 |
| Part 1, `check-windows` | 5, Step 2 |
| Part 1, version step + semver gate + `0.0.0-dev` dry run | 5, Step 2 (both build jobs) |
| Part 1, `build-linux` incl. repack and zsync move | 5, Step 2 |
| Part 1, `build-windows` incl. rename | 5, Step 2 |
| Part 1, NSIS settings | 1, Step 1 |
| Part 2, identity | 1, Step 1 |
| Part 3, trigger (presence check) | 4, Steps 1-3, 7 |
| Part 3, module `plan` / `import` / errors | 2 and 3 |
| Part 3, mapping table | 2, Step 4 |
| Part 3, skipped keys / secrets | 2, Step 4 (`PythonConfig` has no secret field) + Step 2 test |
| Part 3, surfacing (`AppState`, command, toast) | 4, Steps 5-6, 11, 15, 17 |
| Part 3, tests (grid-core `plan`, `import` tempdir, app layer) | 2 Step 2, 3 Step 1, 4 Step 1 |
| Docs 02 and 10 | 6 |
| Memory: close the CI note | 6, Step 5 (reported to the user, not edited) |

No gaps.

**2. Placeholder scan.** No "TBD", "TODO", "implement later", "add appropriate error handling", "write tests for the above" or "similar to Task N" appears. Every code step carries the actual code; every verification step carries the actual command and its expected output. The one forward reference — Task 2 Step 4 telling the implementer to omit `Registry` and `Path` from the `use` block until Task 3 — is an explicit instruction with the exact replacement line, not a placeholder.

**3. Type consistency.** Checked against the real sources:

- `EmulatorEntry` field names match `crates/grid-core/src/config.rs:7-63` exactly, `source_installed_tag` included.
- `CompatToolInstall` is `{name, path, source_id, release_tag}` (`config.rs:70-80`) — the plan never invents a `compat_tool_type`.
- `Config.retroarch_cores` (not `default_retroarch_cores`) and `Config.ui.theme` (not `Config.theme`) — both verified in `config.rs`.
- `InstalledGame` field names match `library/registry.rs:246-311`; `fanart_urls` and `images_version` are covered by `..Default::default()` and named in the code comment.
- `normalize_save_strategy` is the real path `crate::autoconfig::entry::normalize_save_strategy` (`autoconfig/entry.rs:87`).
- `home_dir` is `grid_core::autoconfig::paths::home_dir` (`autoconfig/paths.rs:19`).
- `ImportReport`'s four fields are spelled identically in the Rust struct, the `python_import_notice` return type, the TypeScript `PythonImportReport`, and both frontend tests (`emulators`, `games`, `skipped_games`, `retroachievements` — snake_case on both sides, because the command serializes the Rust struct with no rename).
- `plan` returns `(Config, Vec<InstalledGame>, usize)` in Task 2's interface block, in its implementation, and in every Task 3 call site.
- `import(python_json, config_path, registry, now)` has the same argument order in Task 3's signature, its tests, and Task 4's `startup_import`.
- `pushToast` is the real export of `app/src/lib/stores/toasts.svelte.ts`; `importToastText` is spelled the same in `pythonImport.ts`, `pythonImport.test.ts` and `stores/pythonImport.svelte.ts`.
- Task 1's `productName` (`GRID Launcher`) is exactly what Task 5's `GRID Launcher_${VERSION}_amd64.AppImage` and `GRID Launcher_${VERSION}_x64-setup.exe` paths depend on.
