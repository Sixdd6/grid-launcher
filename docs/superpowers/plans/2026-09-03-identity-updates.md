# Identity / Updates (milestone 9) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port doc 10 (server-update detection, Update action, version label, app version) plus a check-only self-update notice into the Rust/Tauri rewrite.

**Architecture:** A pure detection module in grid-core, a new non-native `Update` install mode next to the existing native merge, an app-layer `UpdateService` that recomputes the update set on connect/install/uninstall and pushes it to the frontend as an event, a once-per-process GitHub release check, and Svelte surfacing (badge, button, version row, banner).

**Tech Stack:** Rust (grid-core, Tauri 2 app crate: chrono, regex, semver, tauri-plugin-opener, wiremock tests), Svelte 5 + TypeScript + vitest, WebdriverIO E2E with the mock RomM and mock forge.

**Spec:** `docs/superpowers/specs/2026-09-03-identity-updates-design.md` — binding. Read it before any task; it settles every conflict inside this plan.

All paths below are relative to `rewrite/` unless they start with `docs/` or `../`.

## Global Constraints

- **Token secrecy (hard):** no token, header, or URL with credentials in files, logs, error strings, IPC payloads, or console output. The GitHub check goes through `ForgeClient`, which never sends `Authorization`. Log hosts, never full URLs, on failure.
- **Verbatim strings:** `Update Available` (badge); `Update` / `Update to v<tag>` (button); `Version: v01234` / `Version: v3.6.0` (label; numeric zero-padded to 5); `A newer server version is no longer available for this game.` (`UPDATE_GONE`); `Updated '<title>' successfully.` (toast); `Native games update through the merge path.` (`NATIVE_UPDATE_REQUIRED`); drawer kind label for `update` is `Update`; window title `GRID Launcher <version>`; banner `GRID Launcher <tag> is available`, buttons `Open release` / `Dismiss`; native confirm label `Saves and configuration will be preserved — confirm update`.
- **Events:** `updates-changed` (payload `UpdateRow[]`), `app-update-available` (payload `{ tag, url }`).
- **Version tags:** numeric `\(v(\d{5})\)` (exactly five digits) tried before semver `\(v(\d+(?:\.\d+)+)\)`; case-insensitive; `(v1234)` matches neither. Kinds never compare across each other.
- **Timestamps:** every `Z` → `+00:00`; naive → UTC; unparseable → None; strict `>`.
- **Update set is never persisted** (doc 10 invariant 5); it lives only in `UpdateService`.
- **Rows without a rom id** are never checked and never offer Update (D-10-a/D-10-g).
- **Self-update:** exactly one `releases/latest` request per process; suppressed when the running version's pre-release contains the identifier `dev` (except under the `e2e` cargo feature with `GRID_LAUNCHER_E2E_UPDATE_CHECK=1`); any failure is silent at debug level.
- **Versions:** `app/src-tauri/tauri.conf.json` and `app/src-tauri/Cargo.toml` become `0.9.0-dev`.
- **Process rules:** unittest-style `#[tokio::test]`/`#[test]` in Rust, vitest in TS; never `git checkout/restore/reset` tracked files; commit with pathspecs after each task; run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before committing Rust; `npm run check` and `npm test` (from `app/`) before committing TS.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/library/update_detection.rs` (new) | pure tag parse/format/compare, timestamp parse, decision function |
| `crates/grid-core/src/library/mod.rs` | `InstallMode::Update`, `install_update`, `NATIVE_UPDATE_REQUIRED`, module decl |
| `crates/grid-core/tests/install_service.rs` | integration tests for `install_update` |
| `app/src-tauri/src/update_service.rs` (new) | update set, refresh pass, button label, `UPDATE_GONE` |
| `app/src-tauri/src/app_update.rs` (new) | self-update check: `is_newer`, `is_dev_build`, `spawn_check` |
| `app/src-tauri/src/commands/updates.rs` (new) | `list_updates`, `update_game`, `app_version`, `open_release_page` |
| `app/src-tauri/src/commands.rs` | `AppState.updates`, refresh triggers in connect/restore/retry/disconnect/uninstall |
| `app/src-tauri/src/lib.rs` | module decls, plugin, title, finalized-hook refresh, self-update spawn, handler list |
| `app/src-tauri/Cargo.toml`, `tauri.conf.json` | deps `tauri-plugin-opener`, `semver`; version `0.9.0-dev` |
| `app/src/lib/api.ts` | `DownloadKind` `'update'`, `UpdateRow`, `UPDATES_CHANGED_EVENT`, `APP_UPDATE_EVENT`, 4 wrappers |
| `app/src/lib/downloads/format.ts` (+test) | `kindLabel('update')` |
| `app/src/lib/stores/updates.svelte.ts` (+test, new) | update rows store, event listener |
| `app/src/lib/stores/appUpdate.svelte.ts` (new) | banner state |
| `app/src/lib/details/version.ts` (+test, new) | TS tag parser/formatter, `versionLabel` |
| `app/src/App.svelte` | init the two stores |
| `app/src/lib/Library.svelte`, `Details.svelte`, `Shell.svelte` | badge, button/confirm/toast/version row, banner |
| `e2e/fixtures-updates/*`, `e2e/seed/updates-seed.mjs`, `e2e/specs/updates.spec.ts`, `e2e/mock-romm/mock-forge.mjs` (+test), `e2e/mock-romm/server.mjs`, `e2e/wdio.conf.ts`, `scripts/e2e.sh` | E2E group `updates` |
| `docs/porting/10-identity-updates.md`, `docs/porting/03-library-install.md`, `README.md` | deviations, mode table, checklist |

---

### Task 1: Core detection module

**Files:**
- Create: `crates/grid-core/src/library/update_detection.rs`
- Modify: `crates/grid-core/src/library/mod.rs` (add `pub mod update_detection;` next to the other `pub mod` lines near the top)

**Interfaces:**
- Consumes: `library::registry::InstalledGame` (`platform`, `rom_file_name`, `server_updated_at`).
- Produces: everything in the code below; Tasks 3, 5 and 7 use `VersionTag`, `rom_file_name_version`, `format_version_tag`, `has_newer_server_rom_version`, `game_has_server_update`, `ServerVersion`, `is_emulators_platform`, `is_windows_pc_platform`.

- [ ] **Step 1: Write the module with its tests**

```rust
//! Server-update detection (`grid_launcher/library/update_detection.py`,
//! docs/porting/10-identity-updates.md "Update detection flow"). Pure: no
//! I/O, no clock. The app layer feeds it one installed row and the server's
//! current view of the same rom.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use regex::Regex;
use std::sync::OnceLock;

use super::registry::InstalledGame;

/// A version tag found inside a rom file name. The two kinds never compare
/// against each other (update_detection.py:64-65).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionTag {
    /// `(vNNNNN)` — exactly five digits.
    Numeric(u32),
    /// `(vX.Y[.Z…])` — at least one dot.
    Semver(Vec<u32>),
}

fn numeric_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\(v(\d{5})\)").unwrap())
}

fn semver_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\(v(\d+(?:\.\d+)+)\)").unwrap())
}

/// `rom_file_name_version` (update_detection.py:20-31): numeric first, then
/// semver, else `None`. `(v1234)` matches neither.
pub fn rom_file_name_version(rom_file_name: &str) -> Option<VersionTag> {
    if let Some(caps) = numeric_re().captures(rom_file_name) {
        return caps[1].parse().ok().map(VersionTag::Numeric);
    }
    let caps = semver_re().captures(rom_file_name)?;
    let parts: Option<Vec<u32>> = caps[1].split('.').map(|p| p.parse().ok()).collect();
    parts.map(VersionTag::Semver)
}

/// `_format_version_tag_for_ui` (grid-launcher.py:3273-3280): `v01234` for a
/// numeric tag, `v3.6.0` for a semver tag.
pub fn format_version_tag(tag: &VersionTag) -> String {
    match tag {
        VersionTag::Numeric(n) => format!("v{n:05}"),
        VersionTag::Semver(parts) => {
            let joined: Vec<String> = parts.iter().map(u32::to_string).collect();
            format!("v{}", joined.join("."))
        }
    }
}

fn semver_is_newer(installed: &[u32], server: &[u32]) -> bool {
    let len = installed.len().max(server.len());
    for i in 0..len {
        let a = installed.get(i).copied().unwrap_or(0);
        let b = server.get(i).copied().unwrap_or(0);
        if b > a {
            return true;
        }
        if b < a {
            return false;
        }
    }
    false
}

/// `has_newer_server_rom_version` (update_detection.py:56-70).
pub fn has_newer_server_rom_version(installed_name: &str, server_name: &str) -> bool {
    let (Some(installed), Some(server)) = (
        rom_file_name_version(installed_name),
        rom_file_name_version(server_name),
    ) else {
        return false;
    };
    match (installed, server) {
        (VersionTag::Numeric(a), VersionTag::Numeric(b)) => b > a,
        (VersionTag::Semver(a), VersionTag::Semver(b)) => semver_is_newer(&a, &b),
        _ => false,
    }
}

/// `_is_windows_pc_platform` (update_detection.py:73-80).
pub fn is_windows_pc_platform(platform: &str) -> bool {
    let normalized = platform.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    normalized.contains("windows") || normalized == "pc"
}

/// The default emulators-platform predicate (update_detection.py:103).
pub fn is_emulators_platform(platform: &str) -> bool {
    platform.trim().to_lowercase() == "emulators"
}

/// `_parse_timestamp` (update_detection.py:83-94): every `Z` becomes
/// `+00:00`; an offset-less value is taken as UTC; anything unparseable is
/// `None`, never an error.
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let candidate = text.replace('Z', "+00:00");
    if let Ok(parsed) = DateTime::parse_from_rfc3339(&candidate) {
        return Some(parsed.with_timezone(&Utc));
    }
    const NAIVE: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];
    for format in NAIVE {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&candidate, format) {
            return Some(naive.and_utc());
        }
    }
    NaiveDate::parse_from_str(&candidate, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|naive| naive.and_utc())
}

/// The server's current view of one rom — the three fields the decision
/// reads. Built by the app layer from a `RomDetail`.
#[derive(Debug, Clone, Copy)]
pub struct ServerVersion<'a> {
    pub platform: &'a str,
    pub rom_file_name: &'a str,
    pub updated_at: &'a str,
}

/// `game_has_server_update` (update_detection.py:97-122).
pub fn game_has_server_update(installed: &InstalledGame, server: &ServerVersion<'_>) -> bool {
    if is_emulators_platform(&installed.platform) || is_emulators_platform(server.platform) {
        return false;
    }
    if (is_windows_pc_platform(&installed.platform) || is_windows_pc_platform(server.platform))
        && has_newer_server_rom_version(&installed.rom_file_name, server.rom_file_name)
    {
        return true;
    }
    // Legacy installs carry no install-time server timestamp.
    let Some(installed_at) = parse_timestamp(&installed.server_updated_at) else {
        return false;
    };
    let Some(server_at) = parse_timestamp(server.updated_at) else {
        return false;
    };
    server_at > installed_at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(platform: &str, rom_file_name: &str, server_updated_at: &str) -> InstalledGame {
        InstalledGame {
            title: "Game".to_string(),
            platform: platform.to_string(),
            rom_file_name: rom_file_name.to_string(),
            server_updated_at: server_updated_at.to_string(),
            ..Default::default()
        }
    }

    fn server<'a>(platform: &'a str, rom_file_name: &'a str, updated_at: &'a str) -> ServerVersion<'a> {
        ServerVersion { platform, rom_file_name, updated_at }
    }

    // tests/test_update_detection.py:162-176
    #[test]
    fn extracts_v_five_digits() {
        assert_eq!(rom_file_name_version("My Game (v00042).zip"), Some(VersionTag::Numeric(42)));
    }

    #[test]
    fn extracts_semver_from_real_filename() {
        assert_eq!(
            rom_file_name_version("A Little to the Left (v3.6.0) (2022) (W_P).7z"),
            Some(VersionTag::Semver(vec![3, 6, 0]))
        );
    }

    #[test]
    fn none_without_matching_tag() {
        assert_eq!(rom_file_name_version("My Game (v1234).zip"), None);
        assert_eq!(rom_file_name_version("My Game.zip"), None);
    }

    #[test]
    fn tag_match_is_case_insensitive() {
        assert_eq!(rom_file_name_version("x (V00007).zip"), Some(VersionTag::Numeric(7)));
    }

    #[test]
    fn numeric_is_preferred_over_semver_when_both_present() {
        assert_eq!(
            rom_file_name_version("x (v1.2) (v00003).zip"),
            Some(VersionTag::Numeric(3))
        );
    }

    #[test]
    fn formats_numeric_zero_padded_and_semver_verbatim() {
        assert_eq!(format_version_tag(&VersionTag::Numeric(42)), "v00042");
        assert_eq!(format_version_tag(&VersionTag::Semver(vec![3, 6, 0])), "v3.6.0");
    }

    // tests/test_update_detection.py:178-238
    #[test]
    fn compares_numerically() {
        assert!(has_newer_server_rom_version("My Game (v00009).zip", "My Game (v00010).zip"));
        assert!(!has_newer_server_rom_version("My Game (v00010).zip", "My Game (v00010).zip"));
        assert!(!has_newer_server_rom_version("My Game (v00011).zip", "My Game (v00010).zip"));
    }

    #[test]
    fn false_when_missing_tags() {
        assert!(!has_newer_server_rom_version("My Game.zip", "My Game (v00010).zip"));
        assert!(!has_newer_server_rom_version("My Game (v00010).zip", "My Game.zip"));
    }

    #[test]
    fn compares_dotted_semver_parts() {
        let a = "A Little to the Left (v3.5.9) (2022) (W_P).7z";
        let b = "A Little to the Left (v3.6.0) (2022) (W_P).7z";
        let c = "A Little to the Left (v3.6.0.1) (2022) (W_P).7z";
        assert!(has_newer_server_rom_version(a, b));
        assert!(!has_newer_server_rom_version(b, a));
        assert!(!has_newer_server_rom_version(b, b));
        assert!(has_newer_server_rom_version(b, c));
        assert!(!has_newer_server_rom_version("x (v1.2).zip", "x (v1.2.0).zip"));
        assert!(has_newer_server_rom_version("x (v1.2).zip", "x (v1.2.1).zip"));
    }

    #[test]
    fn mixed_numeric_and_semver_is_false() {
        assert!(!has_newer_server_rom_version("My Game (v01234).zip", "My Game (v3.6.0).zip"));
        assert!(!has_newer_server_rom_version("My Game (v3.6.0).zip", "My Game (v01234).zip"));
    }

    #[test]
    fn windows_pc_platform_predicate() {
        assert!(is_windows_pc_platform("Windows"));
        assert!(is_windows_pc_platform(" windows 10 "));
        assert!(is_windows_pc_platform("PC"));
        assert!(!is_windows_pc_platform("PC Engine"));
        assert!(!is_windows_pc_platform(""));
        assert!(!is_windows_pc_platform("PS2"));
    }

    #[test]
    fn parses_timestamps_z_naive_and_garbage() {
        let z = parse_timestamp("2026-04-10T14:30:00Z").unwrap();
        let offset = parse_timestamp("2026-04-10T16:30:00+02:00").unwrap();
        let naive = parse_timestamp("2026-04-10T14:30:00").unwrap();
        let spaced = parse_timestamp(" 2026-04-10 14:30:00 ").unwrap();
        let fractional = parse_timestamp("2026-04-10T14:30:00.250Z").unwrap();
        assert_eq!(z, offset);
        assert_eq!(z, naive);
        assert_eq!(z, spaced);
        assert!(fractional > z);
        assert_eq!(parse_timestamp("2026-04-10").unwrap().to_rfc3339(), "2026-04-10T00:00:00+00:00");
        assert_eq!(parse_timestamp(""), None);
        assert_eq!(parse_timestamp("   "), None);
        assert_eq!(parse_timestamp("not a date"), None);
        assert_eq!(parse_timestamp("2026-13-45T00:00:00Z"), None);
    }

    // tests/test_update_detection.py:110-160
    #[test]
    fn true_when_server_timestamp_is_newer() {
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(game_has_server_update(&installed, &server("PS2", "", "2026-04-10T14:30:00Z")));
    }

    #[test]
    fn equal_timestamps_are_not_an_update() {
        let installed = row("PS2", "", "2026-04-10T14:30:00Z");
        assert!(!game_has_server_update(&installed, &server("PS2", "", "2026-04-10T14:30:00Z")));
    }

    #[test]
    fn legacy_install_without_timestamp_is_false() {
        let installed = row("PS2", "", "");
        assert!(!game_has_server_update(&installed, &server("PS2", "", "2026-04-10T14:30:00Z")));
    }

    #[test]
    fn unparseable_server_timestamp_is_false() {
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(&installed, &server("PS2", "", "soon")));
        assert!(!game_has_server_update(&installed, &server("PS2", "", "")));
    }

    #[test]
    fn emulators_platform_is_vetoed_on_either_side() {
        let installed = row("Emulators", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(&installed, &server("Emulators", "", "2026-04-10T14:30:00Z")));
        let installed = row("PS2", "", "2026-04-09T14:30:00Z");
        assert!(!game_has_server_update(&installed, &server(" emulators ", "", "2026-04-10T14:30:00Z")));
    }

    #[test]
    fn windows_uses_rom_file_version_without_timestamps() {
        let installed = row("Windows", "Windows Game (v00009).zip", "");
        assert!(game_has_server_update(
            &installed,
            &server("Windows", "Windows Game (v00010).zip", "")
        ));
    }

    #[test]
    fn windows_older_tag_falls_through_to_timestamps() {
        let installed = row("Windows", "Windows Game (v00010).zip", "2026-04-09T14:30:00Z");
        // The tag says "not newer"; the timestamp still decides.
        assert!(game_has_server_update(
            &installed,
            &server("Windows", "Windows Game (v00010).zip", "2026-04-10T14:30:00Z")
        ));
        assert!(!game_has_server_update(
            &installed,
            &server("Windows", "Windows Game (v00009).zip", "2026-04-09T14:30:00Z")
        ));
    }

    #[test]
    fn non_windows_ignores_rom_file_version() {
        let installed = row("PS2", "PS2 Game (v00009).zip", "");
        assert!(!game_has_server_update(&installed, &server("PS2", "PS2 Game (v00010).zip", "")));
    }

    #[test]
    fn pc_platform_on_the_server_side_enables_the_tag_check() {
        let installed = row("PS2", "Game (v00009).zip", "");
        assert!(game_has_server_update(&installed, &server("PC", "Game (v00010).zip", "")));
    }
}
```

- [ ] **Step 2: Register the module and run the tests**

Add `pub mod update_detection;` to `crates/grid-core/src/library/mod.rs` beside the other library submodule declarations.

Run: `cargo test -p grid-core update_detection`
Expected: all tests above pass.

- [ ] **Step 3: Lint and commit**

```bash
cargo fmt && cargo clippy -p grid-core --all-targets -- -D warnings
git add crates/grid-core/src/library/update_detection.rs crates/grid-core/src/library/mod.rs
git commit -m "rewrite: update_detection core module (doc 10 tag/timestamp rules)"
```

---

### Task 2: `InstallMode::Update` and `install_update`

**Files:**
- Modify: `crates/grid-core/src/library/mod.rs` (`InstallMode` enum ~line 170, `kind()`, a new method after `install_native_update` ~line 911, `finalize_inner` ~line 1463, verbatim-string constants near `NOT_INSTALLED`)
- Modify: `crates/grid-core/tests/install_service.rs` (new tests after `a_same_title_platform_row_with_a_different_rom_id_does_not_skip_finalize`)
- Modify: `app/src/lib/api.ts` (`DownloadKind` union), `app/src/lib/downloads/format.ts` (`kindLabel`), `app/src/lib/downloads/format.test.ts` (or wherever `kindLabel` is tested — `grep -rn "kindLabel" app/src --include=*.test.ts`)

**Interfaces:**
- Consumes: `plan_install(detail, library, client)`, `current_row(rom_id)`, `admit(...)`, `JobKey::Rom`, `is_native_platform`.
- Produces: `InstallMode::Update` (`kind()` = `"update"`); `pub async fn install_update(self: &Arc<Self>, client: Arc<RommClient>, rom_id: i64) -> Result<(), LibraryError>`; `pub const NATIVE_UPDATE_REQUIRED: &str = "Native games update through the merge path.";` (make it `pub` — Task 3 does not need it, but the test does). Frontend: `DownloadKind` includes `'update'`, `kindLabel('update') === 'Update'`.

- [ ] **Step 1: Write the failing integration tests**

Append to `crates/grid-core/tests/install_service.rs` (uses the existing `Harness`, `write_zip`, `detail_json`, `file_spec`, `InstalledGame`, `DownloadStatus` imports; add `use grid_core::library::NATIVE_UPDATE_REQUIRED;` if the file does not glob-import the library module):

```rust
#[tokio::test]
async fn install_update_re_extracts_and_replaces_the_row() {
    let harness = Harness::new().await;
    let staging = tempfile::tempdir().unwrap();
    let bytes = write_zip(
        &staging.path().join("chrono (v00002).zip"),
        &[("game.sfc", b"NEWDATA")],
    );
    harness
        .registry
        .upsert(&InstalledGame {
            title: "Chrono Trigger".to_string(),
            platform: "SNES".to_string(),
            rom_id: Some(1),
            rom_file_name: "chrono (v00001).zip".to_string(),
            archive_path: "/somewhere/chrono.zip".to_string(),
            server_updated_at: "2025-01-01T00:00:00Z".to_string(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();

    let mut detail = detail_json(
        1,
        "Chrono Trigger",
        "SNES",
        "chrono (v00002).zip",
        &[file_spec(11, "chrono (v00002).zip", bytes.len())],
    );
    detail["updated_at"] = serde_json::json!("2026-06-01T00:00:00Z");
    harness.mount_detail(1, detail).await;
    harness.mount_content(1, "chrono (v00002).zip", bytes, 0).await;

    harness
        .service
        .install_update(harness.client.clone(), 1)
        .await
        .unwrap();
    let id = harness.newest_entry_id();
    let entry = harness.wait_terminal(id).await;
    assert_eq!(entry.status, DownloadStatus::Completed, "{}", entry.error);
    assert_eq!(entry.kind, "update");

    // Unlike a base install of an installed rom, the update DID finalize.
    let extracted = harness.library.join("SNES/chrono (v00002)/game.sfc");
    assert_eq!(std::fs::read(&extracted).unwrap(), b"NEWDATA");
    let row = harness.registry.find(Some(1), "", "").unwrap().unwrap();
    assert_eq!(row.rom_file_name, "chrono (v00002).zip");
    assert_eq!(row.server_updated_at, "2026-06-01T00:00:00Z");
    assert_eq!(row.extracted_dir, harness.library.join("SNES/chrono (v00002)").to_string_lossy());
    assert_ne!(row.installed_at, 1);
    assert_eq!(harness.registry.all().unwrap().len(), 1, "the row was replaced, not duplicated");
}

#[tokio::test]
async fn install_update_of_an_unknown_rom_reports_not_installed() {
    let harness = Harness::new().await;
    let err = harness
        .service
        .install_update(harness.client.clone(), 99)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not installed"), "{err}");
    assert!(harness.service.snapshot().entries.is_empty());
}

#[tokio::test]
async fn install_update_refuses_a_native_row() {
    let harness = Harness::new().await;
    harness
        .registry
        .upsert(&InstalledGame {
            title: "My Game".to_string(),
            platform: "Windows".to_string(),
            rom_id: Some(7),
            rom_file_name: "mygame.zip".to_string(),
            extracted_dir: harness.library.to_string_lossy().into_owned(),
            installed_at: 1,
            ..Default::default()
        })
        .unwrap();
    let err = harness
        .service
        .install_update(harness.client.clone(), 7)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), NATIVE_UPDATE_REQUIRED);
    assert!(harness.service.snapshot().entries.is_empty());
}
```

Check the exact `NOT_INSTALLED` text with `grep -n "NOT_INSTALLED" crates/grid-core/src/library/mod.rs` and match the `contains(...)` assertion to it.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p grid-core --test install_service install_update`
Expected: compile error (`install_update` and `NATIVE_UPDATE_REQUIRED` undefined).

- [ ] **Step 3: Implement**

In `crates/grid-core/src/library/mod.rs`:

1. Add the variant and its kind:

```rust
pub enum InstallMode {
    Base,
    /// A non-native game re-installed over its existing install ("update"
    /// mode, install_mixin.py:1867): the base pipeline, minus the
    /// already-installed short-circuit.
    Update,
    Ps4Content,
    Xbox360Content,
    NativeUpdate,
}
// kind(): InstallMode::Update => "update",
```

2. Next to `NOT_INSTALLED`:

```rust
/// `install_update` refuses native rows: those merge through
/// `install_native_update` instead of replacing the install.
pub const NATIVE_UPDATE_REQUIRED: &str = "Native games update through the merge path.";
```

3. After `install_native_update`:

```rust
    /// Starts (or queues) a plain re-install of an already installed
    /// non-native game (Python "update" mode, doc 10 "Performing the update").
    /// Same plan as a base install, but the job is marked `Update` so
    /// `finish_download` never short-circuits on the existing row and
    /// `finalize_base` replaces it. Admitted under `JobKey::Rom`, so an update
    /// and a base install of the same rom can never run side by side.
    pub async fn install_update(
        self: &Arc<Self>,
        client: Arc<RommClient>,
        rom_id: i64,
    ) -> Result<(), LibraryError> {
        let library = self.library_root()?;
        let row = self.current_row(rom_id)?;
        if is_native_platform(&row.platform) {
            return Err(LibraryError::Extract(NATIVE_UPDATE_REQUIRED.to_string()));
        }
        let detail = client.rom_detail(rom_id).await?;
        let mut job = plan_install(&detail, &library, client)?;
        job.mode = InstallMode::Update;
        let key = JobKey::Rom(job.rom_id);
        let title = job.detail.name.clone();
        let platform = job.detail.platform_name.clone();
        self.admit(key, &title, &platform, InstallMode::Update.kind(), JobPayload::Game(job));
        Ok(())
    }
```

Put `library_root()` before `current_row()` so a missing library path errors first, matching `install_content`.

4. `finalize_inner`: `InstallMode::Base | InstallMode::Update => self.finalize_base(id, job, warning),` and update its doc comment ("`Base` and `Update` lay a registry row down").

5. `finish_download`'s skip condition already tests `job.mode == InstallMode::Base`; leave it and extend the comment: "an `Update` job exists precisely to bypass this".

6. Update the `kind()` unit test near line 2983 with `assert_eq!(InstallMode::Update.kind(), "update");`.

Frontend:
- `app/src/lib/api.ts`: add `| 'update'` to `DownloadKind` (after `'base'`).
- `app/src/lib/downloads/format.ts`: `case 'native_update': case 'update': return 'Update';`.
- Add a vitest case asserting `kindLabel('update') === 'Update'` and `actionFor('downloading', 'update') === 'cancel'` in the file that already tests `kindLabel`.

- [ ] **Step 4: Run tests, lint, commit**

```bash
cargo test -p grid-core --test install_service install_update
cargo test -p grid-core kind
cargo fmt && cargo clippy -p grid-core --all-targets -- -D warnings
(cd app && npm run check && npm test)
git add crates/grid-core/src/library/mod.rs crates/grid-core/tests/install_service.rs app/src/lib/api.ts app/src/lib/downloads/format.ts app/src/lib/downloads/*.test.ts
git commit -m "rewrite: InstallMode::Update — plain re-install of an installed non-native rom"
```

---

### Task 3: App-layer `UpdateService`, commands, wiring, title, version bump

**Files:**
- Create: `app/src-tauri/src/update_service.rs`, `app/src-tauri/src/commands/updates.rs`
- Modify: `app/src-tauri/src/commands.rs` (`AppState`, `connect`, `restore_session`, `retry_connect`, `disconnect`, `uninstall_game`), `app/src-tauri/src/lib.rs` (mod decls, `AppState` construction, opener plugin, window title, finalized hook, handler list), `app/src-tauri/Cargo.toml` (`tauri-plugin-opener = "2"`, version), `app/src-tauri/tauri.conf.json` (version)
- Modify: `app/src/lib/api.ts` (types, events, wrappers)

**Interfaces:**
- Consumes (Task 1): `grid_core::library::update_detection::{game_has_server_update, has_newer_server_rom_version, rom_file_name_version, format_version_tag, is_emulators_platform, ServerVersion}`; (Task 2): `InstallService::install_update`; existing `install_native_update`, `registry().all()`, `SessionManager::client()`, `RommClient::rom_detail`, `is_native_platform` (`grid_core::library::platforms`).
- Produces: `update_service::{UpdateService, UpdateRow, UPDATES_CHANGED_EVENT, UPDATE_GONE}`; commands `list_updates`, `update_game`, `app_version`, `open_release_page`; `AppState.updates: Arc<UpdateService>`; frontend `api.listUpdates()`, `api.updateGame(romId)`, `api.appVersion()`, `api.openReleasePage(url)`, `UpdateRow`, `UPDATES_CHANGED_EVENT`.

- [ ] **Step 1: `update_service.rs`**

```rust
//! App-layer server-update tracking (doc 10 "When checks run" / "How an
//! available update is surfaced"). grid-core decides whether ONE row has an
//! update (`library::update_detection`); this module decides WHEN the whole
//! library is re-checked and holds the transient result. Never persisted
//! (doc 10 invariant 5).
//!
//! Triggers (commands.rs / lib.rs): a session comes up (connect, restore,
//! retry), a game finalizes (any install mode), a game is uninstalled. A
//! disconnect clears the set. No timer, no polling.
//!
//! Token secrecy: nothing here logs a URL or header; fetch failures log the
//! rom id only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use grid_core::library::registry::InstalledGame;
use grid_core::library::update_detection::{
    format_version_tag, game_has_server_update, has_newer_server_rom_version,
    is_emulators_platform, rom_file_name_version, ServerVersion,
};
use grid_core::library::InstallService;
use grid_core::session::SessionManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

pub const UPDATES_CHANGED_EVENT: &str = "updates-changed";
/// Verbatim (details_view_mixin.py:1818).
pub const UPDATE_GONE: &str = "A newer server version is no longer available for this game.";
const MAX_IN_FLIGHT: usize = 4;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub server_rom_file_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateRow {
    pub rom_id: i64,
    pub label: String,
}

pub struct UpdateService {
    available: Mutex<HashMap<i64, UpdateInfo>>,
    generation: AtomicU64,
}

impl UpdateService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { available: Mutex::new(HashMap::new()), generation: AtomicU64::new(0) })
    }

    /// `_details_update_button_text_for_game` (grid-launcher.py:3300-3325).
    pub fn button_label_for(installed_rom_file_name: &str, server_rom_file_name: &str) -> String {
        if !has_newer_server_rom_version(installed_rom_file_name, server_rom_file_name) {
            return "Update".to_string();
        }
        match rom_file_name_version(server_rom_file_name) {
            Some(tag) => format!("Update to {}", format_version_tag(&tag)),
            None => "Update".to_string(),
        }
    }

    /// The rows the frontend renders, in ascending rom id order.
    pub fn rows(&self, installed: &[InstalledGame]) -> Vec<UpdateRow> {
        let available = self.available.lock().unwrap();
        let mut rows: Vec<UpdateRow> = installed
            .iter()
            .filter_map(|row| {
                let rom_id = row.rom_id?;
                let info = available.get(&rom_id)?;
                Some(UpdateRow {
                    rom_id,
                    label: Self::button_label_for(&row.rom_file_name, &info.server_rom_file_name),
                })
            })
            .collect();
        rows.sort_by_key(|r| r.rom_id);
        rows
    }

    pub fn has_update(&self, rom_id: i64) -> bool {
        self.available.lock().unwrap().contains_key(&rom_id)
    }

    /// Drops every entry; emits the event only when something changed.
    pub fn clear(&self, app: &AppHandle) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let was_empty = {
            let mut available = self.available.lock().unwrap();
            let was_empty = available.is_empty();
            available.clear();
            was_empty
        };
        if !was_empty {
            let _ = app.emit(UPDATES_CHANGED_EVENT, Vec::<UpdateRow>::new());
        }
    }

    /// One full pass over the registry. Runs on Tauri's async runtime; a
    /// pass that is overtaken by a newer one discards its result.
    pub fn spawn_refresh(self: &Arc<Self>, app: AppHandle, session: Arc<SessionManager>, install: Arc<InstallService>) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            this.refresh(app, session, install).await;
        });
    }

    async fn refresh(self: Arc<Self>, app: AppHandle, session: Arc<SessionManager>, install: Arc<InstallService>) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(client) = session.client() else {
            self.clear(&app);
            return;
        };
        let rows = match tokio::task::spawn_blocking({
            let install = install.clone();
            move || install.registry().all()
        })
        .await
        {
            Ok(Ok(rows)) => rows,
            _ => {
                tracing::warn!("update check skipped: registry read failed");
                return;
            }
        };
        let semaphore = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
        let mut tasks = Vec::new();
        for row in rows {
            let Some(rom_id) = row.rom_id else { continue };
            if is_emulators_platform(&row.platform) {
                continue;
            }
            let client = client.clone();
            let semaphore = semaphore.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = semaphore.acquire_owned().await.ok()?;
                let detail = match client.rom_detail(rom_id).await {
                    Ok(detail) => detail,
                    Err(_) => {
                        tracing::debug!("update check: rom {rom_id} detail fetch failed");
                        return None;
                    }
                };
                let server = ServerVersion {
                    platform: &detail.platform_name,
                    rom_file_name: &detail.fs_name,
                    updated_at: &detail.server_updated_at,
                };
                game_has_server_update(&row, &server)
                    .then(|| (rom_id, UpdateInfo { server_rom_file_name: detail.fs_name.clone() }))
            }));
        }
        let mut next = HashMap::new();
        for task in tasks {
            if let Ok(Some((rom_id, info))) = task.await {
                next.insert(rom_id, info);
            }
        }
        if self.generation.load(Ordering::SeqCst) != generation {
            return; // overtaken by a newer pass (or a clear)
        }
        *self.available.lock().unwrap() = next;
        let installed = install.registry().all().unwrap_or_default();
        let _ = app.emit(UPDATES_CHANGED_EVENT, self.rows(&installed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_label_names_the_target_only_when_the_server_tag_is_newer() {
        assert_eq!(UpdateService::button_label_for("g (v1.0.0).zip", "g (v1.1.0).zip"), "Update to v1.1.0");
        assert_eq!(UpdateService::button_label_for("g (v00009).zip", "g (v00010).zip"), "Update to v00010");
        assert_eq!(UpdateService::button_label_for("g (v1.1.0).zip", "g (v1.0.0).zip"), "Update");
        assert_eq!(UpdateService::button_label_for("g.zip", "g (v1.0.0).zip"), "Update");
        assert_eq!(UpdateService::button_label_for("g (v01234).zip", "g (v1.0.0).zip"), "Update");
    }

    #[test]
    fn rows_only_lists_installed_rows_with_an_entry_and_a_rom_id() {
        let service = UpdateService::new();
        service.available.lock().unwrap().insert(2, UpdateInfo { server_rom_file_name: "b (v2.0).zip".into() });
        service.available.lock().unwrap().insert(9, UpdateInfo { server_rom_file_name: "gone.zip".into() });
        let installed = vec![
            InstalledGame { rom_id: Some(1), rom_file_name: "a.zip".into(), ..Default::default() },
            InstalledGame { rom_id: Some(2), rom_file_name: "b (v1.0).zip".into(), ..Default::default() },
            InstalledGame { rom_id: None, ..Default::default() },
        ];
        assert_eq!(service.rows(&installed), vec![UpdateRow { rom_id: 2, label: "Update to v2.0".into() }]);
        assert!(service.has_update(2));
        assert!(!service.has_update(1));
    }
}
```

If `Registry::all()` does not exist under that name, use the accessor `lib.rs` already calls at startup (`install.registry().all()` appears in the sweep code) — it does exist.

- [ ] **Step 2: `commands/updates.rs`**

```rust
//! Update commands (doc 10): the update set for the UI, the Update action,
//! the app's own version, and the release-page opener for the self-update
//! banner. Thin wrappers — every rule lives in `update_service`,
//! `app_update`, or grid-core.

use grid_core::library::platforms::is_native_platform;
use grid_core::library::update_detection::{game_has_server_update, ServerVersion};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{err, AppState};
use crate::update_service::{UpdateRow, UPDATE_GONE};

/// The only URL prefix `open_release_page` will hand to the OS.
pub const RELEASE_URL_PREFIX: &str = "https://github.com/Sixdd6/grid-launcher/releases/";

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateRow>, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let updates = state.updates.clone();
    tokio::task::spawn_blocking(move || {
        let installed = install.registry().all().map_err(err)?;
        Ok(updates.rows(&installed))
    })
    .await
    .map_err(|e| format!("list_updates did not finish: {e}"))?
}

/// `_perform_game_update_action` (details_view_mixin.py:1803-1884), minus
/// the modal: the frontend confirms native updates before calling this.
#[tauri::command]
pub async fn update_game(state: State<'_, AppState>, app: AppHandle, rom_id: i64) -> Result<(), String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let client = state.session.client().ok_or("not connected")?;
    let session = state.session.clone();
    let updates = state.updates.clone();

    let row = {
        let install = install.clone();
        tokio::task::spawn_blocking(move || install.registry().find(Some(rom_id), "", ""))
            .await
            .map_err(|e| format!("update_game did not finish: {e}"))?
            .map_err(err)?
            .filter(|row| row.rom_id == Some(rom_id))
            .ok_or_else(|| grid_core::library::NOT_INSTALLED.to_string())?
    };

    let detail = match client.rom_detail(rom_id).await {
        Ok(detail) => detail,
        Err(_) => {
            updates.spawn_refresh(app, session, install);
            return Err(UPDATE_GONE.to_string());
        }
    };
    let server = ServerVersion {
        platform: &detail.platform_name,
        rom_file_name: &detail.fs_name,
        updated_at: &detail.server_updated_at,
    };
    if !game_has_server_update(&row, &server) {
        updates.spawn_refresh(app, session, install);
        return Err(UPDATE_GONE.to_string());
    }
    if is_native_platform(&row.platform) {
        install.install_native_update(client, rom_id).await.map_err(err)
    } else {
        install.install_update(client, rom_id).await.map_err(err)
    }
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

pub fn is_release_url(url: &str) -> bool {
    url.starts_with(RELEASE_URL_PREFIX)
}

#[tauri::command]
pub fn open_release_page(app: AppHandle, url: String) -> Result<(), String> {
    if !is_release_url(&url) {
        return Err("refusing to open a non-release URL".to_string());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::is_release_url;

    #[test]
    fn only_the_repo_release_prefix_opens() {
        assert!(is_release_url("https://github.com/Sixdd6/grid-launcher/releases/tag/v1.0.0"));
        assert!(!is_release_url("https://github.com/Sixdd6/grid-launcher/"));
        assert!(!is_release_url("https://example.com/releases/"));
        assert!(!is_release_url("http://github.com/Sixdd6/grid-launcher/releases/tag/v1"));
    }
}
```

Check that `NOT_INSTALLED` is `pub` in grid-core; if it is private, make it `pub` in Task 3 (one-line change in `library/mod.rs`) rather than duplicating the string.

- [ ] **Step 3: Wire `commands.rs` and `lib.rs`**

`commands.rs`:
- `AppState` gains `pub updates: Arc<crate::update_service::UpdateService>,` with a doc comment.
- `connect`, `retry_connect`: after the replenish spawn, `state.updates.spawn_refresh(app.clone(), state.session.clone(), install.clone());` inside the same `if let Ok(install)` block (clone `app` before the first use; `AppHandle` is `Clone`).
- `restore_session`: same, inside the `Connected` branch.
- `disconnect`: signature gains `app: tauri::AppHandle`; after a successful disconnect call `state.updates.clear(&app);`.
- `uninstall_game`: signature gains `app: tauri::AppHandle`; on `Ok`, `state.updates.spawn_refresh(app, state.session.clone(), install.clone())` (clone the `Arc<InstallService>` before the `spawn_blocking` move).
- Add `pub mod updates;` next to `pub mod specials;` (the `commands/` submodule declarations live in `commands.rs`).

`lib.rs`:
- `mod app_update;` (Task 4 creates it — add the decl in Task 4, not here), `mod update_service;`.
- `AppState { …, updates: update_service::UpdateService::new() }`.
- `.plugin(tauri_plugin_opener::init())` on the builder (unconditional).
- In `setup`, first thing: set the title —

```rust
let version = app.package_info().version.to_string();
if let Some(window) = app.get_webview_window("main") {
    let _ = window.set_title(&format!("GRID Launcher {version}"));
}
```

- In the `set_game_finalized_hook` closure: clone `state.updates`, an `app.handle().clone()`, and the session/install Arcs, and call `updates.spawn_refresh(handle.clone(), session.clone(), install_for_game.clone())` after the firmware spawn (one hook, two effects).
- Register the four commands in `generate_handler!`.

`Cargo.toml` (app): add `tauri-plugin-opener = "2"`; set `version = "0.9.0-dev"`. `tauri.conf.json`: `"version": "0.9.0-dev"`.

`api.ts`:

```ts
export type UpdateRow = { rom_id: number; label: string };
/// Emitted after every update-set recompute (connect, install, uninstall) and on disconnect (empty).
export const UPDATES_CHANGED_EVENT = 'updates-changed';
// in `api`:
  listUpdates: () => invoke<UpdateRow[]>('list_updates'),
  updateGame: (romId: number) => invoke<void>('update_game', { romId }),
  appVersion: () => invoke<string>('app_version'),
  openReleasePage: (url: string) => invoke<void>('open_release_page', { url }),
```

- [ ] **Step 4: Build, test, commit**

```bash
cargo test -p app
cargo fmt && cargo clippy -p app --all-targets -- -D warnings
scripts/check_secret_hygiene.sh
(cd app && npm run check)
git add app/src-tauri/src/update_service.rs app/src-tauri/src/commands/updates.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml app/src-tauri/tauri.conf.json app/src/lib/api.ts Cargo.lock
git commit -m "rewrite: UpdateService, update commands, window title, opener plugin, version 0.9.0-dev"
```

`Cargo.lock` lives at `rewrite/Cargo.lock`; include it.

---

### Task 4: Check-only self-update

**Files:**
- Create: `app/src-tauri/src/app_update.rs`
- Modify: `app/src-tauri/src/lib.rs` (`mod app_update;`, spawn in `setup` after the title), `app/src-tauri/Cargo.toml` (`semver = "1"`), `app/src/lib/api.ts` (`APP_UPDATE_EVENT`, `AppUpdateNotice` type)

**Interfaces:**
- Consumes: `grid_core::launch::forge::ForgeClient::{new, get}` (public), `reqwest::Response::json`.
- Produces: `app_update::{is_newer, is_dev_build, should_check, spawn_check, APP_UPDATE_EVENT, AppUpdateNotice}`; frontend `APP_UPDATE_EVENT = 'app-update-available'`, `AppUpdateNotice = { tag: string; url: string }`.

- [ ] **Step 1: Write `app_update.rs` with tests**

```rust
//! Check-only launcher self-update (spec §5, doc 10 OQ14 ruling D-10-h):
//! one GitHub `releases/latest` request per process, a banner when the tag
//! is newer than the running version, nothing downloaded or installed.
//!
//! Goes through grid-core's `ForgeClient`: no RomM credential can reach the
//! forge, and the E2E build's `GRID_LAUNCHER_E2E_FORGE_BASE` redirect
//! applies. Every failure is silent at debug level, naming the host only.

use grid_core::launch::forge::ForgeClient;
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub const APP_UPDATE_EVENT: &str = "app-update-available";
pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/Sixdd6/grid-launcher/releases/latest";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppUpdateNotice {
    pub tag: String,
    pub url: String,
}

#[derive(Deserialize)]
struct LatestRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
}

/// Whether `tag` (with or without a leading `v`) is a newer semver than
/// `current`. Unparseable input on either side is "not newer".
pub fn is_newer(current: &str, tag: &str) -> bool {
    let tag = tag.trim();
    let tag = tag.strip_prefix('v').or_else(|| tag.strip_prefix('V')).unwrap_or(tag);
    match (Version::parse(current.trim()), Version::parse(tag)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}

/// A source build: the pre-release carries a `dev` identifier (`0.9.0-dev`).
pub fn is_dev_build(current: &str) -> bool {
    Version::parse(current.trim())
        .map(|v| v.pre.as_str().split('.').any(|id| id == "dev"))
        .unwrap_or(false)
}

/// The gate: dev builds never check, unless the `e2e` build is told to.
pub fn should_check(current: &str, e2e_forced: bool) -> bool {
    !is_dev_build(current) || e2e_forced
}

fn e2e_forced() -> bool {
    cfg!(feature = "e2e") && std::env::var("GRID_LAUNCHER_E2E_UPDATE_CHECK").is_ok_and(|v| v == "1")
}

/// Runs the check once, on Tauri's async runtime. Call from `setup`.
pub fn spawn_check(app: AppHandle) {
    let current = app.package_info().version.to_string();
    if !should_check(&current, e2e_forced()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Some(notice) = fetch_notice(&current).await {
            let _ = app.emit(APP_UPDATE_EVENT, notice);
        }
    });
}

async fn fetch_notice(current: &str) -> Option<AppUpdateNotice> {
    let client = ForgeClient::new().ok()?;
    let response = match client.get(LATEST_RELEASE_URL, true).await {
        Ok(response) => response,
        Err(_) => {
            tracing::debug!("self-update check: request to api.github.com failed");
            return None;
        }
    };
    let release: LatestRelease = match response.json().await {
        Ok(release) => release,
        Err(_) => {
            tracing::debug!("self-update check: release JSON did not decode");
            return None;
        }
    };
    if release.tag_name.is_empty() || release.html_url.is_empty() || !is_newer(current, &release.tag_name) {
        return None;
    }
    Some(AppUpdateNotice { tag: release.tag_name, url: release.html_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_compares_semver_with_prerelease_precedence() {
        assert!(is_newer("0.9.0", "v0.9.1"));
        assert!(is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "v0.9.0"));
        assert!(!is_newer("0.9.1", "v0.9.0"));
        assert!(is_newer("0.9.0-beta1", "v0.9.0"));
        assert!(!is_newer("0.9.0", "v0.9.0-beta1"));
        assert!(is_newer("0.9.0-dev", "v9.9.9-e2e"));
        assert!(is_newer("0.9.0-dev", "V0.9.0"));
    }

    #[test]
    fn garbage_is_never_newer() {
        assert!(!is_newer("0.9.0", "latest"));
        assert!(!is_newer("0.9.0", ""));
        assert!(!is_newer("not-a-version", "v1.0.0"));
    }

    #[test]
    fn dev_builds_are_recognised_and_gated() {
        assert!(is_dev_build("0.9.0-dev"));
        assert!(is_dev_build("0.9.0-dev.3"));
        assert!(!is_dev_build("0.9.0-beta4"));
        assert!(!is_dev_build("0.9.0"));
        assert!(!should_check("0.9.0-dev", false));
        assert!(should_check("0.9.0-dev", true));
        assert!(should_check("0.9.0", false));
    }
}
```

Also add a wiremock-based test in the same module that starts a `MockServer`, sets `GRID_LAUNCHER_E2E_FORGE_BASE` to its URI, mounts `GET /api.github.com/repos/Sixdd6/grid-launcher/releases/latest` returning `{"tag_name":"v9.9.9","html_url":"https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9"}` and asserts `fetch_notice("0.9.0").await == Some(...)`, plus a 404 mount asserting `None`. This only works when the crate is built with `--features e2e` (the redirect is feature-gated), so mark it `#[cfg(feature = "e2e")]` and run it with `cargo test -p app --features e2e app_update`. Set the env var inside the test with `std::env::set_var` and restore it afterwards; since other tests in the crate may read it, name the test so it is obvious and run it serially (`#[serial_test]` is not a dependency — instead keep it the only test in the crate that touches that variable).

- [ ] **Step 2: Wire**

`lib.rs`: `mod app_update;`; in `setup`, right after the title block: `app_update::spawn_check(app.handle().clone());`. `Cargo.toml`: `semver = "1"`. `api.ts`:

```ts
export type AppUpdateNotice = { tag: string; url: string };
/// Emitted at most once per process when a newer launcher release exists.
export const APP_UPDATE_EVENT = 'app-update-available';
```

- [ ] **Step 3: Test, lint, commit**

```bash
cargo test -p app app_update
cargo test -p app --features e2e app_update
cargo fmt && cargo clippy -p app --all-targets -- -D warnings && cargo clippy -p app --all-targets --features e2e -- -D warnings
git add app/src-tauri/src/app_update.rs app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml app/src/lib/api.ts Cargo.lock
git commit -m "rewrite: check-only self-update notice via the forge client"
```

---

### Task 5: Frontend stores and pure helpers

**Files:**
- Create: `app/src/lib/stores/updates.svelte.ts`, `app/src/lib/stores/updates.test.ts`, `app/src/lib/stores/appUpdate.svelte.ts`, `app/src/lib/details/version.ts`, `app/src/lib/details/version.test.ts`
- Modify: `app/src/App.svelte` (init both stores in the shell-phase effect)

**Interfaces:**
- Consumes: `api.listUpdates`, `UPDATES_CHANGED_EVENT`, `APP_UPDATE_EVENT`, `AppUpdateNotice`, `UpdateRow` (Tasks 3–4).
- Produces: `updates.rows`, `labelFor(rows, romId)` (pure) and `updates.labelFor(romId)`, `refresh()`, `init()`; `appUpdate.notice`, `appUpdate.dismissed`, `dismiss()`, `initAppUpdate()`; `parseVersionTag`, `formatVersionTag`, `versionLabel`.

- [ ] **Step 1: `details/version.ts` + test**

```ts
// Details version label (grid-launcher.py:3273-3297, doc 10 "Native game
// version detection"). A TS mirror of grid-core's update_detection tag
// rules — kept in sync by version.test.ts, which pins the same cases.

export type VersionTag = { kind: 'numeric'; value: number } | { kind: 'semver'; parts: number[] };

const NUMERIC = /\(v(\d{5})\)/i;
const SEMVER = /\(v(\d+(?:\.\d+)+)\)/i;

export function parseVersionTag(romFileName: string): VersionTag | null {
  const numeric = NUMERIC.exec(romFileName);
  if (numeric) return { kind: 'numeric', value: Number(numeric[1]) };
  const semver = SEMVER.exec(romFileName);
  if (!semver) return null;
  return { kind: 'semver', parts: semver[1].split('.').map(Number) };
}

export function formatVersionTag(tag: VersionTag): string {
  return tag.kind === 'numeric' ? `v${String(tag.value).padStart(5, '0')}` : `v${tag.parts.join('.')}`;
}

export function isWindowsPcPlatform(platform: string): boolean {
  const normalized = platform.trim().toLowerCase();
  return normalized !== '' && (normalized.includes('windows') || normalized === 'pc');
}

/**
 * `_details_version_label_text_for_game`: for a Windows/PC platform, the
 * first tag found in `romFileNames` (server fs_name first, then the
 * installed row's rom_file_name) renders as `Version: v…`; otherwise the
 * trimmed `revision` verbatim (no prefix — Python parity); `''` hides the row.
 */
export function versionLabel(platform: string, romFileNames: string[], revision: string): string {
  if (isWindowsPcPlatform(platform)) {
    for (const name of romFileNames) {
      const tag = parseVersionTag(name);
      if (tag) return `Version: ${formatVersionTag(tag)}`;
    }
  }
  return revision.trim();
}
```

Test cases (vitest): `(v00042)` → numeric 42 → `v00042`; real semver filename → `v3.6.0`; `(v1234)` and `My Game.zip` → null; numeric preferred over semver; `versionLabel('Windows', ['', 'g (v1.0.0).zip'], '')` → `Version: v1.0.0`; `versionLabel('PS2', ['g (v1.0.0).zip'], ' r2 ')` → `r2`; `versionLabel('Windows', ['g.zip'], '')` → `''`.

- [ ] **Step 2: `stores/updates.svelte.ts` + test**

```ts
// Server-update set (doc 10). Mirrors compatTools.svelte.ts: a `$state`
// snapshot behind getters, `refresh()` via the command, `init()` = refresh
// then listen. The event payload IS the new row list, so the listener
// applies it directly instead of re-fetching.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, UPDATES_CHANGED_EVENT, type UpdateRow } from '../api';

const state = $state<{ rows: UpdateRow[] }>({ rows: [] });

/** Pure, exported for tests: the button label for `romId`, or null when it has no update. */
export function labelFor(rows: UpdateRow[], romId: number | null): string | null {
  if (romId === null) return null;
  return rows.find((r) => r.rom_id === romId)?.label ?? null;
}

export const updates = {
  get rows() {
    return state.rows;
  },
  labelFor(romId: number | null): string | null {
    return labelFor(state.rows, romId);
  },
};

export async function refresh(): Promise<void> {
  state.rows = await api.listUpdates();
}

export async function init(): Promise<UnlistenFn> {
  await refresh().catch(() => {});
  return listen<UpdateRow[]>(UPDATES_CHANGED_EVENT, (e) => {
    state.rows = e.payload;
  });
}
```

Test `labelFor`: found → label; missing → null; `null` rom id → null.

- [ ] **Step 3: `stores/appUpdate.svelte.ts`**

```ts
// The self-update banner's state. Module-scoped so a dismissal survives
// Shell remounts for the rest of the process.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { APP_UPDATE_EVENT, type AppUpdateNotice } from '../api';

const state = $state<{ notice: AppUpdateNotice | null; dismissed: boolean }>({ notice: null, dismissed: false });

export const appUpdate = {
  get notice() {
    return state.dismissed ? null : state.notice;
  },
};

export function dismiss(): void {
  state.dismissed = true;
}

export function initAppUpdate(): Promise<UnlistenFn> {
  return listen<AppUpdateNotice>(APP_UPDATE_EVENT, (e) => {
    state.notice = e.payload;
  });
}
```

- [ ] **Step 4: `App.svelte`**

Register `initAppUpdate()` in the same early effect as `initReplenishListener` (the event can fire before the shell mounts), and `initUpdates()` (import `init as initUpdates`) next to `initCompatTools` in the shell-phase effect, with matching unlisten cleanup.

- [ ] **Step 5: Test, check, commit**

```bash
(cd app && npm test && npm run check)
git add app/src/lib/stores/updates.svelte.ts app/src/lib/stores/updates.test.ts app/src/lib/stores/appUpdate.svelte.ts app/src/lib/details/version.ts app/src/lib/details/version.test.ts app/src/App.svelte
git commit -m "rewrite: updates/appUpdate stores and the version-label helper"
```

---

### Task 6: Frontend surfacing (designer)

**Files:**
- Modify: `app/src/lib/Library.svelte`, `app/src/lib/Details.svelte`, `app/src/lib/Shell.svelte`

**Interfaces:**
- Consumes: `updates.labelFor`, `appUpdate.notice`, `dismiss`, `versionLabel`, `api.updateGame`, `api.openReleasePage`, `isNativePlatform` (`details/actions.ts`), `installedRow` (already derived in Details), `detail` (`RomDetail | null`, already fetched in Details), `downloads.entries`.
- Produces: the test ids below — Task 7 asserts on them verbatim.

- [ ] **Step 1: Library badge**

Inside the card `.caption`, after the platform span:

```svelte
{#if updates.labelFor(row.rom_id) !== null}
  <span data-testid={`library-update-badge-${row.rom_id}`} class="update-badge">Update Available</span>
{/if}
```

Style: small, warning-toned (the theme's existing accent for warnings), 600 weight, 12px — the Python indicator is `color: warning; font-size: 12px; font-weight: 600`.

- [ ] **Step 2: Details — version row, Update button, confirm, toast**

Script additions:

```ts
import { updates } from './stores/updates.svelte';
import { versionLabel } from './details/version';

let updateLabel = $derived(installedNow ? updates.labelFor(subject.romId) : null);
let version = $derived(
  versionLabel(subject.platformName, [detail?.fs_name ?? '', installedRow?.rom_file_name ?? ''], detail?.revision || installedRow?.revision || '')
);
let confirmingUpdate = $state(false);
let updateToast = $state<string | null>(null);
let updatePending = $state(false);

async function handleUpdateClick() {
  if (subject.romId === null) return;
  if (isNativeInstall && !confirmingUpdate) {
    confirmingUpdate = true;
    return;
  }
  error = null;
  updatePending = true;
  try {
    await api.updateGame(subject.romId);
  } catch (err) {
    error = errorMessage(err);
  } finally {
    updatePending = false;
    confirmingUpdate = false;
  }
}
```

Toast effect: track the previous status of drawer entries for this rom whose `kind` is `'update'` or `'native_update'`; when one transitions into `'completed'`, set `updateToast = \`Updated '${subject.name}' successfully.\``. Reuse the `wasLive` pattern already in the file (a `$effect` over `downloads.entries`).

Markup, in the `installedNow` branch after the Uninstall button:

```svelte
{#if updateLabel !== null}
  <button
    data-testid="details-update"
    class:confirm={confirmingUpdate}
    disabled={pending || updatePending || liveEntry !== undefined}
    onclick={handleUpdateClick}
  >
    {updatePending ? 'Updating…' : confirmingUpdate ? 'Saves and configuration will be preserved — confirm update' : updateLabel}
  </button>
{/if}
```

Version row: in the metadata column next to `details-rating`:

```svelte
{#if version}
  <p data-testid="details-version" class="version">{version}</p>
{/if}
```

Toast: below the action row, `{#if updateToast}<p data-testid="details-update-toast" class="hint">{updateToast}</p>{/if}`.

Note the `liveEntry` derivation matches ANY live entry for this rom, so the Update button hides behind `Installing…`/`Cancel` while the update runs — that is the intended state (doc 10: disabled while an install is in flight).

- [ ] **Step 3: Shell banner**

Import `appUpdate`, `dismiss` and `api`. Directly under the top bar:

```svelte
{#if appUpdate.notice}
  <div data-testid="app-update-banner" class="update-banner">
    <span>GRID Launcher {appUpdate.notice.tag} is available</span>
    <button data-testid="app-update-open" onclick={() => api.openReleasePage(appUpdate.notice!.url).catch(() => {})}>Open release</button>
    <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
  </div>
{/if}
```

Style: a single-line strip in the theme's accent, buttons inline, no layout shift for the sections below beyond the strip's own height.

- [ ] **Step 4: Check and commit**

```bash
(cd app && npm run check && npm test)
git add app/src/lib/Library.svelte app/src/lib/Details.svelte app/src/lib/Shell.svelte
git commit -m "rewrite: Update Available badge, Details Update button/version row/toast, self-update banner"
```

---

### Task 7: E2E group `updates`

**Files:**
- Create: `e2e/fixtures-updates/{platforms,roms,rom-details}.json`, `e2e/seed/updates-seed.mjs`, `e2e/specs/updates.spec.ts`
- Modify: `e2e/mock-romm/server.mjs` (`contentForFile`), `e2e/mock-romm/mock-forge.mjs` (+ `mock-forge.test.mjs`), `e2e/wdio.conf.ts`, `scripts/e2e.sh`

**Interfaces:**
- Consumes: test ids from Task 6 (`library-card-<id>`, `library-update-badge-<id>`, `details-update`, `details-version`, `details-update-toast`, `app-update-banner`, `app-update-open`, `app-update-dismiss`), drawer test ids used by `native.spec.ts`, `GRID_LAUNCHER_E2E_UPDATE_CHECK` (Task 4).
- Produces: stage group `updates`.

- [ ] **Step 1: Fixtures**

`platforms.json`: `[{ "id": 1, "name": "SNES", "slug": "snes", "rom_count": 2 }, { "id": 2, "name": "Windows", "slug": "win", "rom_count": 1 }]`.

`roms.json`: `"1": [801 "Old Rom", 803 "Current Rom"]`, `"2": [802 "My Game"]` in the shape of `fixtures-native/roms.json`.

`rom-details.json` (shape of `fixtures-native/rom-details.json`):
- 801: platform_display_name `SNES`, `fs_name` `newrom.zip`, `updated_at` `2026-06-01T00:00:00Z`, files `[ {id 4801, "newrom.zip"} ]`.
- 802: platform_display_name `Windows`, `fs_name` `mygame (v1.1.0).zip`, `updated_at` `2026-01-01T00:00:00Z`, files `[ {id 4802, "mygame (v1.1.0).zip"}, {id 4803, "game.json"} ]`.
- 803: platform_display_name `SNES`, `fs_name` `current.zip`, `updated_at` `2026-01-01T00:00:00Z`, files `[ {id 4804, "current.zip"} ]`.
- No entry for 804 (the mock answers 404 for unknown ids — verify with `grep -n "404" e2e/mock-romm/server.mjs`).

`server.mjs` `contentForFile`: add `if (lower === "mygame (v1.1.0).zip") return content.nativeZipBytes;` above the generic `.zip` line, so the native update archive carries `MyGame/mygame.exe`.

- [ ] **Step 2: Seed**

`updates-seed.mjs`, modelled on `native-seed.mjs` + `launch-seed.mjs`'s `INSERT`: library path, `default_compat_tool = "wine"` (not launched, harmless), registry via `writeRegistry(dbPath, extraSql)` with four rows:

| rom_id | title | platform | rom_file_name | server_updated_at | on disk |
|---|---|---|---|---|---|
| 801 | Old Rom | SNES | `oldrom.zip` | `2025-01-01T00:00:00Z` | `extracted_dir` = `<lib>/SNES/oldrom` with `old.sfc`; `extracted_path` = that file |
| 802 | My Game | Windows | `mygame (v1.0.0).zip` | `2026-01-01T00:00:00Z` | `native_game_dir` = `<lib>/Windows/My Game`; `extracted_dir` = `<lib>/Windows/My Game/game` holding `MyGame/mygame.exe` (8 bytes) and `saves/slot1.sav` (content `SAVE1`); `extracted_path` = the exe |
| 803 | Current Rom | SNES | `current.zip` | `2026-01-01T00:00:00Z` | `extracted_dir` = `<lib>/SNES/current` with `game.sfc` |
| 804 | Ghost Rom | SNES | `ghost.zip` | `2025-01-01T00:00:00Z` | `extracted_dir` = `<lib>/SNES/ghost` with `game.sfc` |

`title_key`/`platform_key` lowercased; `installed_at` = now.

- [ ] **Step 3: Mock forge**

Add exports `GRID_LAUNCHER_TAG = "v9.9.9-e2e"`, `GRID_LAUNCHER_RELEASE_PATH = "/api.github.com/repos/Sixdd6/grid-launcher/releases/latest"`, `GRID_LAUNCHER_RELEASE_URL = "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9-e2e"`, a `gridLauncherRelease()` returning `{ tag_name, html_url, assets: [] }`, and a `GET` branch in `handleRequest` before the 404. Add a case to `mock-forge.test.mjs` asserting the JSON.

- [ ] **Step 4: Runner wiring**

`scripts/e2e.sh`: `"updates:specs/updates.spec.ts"` in `STAGE_GROUPS`; `updates) printf -- '--fixtures-dir fixtures-updates' ;;` in `mock_args_for_group`; `updates) printf '%s' "$E2E_DIR/seed/updates-seed.mjs" ;;` in `seed_script_for_group`; `group_needs_forge` becomes `[[ "$1" == "emulator-catalog" || "$1" == "updates" ]]`.

`e2e/wdio.conf.ts`: in the `E2E_FORGE_URL` spread add `GRID_LAUNCHER_E2E_UPDATE_CHECK: '1'`.

- [ ] **Step 5: Spec**

`updates.spec.ts`, using the helpers and connect preamble from `native.spec.ts`:

1. Connect; click `nav-library`; wait for `library-card-801`.
2. `library-update-badge-801` and `-802` exist; `-803` and `-804` do not (`waitForExist({ reverse: true })` after the two positives exist).
3. Self-update: `app-update-banner` exists and contains `v9.9.9-e2e`; click `app-update-dismiss`; banner gone. (`app-update-open` is NOT clicked — it would spawn a browser.)
4. Open Details on 801: `details-update` text is `Update`; no `details-version`. Click; open the drawer; the newest row shows the `Update` kind badge; wait for it to complete (`INSTALL_TIMEOUT`); `details-update-toast` reads `Updated 'Old Rom' successfully.`; a file exists under `<lib>/SNES/newrom/`; badge 801 gone.
5. Open Details on 802: `details-version` reads `Version: v1.0.0`; `details-update` reads `Update to v1.1.0`; click once → label `Saves and configuration will be preserved — confirm update`; click again; wait for the drawer row to complete; `<lib>/Windows/My Game/game/saves/slot1.sav` still reads `SAVE1`; `MyGame/mygame.exe` exists; `details-version` now reads `Version: v1.1.0`; badge 802 gone.
6. Open Details on 804: no `details-update`.

Use `existsSync`/`readFileSync` from `node:fs` for the on-disk assertions, as `native.spec.ts` does.

- [ ] **Step 6: Run and commit**

```bash
(cd e2e && node --test mock-romm/mock-forge.test.mjs)
scripts/e2e.sh updates
git add e2e/fixtures-updates e2e/seed/updates-seed.mjs e2e/specs/updates.spec.ts e2e/mock-romm/server.mjs e2e/mock-romm/mock-forge.mjs e2e/mock-romm/mock-forge.test.mjs e2e/wdio.conf.ts scripts/e2e.sh
git commit -m "rewrite: E2E group updates — badges, Update flows, self-update banner"
```

Do not set `E2E_SKIP_BUILD` (the stamp ignores sources; see memory `sdd-harness-notes`).

---

### Task 8: Docs

**Files:**
- Modify: `docs/porting/10-identity-updates.md` (new section "Rust port deviations (milestone 9)" after the milestone 7 section), `docs/porting/03-library-install.md` (the install-mode table in the milestone 8 deviations + one bullet), `README.md` (E2E table row `updates`; "Milestone 9 manual checklist")

- [ ] **Step 1: Doc 10** — record D-10-b … D-10-j exactly as spec §7 states them, each with the Rust anchor (`crates/grid-core/src/library/update_detection.rs`, `app/src-tauri/src/update_service.rs`, `app/src-tauri/src/app_update.rs`, `app/src/lib/details/version.ts`), plus a short "What the Rust port checks and when" paragraph (triggers, bounded concurrency, event).
- [ ] **Step 2: Doc 03** — add `Update` (`kind` `update`) to the mode list with one line: "plain re-install of an installed non-native rom; bypasses the already-installed short-circuit; finalizes as Base (D-10-j: no pre-clean)".
- [ ] **Step 3: README** — E2E row: `updates | seeded rows + fixtures-updates + mock forge | badges, non-native Update, native merge preserving saves, absent server entry, self-update banner`; manual checklist: window title shows the version; an installed game updated on the server shows the badge after reconnect; Update on a Windows game keeps saves; the banner appears on a release build only.
- [ ] **Step 4: Commit**

```bash
git add docs/porting/10-identity-updates.md docs/porting/03-library-install.md README.md
git commit -m "rewrite: milestone 9 deviations in docs 10/03; README updates row and checklist"
```

---

## Self-review notes

- **Spec coverage:** §2 → T1; §3 → T2; §4 → T3; §5 → T4; §6 → T5+T6; §7 → T8; §8 → T1/T2/T3/T4/T5/T7 tests and the gate.
- **Type consistency:** `ServerVersion { platform, rom_file_name, updated_at }` is used identically in T1, T3; `UpdateRow { rom_id, label }` in T3 (Rust, serde snake_case matches the TS type) and T5; `labelFor(rows, romId)` pure + `updates.labelFor(romId)` method in T5, consumed in T6; `kind` `"update"` in T2 (Rust `kind()`), T2 (`DownloadKind`), T6 (toast filter), T7 (drawer badge `Update`).
- **Known judgment points for the executor:** (a) the exact `NOT_INSTALLED` text and its visibility; (b) whether `Registry::all` is the accessor name; (c) the vitest file that owns `kindLabel` tests; (d) the drawer row test ids in `native.spec.ts` to reuse in T7. Each is a one-line `grep` away and is called out in its task.
