# Desktop UI redesign 1 — shell and theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the modal-heavy shell with a five-view frame — top bar, pill navigation, RomM v2 theme tokens, a download footer strip, blurred background art, and Settings › Appearance — with today's Library, Server, Emulators and Downloads content moved inside it unchanged.

**Architecture:** `Shell.svelte` becomes the only navigator: five view roots stay mounted and switch with the `hidden` attribute, exactly as Library/Server do today. Emulators stops being a modal and becomes a view (its content is untouched); the downloads drawer becomes a view plus a 28px status strip. Theme tokens live once in `app.css` and are selected by `prefers-color-scheme` unless a `data-theme` attribute — written from the new `ui.theme` config field — overrides them. Two new Tauri commands (`get_ui_settings`, `set_ui_settings`) and one opener (`open_server_page`) are the only backend additions.

**Tech Stack:** Rust (grid-core `config`, Tauri 2 `app` crate), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — binding. This plan implements **delivery item 1 only** (§12.1): §3 Shell, §4 Theme tokens, §10 Appearance page only, §11 test-id renames. Plans 2–5 (Library/Server rails and grids, the Details popup layout, the Downloads segments and sparklines, the Emulators/Settings rails) are explicitly NOT implemented here; this plan only scaffolds the roots they slot into.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Global Constraints

- **Token secrecy (hard):** tokens only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output.
- **Theme tokens, spec §4, verbatim.** Dark: `--bg #07070f`, `--surface rgba(255,255,255,.07)`, `--surface-2 #14141f`, `--border #22223a`, `--text #ffffff`, `--text-muted #9a9ab0`. Light: `--bg #f5f5fa`, `--surface rgba(0,0,0,.035)`, `--text #111117`, primary darkens to `#553E98`. Shared: primary `#8B74E8` (hover `#A18FFF`, pressed `#6043C8`), secondary `#9E8CD6`, accent `#E1A38D`, favourite `#FF4F6B`, success `#4ADE80`, warning `#FBBF24`, danger `#FF5050`, info `#93C5FD`, disk-graph teal `#2dd4bf`. Type: Segoe UI / system-ui / Inter; base 13px; titles 15–20px semibold. Spacing on a 4px scale; radii 4 (controls) / 6 (chips) / 8 (rows) / 14 (cards) / 100 (pills). Motion 150 / 220 / 360ms.
- **CSS variables are defined once, in `app/src/app.css`.** Dark is the default under `prefers-color-scheme: dark` and whenever `ui.theme = "dark"`; light applies under the light scheme or `ui.theme = "light"`. The `data-theme` attribute on `<html>` carries the override.
- **Top bar 58px.** Pill tabs in order Library, Server, Downloads, Emulators, Settings, with test ids `nav-library`, `nav-server`, `nav-downloads`, `nav-emulators`, `nav-settings`. Keyboard `Ctrl+1..5` switches views.
- **Views stay mounted and switch with the `hidden` attribute.**
- **Footer strip 28px**, test id `downloads-footer` kept, hidden when nothing is live, shows `⬇ <title> · <percent> · <speed>` and an "Open Downloads" link. The sparkline is plan 4 — leave a slot.
- **Background art:** last-viewed game cover (details opened, or a card hovered for more than 500ms), falling back to the most recently played installed game on startup; blur 40px; opacity from `ui.background_fade` (0–60, default 25); cross-fade 360ms; test id `background-art`.
- **Settings view root `settings-view`** with rail entries `settings-nav-<page>`. Only the Appearance page is built in this plan (`theme-select` with system/dark/light, `background-fade` range input with live preview). Every other rail entry renders the placeholder line `Coming in a later step` — that exact text.
- **The app-update banner is removed**, replaced by a badge `app-update-badge` on the server menu and an "Updates" entry in Settings that shows the stored notice tag with the existing `app-update-open` link.
- **Test-id renames (spec §11):** `emulators-open` → `nav-emulators`; `emulators-panel` → the Emulators view root; `emulators-close` removed; `downloads-drawer` → the Downloads view root. **Specs are updated in the same task that renames an id.**
- **Every task ends with** `cargo fmt` (from `rewrite/`), `cargo clippy --workspace --all-targets -- -D warnings` clean (from `rewrite/`), `npm run check` and `npx vitest run` green (from `rewrite/app`), and a commit whose subject starts `rewrite: `. The final E2E task runs **every** group (`scripts/e2e.sh` with no argument) and must be green.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **Test commands:** `cargo test -p grid-core` and `cargo test -p app` from `rewrite/`; `npm run check` and `npx vitest run` from `rewrite/app`; `scripts/e2e.sh <group>` from `rewrite/`.
- **No component test harness exists** in this repo (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted, unit-tested pure module plus `npm run check` and E2E — never by a fabricated component test.

---

## File map

| File | Responsibility |
|---|---|
| `crates/grid-core/src/config.rs` | `UiSettings { theme, background_fade }` + `Config.ui` with serde defaults |
| `app/src-tauri/src/commands.rs` | `get_ui_settings`, `set_ui_settings`, `open_server_page`, and the pure `normalize_ui_settings` / `browsable_server_url` helpers |
| `app/src-tauri/src/lib.rs` | handler registration for the three new commands |
| `app/src/lib/api.ts` | `UiSettings` type, `getUiSettings`, `setUiSettings`, `openServerPage` |
| `app/src/app.css` | the whole §4 token set, both schemes, `data-theme` override, `.view-content` width cap |
| `app/src/lib/theme.ts` (+ `theme.test.ts`) | pure theme/fade resolution: `normalizeTheme`, `resolveTheme`, `themeAttribute`, `clampFade` |
| `app/src/lib/stores/uiSettings.svelte.ts` | loads/saves `ui.*`, applies `data-theme`, follows `prefers-color-scheme` |
| `app/src/lib/shell.ts` (+ `shell.test.ts`) | `VIEWS`, `View`, `initialView`, `viewLabel`, `viewForDigit` |
| `app/src/lib/Shell.svelte` | top bar, pill nav, `Ctrl+1..5`, five view roots, server menu, `app-update-badge` |
| `app/src/lib/Emulators.svelte` | becomes a plain view: no backdrop, no close button, no Escape; gains an `active` prop |
| `app/src/lib/Downloads.svelte` | the list only, rendered inside `downloads-view` |
| `app/src/lib/DownloadsFooter.svelte` | the 28px strip, `downloads-footer`, "Open Downloads" |
| `app/src/lib/downloads/format.ts` (+ `format.test.ts`) | `footerLine` |
| `app/src/lib/background.ts` (+ `background.test.ts`) | `startupCover` |
| `app/src/lib/stores/lastViewed.svelte.ts` | the cover the background art shows |
| `app/src/lib/BackgroundArt.svelte` | two-layer blurred cross-fading backdrop, `background-art` |
| `app/src/lib/settings.ts` (+ `settings.test.ts`) | `SETTINGS_PAGES`, `SettingsPage`, `settingsPageLabel` |
| `app/src/lib/Settings.svelte` | `settings-view`, the rail, the Appearance page, the Updates entry |
| `app/src/lib/Library.svelte`, `app/src/lib/Server.svelte` | hover/open hooks that feed `lastViewed` (content otherwise unchanged) |
| `app/src/App.svelte` | `initUiSettings` wiring |
| `e2e/specs/*.spec.ts` | id renames, per §11, in the task that renames them |
| `SPEC.md`, `rewrite/README.md` | desktop UI section + manual checklist rows |

---

### Task 1: `Config.ui` and the three new Tauri commands

**Files:**
- Modify: `crates/grid-core/src/config.rs:147-151` (add the field before `extra`), `:165-188` (the `Default` impl), `:248` (the test module)
- Modify: `app/src-tauri/src/commands.rs:306-325` (add the new commands after `set_library_path`), `:1803` (append tests to `mod retroachievements_tests`'s file tail as a NEW module)
- Modify: `app/src-tauri/src/lib.rs:265-325` (the `generate_handler!` list)
- Modify: `app/src/lib/api.ts:14` (types) and `:309-311` (wrappers)

**Interfaces:**
- Consumes: `grid_core::config::Config` and `Config::default_path()`; `crate::config_write::modify_config(path, |config| ...)` (`app/src-tauri/src/config_write.rs:56`); `crate::commands::err`.
- Produces, used by Tasks 2, 5 and 6:
  - `pub struct grid_core::config::UiSettings { pub theme: String, pub background_fade: u8 }` (Serialize + Deserialize + Clone + PartialEq + Debug), `Default` = `{ theme: "system", background_fade: 25 }`.
  - `Config.ui: UiSettings`.
  - `pub fn normalize_ui_settings(settings: UiSettings) -> UiSettings` in `app/src-tauri/src/commands.rs`.
  - `pub fn browsable_server_url(raw: &str) -> Option<String>` in `app/src-tauri/src/commands.rs`.
  - Tauri commands `get_ui_settings() -> UiSettings`, `set_ui_settings(settings: UiSettings)`, `open_server_page()`.
  - TS: `export type UiSettings = { theme: 'system' | 'dark' | 'light'; background_fade: number }`, `api.getUiSettings()`, `api.setUiSettings(settings)`, `api.openServerPage()`.

- [ ] **Step 1: Write the failing Rust tests**

Append to the existing `#[cfg(test)] mod tests` block in `crates/grid-core/src/config.rs` (it starts at line 248 and already has `use super::*;`):

```rust
    #[test]
    fn ui_settings_default_to_system_and_a_25_percent_fade() {
        let ui = UiSettings::default();
        assert_eq!(ui.theme, "system");
        assert_eq!(ui.background_fade, 25);
        assert_eq!(Config::default().ui, ui);
    }

    #[test]
    fn a_config_written_before_the_ui_table_existed_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "schema_version = 1\nserver_url = \"https://romm.example\"\n",
        )
        .unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "system");
        assert_eq!(loaded.ui.background_fade, 25);
    }

    #[test]
    fn ui_settings_round_trip_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            ui: UiSettings {
                theme: "dark".to_string(),
                background_fade: 60,
            },
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("[ui]"), "written config:\n{written}");
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "dark");
        assert_eq!(loaded.ui.background_fade, 60);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run from `rewrite/`: `cargo test -p grid-core config::tests::ui_settings config::tests::a_config_written_before`
Expected: FAIL to compile — `cannot find type 'UiSettings' in this scope`.

- [ ] **Step 3: Add `UiSettings` and `Config.ui`**

In `crates/grid-core/src/config.rs`, add this type directly after the `CompatToolInstall` struct (which ends at line 63, just before `#[derive(Debug, thiserror::Error)] pub enum ConfigError`):

```rust
/// Desktop-shell appearance settings (design §4, §10 Appearance). Both
/// fields default so a config written before this table existed loads
/// unchanged, and `Config::save` emits `[ui]` after every scalar key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    /// `"system"` (follow `prefers-color-scheme`), `"dark"` or `"light"`.
    /// Stored as a plain string rather than an enum so an unknown value
    /// written by a newer build round-trips instead of failing the whole
    /// config load; the app layer normalizes on write and the frontend
    /// normalizes on read.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Background-art opacity in percent, 0–60 (design §3). Clamped on
    /// write by `normalize_ui_settings`; read sites clamp again.
    #[serde(default = "default_background_fade")]
    pub background_fade: u8,
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_background_fade() -> u8 {
    25
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            background_fade: default_background_fade(),
        }
    }
}
```

Then add the field to `Config`, directly after `pub compat_tool_installs: Vec<CompatToolInstall>,` (line 147) and BEFORE the flattened `extra` map:

```rust
    /// Desktop shell appearance. A TOML table, so it must stay after every
    /// scalar key in this struct.
    #[serde(default)]
    pub ui: UiSettings,
```

and to the `Default` impl, after `compat_tool_installs: Vec::new(),` (line 185):

```rust
            ui: UiSettings::default(),
```

- [ ] **Step 4: Run the grid-core tests**

Run from `rewrite/`: `cargo test -p grid-core config::`
Expected: PASS, including the three new tests and every pre-existing `config::tests` case.

- [ ] **Step 5: Write the failing app-layer tests**

Append a NEW test module at the very end of `app/src-tauri/src/commands.rs` (after the closing brace of `mod retroachievements_tests`):

```rust
#[cfg(test)]
mod ui_settings_tests {
    use super::*;

    #[test]
    fn an_unknown_theme_normalizes_to_system() {
        for raw in ["", "SYSTEM", "solarized", "  dark  "] {
            let out = normalize_ui_settings(UiSettings {
                theme: raw.to_string(),
                background_fade: 25,
            });
            let expected = if raw.trim() == "dark" { "dark" } else { "system" };
            assert_eq!(out.theme, expected, "input {raw:?}");
        }
    }

    #[test]
    fn the_three_known_themes_are_stored_verbatim() {
        for raw in ["system", "dark", "light"] {
            let out = normalize_ui_settings(UiSettings {
                theme: raw.to_string(),
                background_fade: 0,
            });
            assert_eq!(out.theme, raw);
        }
    }

    #[test]
    fn the_fade_is_clamped_to_the_designs_zero_to_sixty() {
        let fade = |value: u8| {
            normalize_ui_settings(UiSettings {
                theme: "system".to_string(),
                background_fade: value,
            })
            .background_fade
        };
        assert_eq!(fade(0), 0);
        assert_eq!(fade(25), 25);
        assert_eq!(fade(60), 60);
        assert_eq!(fade(61), 60);
        assert_eq!(fade(255), 60);
    }

    #[test]
    fn a_server_url_carrying_userinfo_is_never_handed_to_the_os_opener() {
        // Basic-auth mode puts the password in the URL the user typed. It
        // must never reach a browser command line, a shell history, or a
        // desktop portal log.
        assert_eq!(browsable_server_url("http://user:pw@romm.example/romm"), None);
        assert_eq!(browsable_server_url("https://user@romm.example"), None);
    }

    #[test]
    fn only_plain_http_and_https_urls_are_browsable() {
        assert_eq!(
            browsable_server_url("https://romm.example:8080/romm"),
            Some("https://romm.example:8080/romm".to_string())
        );
        assert_eq!(
            browsable_server_url("  http://192.168.1.5:8000  "),
            Some("http://192.168.1.5:8000".to_string())
        );
        assert_eq!(browsable_server_url(""), None);
        assert_eq!(browsable_server_url("romm.example"), None);
        assert_eq!(browsable_server_url("file:///etc/passwd"), None);
        assert_eq!(browsable_server_url("javascript:alert(1)"), None);
    }
}
```

- [ ] **Step 6: Run them to verify they fail**

Run from `rewrite/`: `cargo test -p app ui_settings_tests`
Expected: FAIL to compile — `cannot find function 'normalize_ui_settings' in this scope`.

- [ ] **Step 7: Add the helpers and the three commands**

In `app/src-tauri/src/commands.rs`, extend the grid-core config import at line 8 from

```rust
use grid_core::config::{Config, EmulatorEntry};
```

to

```rust
use grid_core::config::{Config, EmulatorEntry, UiSettings};
```

and add this block directly after `set_library_path` (which ends at line 325):

```rust
// --- desktop shell appearance (design §4, §10) --------------------------------

/// The highest background-art opacity the Appearance slider offers
/// (design §3: "0–60%").
const MAX_BACKGROUND_FADE: u8 = 60;

/// What actually gets written to `config.toml` for a set of appearance
/// settings: an unrecognized theme falls back to `"system"` (rather than
/// being rejected, which would make a stale frontend unable to save
/// anything), and the fade is clamped into the design's range.
pub fn normalize_ui_settings(settings: UiSettings) -> UiSettings {
    let theme = match settings.theme.trim() {
        "dark" => "dark",
        "light" => "light",
        _ => "system",
    };
    UiSettings {
        theme: theme.to_string(),
        background_fade: settings.background_fade.min(MAX_BACKGROUND_FADE),
    }
}

/// The stored server URL, when it is safe to hand to the OS opener.
///
/// `None` for anything that is not a plain `http`/`https` URL, and — the
/// reason this function exists — `None` for any URL carrying userinfo:
/// basic-auth mode lets the user type `http://user:password@host/romm`,
/// and a password must never leave the keyring for a browser command
/// line. Deliberately hand-rolled rather than pulled from a URL crate:
/// the check is a prefix test plus an `@` scan of the authority, and this
/// crate has no URL dependency.
pub fn browsable_server_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(trimmed.to_string())
}

#[tauri::command]
pub async fn get_ui_settings() -> Result<UiSettings, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.ui)
    })
    .await
    .map_err(|e| format!("get_ui_settings did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_ui_settings(settings: UiSettings) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            config.ui = normalize_ui_settings(settings);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_ui_settings did not finish: {e}"))?
}

/// Opens the configured RomM server in the user's browser (design §3, the
/// server menu). Takes NO url argument on purpose: the frontend cannot
/// choose what gets opened, and the stored URL is filtered by
/// [`browsable_server_url`] before it reaches the opener.
#[tauri::command]
pub async fn open_server_page(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let url = tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok::<Option<String>, String>(browsable_server_url(&config.server_url))
    })
    .await
    .map_err(|e| format!("open_server_page did not finish: {e}"))??
    .ok_or("no server URL to open")?;
    app.opener().open_url(url, None::<&str>).map_err(err)
}
```

Note the `OpenerExt` import style: it mirrors `open_release_page` in `app/src-tauri/src/commands/updates.rs:97`. If that file imports the trait at module level instead, follow the same shape there rather than the local `use`.

- [ ] **Step 8: Register the commands**

In `app/src-tauri/src/lib.rs`, inside `tauri::generate_handler![...]`, add these three lines directly after `commands::set_library_path,` (line 283):

```rust
            commands::get_ui_settings,
            commands::set_ui_settings,
            commands::open_server_page,
```

- [ ] **Step 9: Run the Rust suite**

Run from `rewrite/`: `cargo test -p grid-core && cargo test -p app`
Expected: PASS, including `ui_settings_tests`.

- [ ] **Step 10: Add the TypeScript wrappers**

In `app/src/lib/api.ts`, add the type after the `PlatformRef` type (line 14):

```ts
/** Desktop shell appearance, mirroring `grid_core::config::UiSettings`. */
export type UiSettings = { theme: 'system' | 'dark' | 'light'; background_fade: number };
```

and the three wrappers directly after `setLibraryPath` (line 310):

```ts
  getUiSettings: () => invoke<UiSettings>('get_ui_settings'),
  setUiSettings: (settings: UiSettings) => invoke<void>('set_ui_settings', { settings }),
  /** Opens the configured RomM server in the browser. The URL comes from the
   *  backend's own config read — never from the frontend. */
  openServerPage: () => invoke<void>('open_server_page'),
```

- [ ] **Step 11: Run the frontend gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS, no new svelte-check or tsc errors.

- [ ] **Step 12: Format, lint, commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add crates/grid-core/src/config.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts
git commit -m "rewrite: add ui.theme/ui.background_fade config and their commands"
```

---

### Task 2: Theme tokens and `data-theme` resolution

**Files:**
- Rewrite: `app/src/app.css` (all 60 lines)
- Create: `app/src/lib/theme.ts`, `app/src/lib/theme.test.ts`
- Create: `app/src/lib/stores/uiSettings.svelte.ts`
- Modify: `app/src/App.svelte:1-45` (register the store's init effect)

**Interfaces:**
- Consumes: `api.getUiSettings()`, `api.setUiSettings(settings)`, `type UiSettings` from Task 1.
- Produces, used by Tasks 3, 5 and 6:
  - `app/src/lib/theme.ts`: `export type ThemeChoice = 'system' | 'dark' | 'light'`, `export type ResolvedTheme = 'dark' | 'light'`, `export const FADE_DEFAULT = 25`, `export const FADE_MAX = 60`, `export function normalizeTheme(raw: string): ThemeChoice`, `export function resolveTheme(choice: ThemeChoice, prefersDark: boolean): ResolvedTheme`, `export function themeAttribute(choice: ThemeChoice): ResolvedTheme | null`, `export function clampFade(value: number): number`.
  - `app/src/lib/stores/uiSettings.svelte.ts`: `export const uiSettings` with getters `theme: ThemeChoice`, `backgroundFade: number`, `resolved: ResolvedTheme`; `export function initUiSettings(): Promise<() => void>`; `export async function setTheme(choice: ThemeChoice): Promise<void>`; `export function previewBackgroundFade(value: number): void`; `export async function commitBackgroundFade(value: number): Promise<void>`.
  - CSS variables available to every component: `--bg`, `--surface`, `--surface-2`, `--border`, `--text`, `--text-h`, `--text-muted`, `--primary`, `--primary-hover`, `--primary-pressed`, `--secondary`, `--accent`, `--accent-warm`, `--favourite`, `--success`, `--warning`, `--danger`, `--info`, `--graph-disk`, `--r-control`, `--r-chip`, `--r-row`, `--r-card`, `--r-pill`, `--m-fast`, `--m-base`, `--m-slow`, `--topbar-h`, `--footer-h`, and the `.view-content` class.

- [ ] **Step 1: Write the failing tests**

Create `app/src/lib/theme.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { clampFade, FADE_DEFAULT, normalizeTheme, resolveTheme, themeAttribute } from './theme';

describe('normalizeTheme', () => {
  it('accepts the three stored spellings', () => {
    expect(normalizeTheme('system')).toBe('system');
    expect(normalizeTheme('dark')).toBe('dark');
    expect(normalizeTheme('light')).toBe('light');
  });
  it('falls back to system for anything else, including case and padding', () => {
    expect(normalizeTheme('')).toBe('system');
    expect(normalizeTheme('Dark')).toBe('system');
    expect(normalizeTheme('solarized')).toBe('system');
  });
  it('trims, because a hand-edited config.toml can carry spaces', () => {
    expect(normalizeTheme('  light  ')).toBe('light');
  });
});

describe('resolveTheme', () => {
  it('follows the OS only for the system choice', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
  });
  it('ignores the OS when the user picked a theme', () => {
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('light', true)).toBe('light');
  });
});

describe('themeAttribute', () => {
  it('writes no attribute for system, so the media query decides', () => {
    expect(themeAttribute('system')).toBeNull();
  });
  it('writes the override for an explicit choice', () => {
    expect(themeAttribute('dark')).toBe('dark');
    expect(themeAttribute('light')).toBe('light');
  });
});

describe('clampFade', () => {
  it('keeps values inside the design range', () => {
    expect(clampFade(0)).toBe(0);
    expect(clampFade(25)).toBe(25);
    expect(clampFade(60)).toBe(60);
  });
  it('clamps out-of-range values instead of rejecting them', () => {
    expect(clampFade(-5)).toBe(0);
    expect(clampFade(120)).toBe(60);
  });
  it('rounds fractional slider values and falls back on garbage', () => {
    expect(clampFade(30.6)).toBe(31);
    expect(clampFade(Number.NaN)).toBe(FADE_DEFAULT);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/theme.test.ts`
Expected: FAIL — `Failed to resolve import "./theme"`.

- [ ] **Step 3: Write `theme.ts`**

Create `app/src/lib/theme.ts`:

```ts
// Pure theme + background-fade resolution (design §4). No store and no DOM
// imports here: this module is the unit-testable half of the appearance
// settings, and `stores/uiSettings.svelte.ts` is the reactive half.

export type ThemeChoice = 'system' | 'dark' | 'light';
export type ResolvedTheme = 'dark' | 'light';

/** Design §3: the Appearance slider's range and its default. */
export const FADE_DEFAULT = 25;
export const FADE_MAX = 60;

const CHOICES: ThemeChoice[] = ['system', 'dark', 'light'];

/**
 * `ui.theme` is stored as a free string so an unknown value written by a
 * newer build round-trips through the config instead of failing the load
 * (grid-core's `UiSettings`). Anything this build does not recognize reads
 * back as "follow the OS".
 */
export function normalizeTheme(raw: string): ThemeChoice {
  const trimmed = raw.trim();
  return (CHOICES as string[]).includes(trimmed) ? (trimmed as ThemeChoice) : 'system';
}

export function resolveTheme(choice: ThemeChoice, prefersDark: boolean): ResolvedTheme {
  if (choice === 'dark' || choice === 'light') return choice;
  return prefersDark ? 'dark' : 'light';
}

/**
 * What belongs in `<html data-theme>`: `null` for "system", so the CSS
 * `prefers-color-scheme` media query is left to decide, and the explicit
 * theme otherwise. app.css keys its override blocks off this attribute.
 */
export function themeAttribute(choice: ThemeChoice): ResolvedTheme | null {
  return choice === 'system' ? null : choice;
}

export function clampFade(value: number): number {
  if (!Number.isFinite(value)) return FADE_DEFAULT;
  return Math.min(FADE_MAX, Math.max(0, Math.round(value)));
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/theme.test.ts`
Expected: PASS, 11 assertions across 4 describes.

- [ ] **Step 5: Replace `app.css` with the §4 token set**

Overwrite `app/src/app.css` entirely with:

```css
/* Theme tokens, adopted from RomM v2's `tokens/index.ts` (design §4).
   Defined ONCE, here. Light is the base palette on :root; dark replaces
   the same variables under `prefers-color-scheme: dark` and under an
   explicit `data-theme="dark"`, and light is restated under
   `data-theme="light"` so the override wins in both directions. */
:root {
  /* Surfaces and text — light */
  --bg: #f5f5fa;
  --surface: rgba(0, 0, 0, 0.035);
  --surface-2: #ffffff;
  --border: #dcdce6;
  --text: #111117;
  --text-h: #111117;
  --text-muted: #5a5a70;

  /* Brand — primary darkens in light mode (design §4) */
  --primary: #553e98;
  --primary-hover: #6043c8;
  --primary-pressed: #45307c;
  --secondary: #9e8cd6;
  --accent-warm: #e1a38d;
  --favourite: #ff4f6b;

  /* Status */
  --success: #4ade80;
  --warning: #fbbf24;
  --danger: #ff5050;
  --info: #93c5fd;
  --graph-disk: #2dd4bf;

  /* `--accent` predates the redesign and is still read by pre-redesign
     components (focus rings, progress fills). It tracks --primary. */
  --accent: var(--primary);

  /* Type */
  --sans: 'Segoe UI', system-ui, Inter, Roboto, sans-serif;

  /* Radii (design §4: 4 controls / 6 chips / 8 rows / 14 cards / 100 pills) */
  --r-control: 4px;
  --r-chip: 6px;
  --r-row: 8px;
  --r-card: 14px;
  --r-pill: 100px;

  /* Motion */
  --m-fast: 150ms;
  --m-base: 220ms;
  --m-slow: 360ms;

  /* Shell metrics (design §3) */
  --topbar-h: 58px;
  --footer-h: 28px;

  font: 13px/145% var(--sans);
  color-scheme: light dark;
  color: var(--text);
  background: var(--bg);
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme='light']) {
    --bg: #07070f;
    --surface: rgba(255, 255, 255, 0.07);
    --surface-2: #14141f;
    --border: #22223a;
    --text: #ffffff;
    --text-h: #ffffff;
    --text-muted: #9a9ab0;
    --primary: #8b74e8;
    --primary-hover: #a18fff;
    --primary-pressed: #6043c8;
  }
}

:root[data-theme='dark'] {
  --bg: #07070f;
  --surface: rgba(255, 255, 255, 0.07);
  --surface-2: #14141f;
  --border: #22223a;
  --text: #ffffff;
  --text-h: #ffffff;
  --text-muted: #9a9ab0;
  --primary: #8b74e8;
  --primary-hover: #a18fff;
  --primary-pressed: #6043c8;
}

:root[data-theme='light'] {
  --bg: #f5f5fa;
  --surface: rgba(0, 0, 0, 0.035);
  --surface-2: #ffffff;
  --border: #dcdce6;
  --text: #111117;
  --text-h: #111117;
  --text-muted: #5a5a70;
  --primary: #553e98;
  --primary-hover: #6043c8;
  --primary-pressed: #45307c;
}

body {
  margin: 0;
}

h1 {
  font-family: var(--sans);
  font-size: 20px;
  font-weight: 600;
  color: var(--text-h);
}

#app {
  min-height: 100svh;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
}

/* D-UI-7: list-shaped panes cap at 1100px and centre. Grids do not use
   this class — they may run to the full window width. */
.view-content {
  width: 100%;
  max-width: 1100px;
  margin: 0 auto;
  box-sizing: border-box;
}
```

Two notes for the implementer: the base font size drops from 18px to the design's 13px, so pre-redesign components that set explicit `font-size: 13px` now match the shell instead of shrinking against it; and `--text` and `--text-h` are deliberately the same value in both schemes (RomM v2 has one text colour and a muted one — `--text-muted` replaces the old grey `--text`). Do not chase down every `var(--text)` use in this task; plans 2–5 restyle those panes.

- [ ] **Step 6: Write the settings store**

Create `app/src/lib/stores/uiSettings.svelte.ts`:

```ts
// Appearance settings: the config-backed half of `lib/theme.ts`. Module
// scoped, like `appUpdate.svelte.ts`, so the resolved theme survives Shell
// remounts and every view reads one source.
import { api } from '../api';
import {
  clampFade,
  FADE_DEFAULT,
  normalizeTheme,
  resolveTheme,
  themeAttribute,
  type ResolvedTheme,
  type ThemeChoice,
} from '../theme';

const state = $state<{ theme: ThemeChoice; backgroundFade: number; prefersDark: boolean }>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  prefersDark: false,
});

export const uiSettings = {
  get theme(): ThemeChoice {
    return state.theme;
  },
  get backgroundFade(): number {
    return state.backgroundFade;
  },
  get resolved(): ResolvedTheme {
    return resolveTheme(state.theme, state.prefersDark);
  },
};

/** The single writer of `<html data-theme>`. */
function applyTheme(choice: ThemeChoice): void {
  const attribute = themeAttribute(choice);
  if (attribute === null) delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = attribute;
}

/**
 * Loads the stored settings, applies the attribute, and follows the OS
 * scheme for as long as the returned disposer is not called. Returns a
 * plain function (not a promise of one) so `$effect` teardown is trivial.
 * A failed load is NOT surfaced: the defaults are a perfectly usable
 * shell, and a missing config is the normal first-run case.
 */
export async function initUiSettings(): Promise<() => void> {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  state.prefersDark = media.matches;
  const onChange = (e: MediaQueryListEvent) => {
    state.prefersDark = e.matches;
  };
  media.addEventListener('change', onChange);

  try {
    const stored = await api.getUiSettings();
    state.theme = normalizeTheme(stored.theme);
    state.backgroundFade = clampFade(stored.background_fade);
  } catch {
    // Defaults already in `state`.
  }
  applyTheme(state.theme);

  return () => media.removeEventListener('change', onChange);
}

/** Applies immediately, then persists. */
export async function setTheme(choice: ThemeChoice): Promise<void> {
  state.theme = choice;
  applyTheme(choice);
  await api.setUiSettings({ theme: choice, background_fade: state.backgroundFade });
}

/** Slider drag: updates the live preview without touching the config. */
export function previewBackgroundFade(value: number): void {
  state.backgroundFade = clampFade(value);
}

/** Slider release: persists whatever the preview settled on. */
export async function commitBackgroundFade(value: number): Promise<void> {
  previewBackgroundFade(value);
  await api.setUiSettings({ theme: state.theme, background_fade: state.backgroundFade });
}
```

- [ ] **Step 7: Wire it into `App.svelte`**

In `app/src/App.svelte`, add the import beside the other store imports (after line 11's `initReplenishListener` import):

```ts
  import { initUiSettings } from './lib/stores/uiSettings.svelte';
```

and add this effect directly after the `initAppUpdate` effect (which ends at line 40), before the big restore effect:

```ts
  // The theme must be on `<html>` before the first paint the user sees, so
  // this registers alongside the other pre-shell effects rather than inside
  // Shell.svelte.
  $effect(() => {
    const un = initUiSettings();
    return () => {
      un.then((f) => f());
    };
  });
```

- [ ] **Step 8: Run the frontend gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS. `theme.test.ts` runs alongside the existing suites.

- [ ] **Step 9: Commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src/app.css app/src/lib/theme.ts app/src/lib/theme.test.ts \
  app/src/lib/stores/uiSettings.svelte.ts app/src/App.svelte
git commit -m "rewrite: add RomM v2 theme tokens and the data-theme override"
```

---

### Task 3: The five-view shell

**Files:**
- Modify: `app/src/lib/shell.ts:3` (the `Section` type) and `:24-27` (`initialSection`)
- Modify: `app/src/lib/shell.test.ts:2` and `:20-24`
- Rewrite: `app/src/lib/Shell.svelte` (all 230 lines)
- Modify: `app/src/lib/Emulators.svelte:30` (props), `:199-204` (panel focus), `:205-212` (the load effect), `:570-586` (Escape/backdrop/panel root), `:593` (the close button), `:948-950` (the closing tags), `:953-1002` (backdrop/panel/close styles)
- Modify: `e2e/specs/emulators.spec.ts:63-64`, `:258-268`; `e2e/specs/launch.spec.ts:69-83`; `e2e/specs/cloud-saves.spec.ts:127-143`; `e2e/specs/emulator-catalog.spec.ts:116-131`; `e2e/specs/firmware.spec.ts:90-93`

**Interfaces:**
- Consumes: `uiSettings` is not read here; `chipLabel`, `hostOf` from `shell.ts`; `session`, `retry`, `disconnect` from `stores/session.svelte`; `api.openServerPage()` (Task 1); `appUpdate` from `stores/appUpdate.svelte`.
- Produces, used by Tasks 4, 5, 6 and 7:
  - `app/src/lib/shell.ts`: `export const VIEWS = ['library','server','downloads','emulators','settings'] as const`, `export type View = (typeof VIEWS)[number]`, `export function initialView(connected: boolean): View`, `export function viewLabel(view: View): string`, `export function viewForDigit(key: string): View | null`.
  - `Shell.svelte` exports `handleNav(action)` (unchanged contract) and `show(next: View): void`, called by the footer's "Open Downloads" link (Task 4) and the update badge (Task 6).
  - View roots, all always mounted: `library-view`, `server-view`, `downloads-view`, `emulators-view`, `settings-view`.
  - `Emulators.svelte` takes `{ active?: boolean }` and renders `data-testid="emulators-view"` as its root.

- [ ] **Step 1: Write the failing tests**

Replace the second `describe` block in `app/src/lib/shell.test.ts` (lines 20-32) with this, and change its import line (line 2) to `import { applyRestore, chipLabel, hostOf, initialView, viewForDigit, viewLabel } from './shell';`:

```ts
describe('initialView / viewLabel / viewForDigit / chipLabel / hostOf', () => {
  it('opens Server when connected and Library when offline (R2)', () => {
    expect(initialView(true)).toBe('server');
    expect(initialView(false)).toBe('library');
  });
  it('labels every pill', () => {
    expect(viewLabel('library')).toBe('Library');
    expect(viewLabel('server')).toBe('Server');
    expect(viewLabel('downloads')).toBe('Downloads');
    expect(viewLabel('emulators')).toBe('Emulators');
    expect(viewLabel('settings')).toBe('Settings');
  });
  it('maps Ctrl+1..5 onto the pill order (design §3)', () => {
    expect(viewForDigit('1')).toBe('library');
    expect(viewForDigit('2')).toBe('server');
    expect(viewForDigit('3')).toBe('downloads');
    expect(viewForDigit('4')).toBe('emulators');
    expect(viewForDigit('5')).toBe('settings');
  });
  it('ignores every other key, including 0, 6 and non-digits', () => {
    expect(viewForDigit('0')).toBeNull();
    expect(viewForDigit('6')).toBeNull();
    expect(viewForDigit('f')).toBeNull();
    expect(viewForDigit('')).toBeNull();
    expect(viewForDigit('11')).toBeNull();
  });
  it('labels the chip', () => {
    expect(chipLabel({ phase: 'shell', connected: true, serverUrl: 'https://romm.example:8080/base', username: 'six', lastError: null })).toBe('six @ romm.example:8080');
    expect(chipLabel({ phase: 'shell', connected: false, serverUrl: 'https://x', username: 'six', lastError: 'e' })).toBe('Not connected');
  });
  it('hostOf falls back to the raw string', () => {
    expect(hostOf('not a url')).toBe('not a url');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/shell.test.ts`
Expected: FAIL — `No "initialView" export is defined on the "./shell" mock`/import error for `initialView`, `viewLabel`, `viewForDigit`.

- [ ] **Step 3: Extend `shell.ts`**

In `app/src/lib/shell.ts`, replace line 3 (`export type Section = 'library' | 'server';`) with:

```ts
/** The five first-class views, in pill order (design §3). The index in this
 *  array is also the `Ctrl+<n>` accelerator. */
export const VIEWS = ['library', 'server', 'downloads', 'emulators', 'settings'] as const;
export type View = (typeof VIEWS)[number];
```

and replace `initialSection` (lines 24-27) with:

```ts
/** R2: Server first when connected (E2E specs wait for platform-btn-1 after connecting), Library when offline. */
export function initialView(connected: boolean): View {
  return connected ? 'server' : 'library';
}

export function viewLabel(view: View): string {
  return view.charAt(0).toUpperCase() + view.slice(1);
}

/**
 * The view a `Ctrl+<key>` accelerator selects, or `null` when the key is
 * not one of `1`..`5`. Takes the raw `KeyboardEvent.key` so the caller does
 * no parsing of its own; a multi-character key (`"F1"`, `"11"`) never
 * matches because the lookup is by exact string.
 */
export function viewForDigit(key: string): View | null {
  const index = VIEWS.findIndex((_, i) => String(i + 1) === key);
  return index === -1 ? null : VIEWS[index];
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/shell.test.ts`
Expected: PASS.

- [ ] **Step 5: Turn `Emulators.svelte` from a modal into a view**

Five edits in `app/src/lib/Emulators.svelte`:

(a) Replace the props line (line 30):

```ts
  // Mounted for the whole session now that Emulators is a view, so the
  // refresh below is gated on being the visible view: navigating away and
  // back re-runs `list_platforms`, which is what makes a cleared default
  // survive (the emulators spec's "(none)" case).
  let { active = true }: { active?: boolean } = $props();
```

(b) Delete `let panelEl = $state<HTMLElement | null>(null);` and the focus effect (lines 200-204):

```ts
  let panelEl = $state<HTMLElement | null>(null);

  $effect(() => {
    panelEl?.focus();
  });
```

(c) Gate the load effect (lines 206-212) on `active`:

```ts
  $effect(() => {
    if (!active) return;
    refreshEmulators();
    refreshPlatformsAndDefaults();
    refreshProfiles();
    refreshRaStatus();
    refreshCloudSettings();
  });
```

(d) Delete `onKey` and `onBackdropClick` (lines 570-579) and replace the wrapper markup at lines 582-593:

```svelte
<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="emulators-panel"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label="Emulators"
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="emulators-close" class="close" onclick={onClose} aria-label="Close">×</button>
    <h2>Emulators</h2>
```

with:

```svelte
<section data-testid="emulators-view" class="emulators-view view-content" aria-label="Emulators">
  <h2>Emulators</h2>
```

(e) Close it: the file's last two markup lines before `<style>` (lines 948-950) currently read

```svelte
    </section>
  </div>
</div>
```

Replace them with:

```svelte
    </section>
</section>
```

(f) In the `<style>` block, delete the `.backdrop`, `.panel`, `.panel:focus-visible`, `.close` and `.close:hover, .close:focus-visible` rules (lines 953-1001) and put this in their place:

```css
  .emulators-view {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    box-sizing: border-box;
  }
```

Also drop the `padding-right: 28px;` from the `h2` rule that followed them — it only existed to clear the close button.

- [ ] **Step 6: Rewrite `Shell.svelte`**

Replace the whole of `app/src/lib/Shell.svelte` with:

```svelte
<script lang="ts">
  import Library from './Library.svelte';
  import Server from './Server.svelte';
  import Downloads from './Downloads.svelte';
  import Emulators from './Emulators.svelte';
  import { api } from './api';
  import { session, retry, disconnect } from './stores/session.svelte';
  import { appUpdate, dismiss } from './stores/appUpdate.svelte';
  import { refresh as refreshInstalled } from './stores/installed.svelte';
  import { chipLabel, hostOf, initialView, VIEWS, viewForDigit, viewLabel, type View } from './shell';
  import type { NavDirection } from './focus/grid';

  // Set once when the shell first mounts (R2): Server when the restored/just
  // -connected session is online, Library when it came up offline. Switching
  // views afterward is a user action — a pill, Ctrl+1..5, or `show()`.
  let view = $state<View>(initialView(session.connected));

  let library = $state<ReturnType<typeof Library> | null>(null);
  let server = $state<ReturnType<typeof Server> | null>(null);
  let serverMenuOpen = $state(false);

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (view === 'library') library?.handleNav(action);
    else if (view === 'server') server?.handleNav(action);
  }

  /** Programmatic navigation, for the footer strip and the update badge. */
  export function show(next: View) {
    view = next;
  }

  // Ctrl+1..5 (design §3). Alt/Shift are excluded so this never steals a
  // window-manager or text-editing chord; Meta is accepted alongside Ctrl so
  // the same accelerator works on macOS.
  function onKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    const next = viewForDigit(e.key);
    if (next === null) return;
    e.preventDefault();
    view = next;
  }

  function openServer() {
    serverMenuOpen = false;
    api.openServerPage().catch(() => {
      // The opener refuses a URL it cannot browse to (userinfo, or none
      // stored). Nothing to report: the menu item is a convenience.
    });
  }

  $effect(() => {
    // Independent of the connection: Library shows installed games with
    // cached covers offline, so the registry must load once on mount even
    // when the shell comes up unreachable (Server.svelte's own refresh only
    // runs inside its session.connected-gated effect).
    // The `images-replenished` listener is NOT registered here: the replenish
    // job is spawned during restore/connect, before this shell ever mounts.
    // App.svelte owns it.
    refreshInstalled();
  });
</script>

<svelte:window onkeydown={onKeydown} />

<header data-testid="shell-topbar" class="topbar">
  <div class="brand">
    <span class="logo" aria-hidden="true">▦</span>
    <span class="wordmark">GRID</span>
  </div>

  <nav class="pills" aria-label="Views">
    {#each VIEWS as v (v)}
      <button
        data-testid={`nav-${v}`}
        class="pill"
        class:active={view === v}
        aria-current={view === v ? 'page' : undefined}
        onclick={() => (view = v)}
      >
        {viewLabel(v)}
      </button>
    {/each}
  </nav>

  <div class="session">
    {#if appUpdate.notice}
      <button
        data-testid="app-update-badge"
        class="update-badge"
        title={`GRID Launcher ${appUpdate.notice.tag} is available`}
        onclick={() => (view = 'settings')}
      >
        Update
      </button>
    {/if}
    <span class="status-dot" class:online={session.connected} aria-hidden="true"></span>
    <button
      data-testid="session-chip"
      class="chip"
      title={session.lastError ?? undefined}
      aria-expanded={serverMenuOpen}
      onclick={() => (serverMenuOpen = !serverMenuOpen)}
    >
      {chipLabel(session)}
    </button>
    {#if serverMenuOpen}
      <div class="server-menu" role="menu">
        {#if !session.connected}
          <button
            data-testid="session-retry"
            role="menuitem"
            disabled={session.busy}
            onclick={() => { serverMenuOpen = false; retry(); }}
          >
            Reconnect
          </button>
        {/if}
        <button
          data-testid="session-disconnect"
          role="menuitem"
          onclick={() => { serverMenuOpen = false; disconnect(); }}
        >
          Disconnect
        </button>
        <button data-testid="session-open-romm" role="menuitem" onclick={openServer}>
          Open RomM in browser
        </button>
        <span class="menu-host">{hostOf(session.serverUrl)}</span>
      </div>
    {/if}
  </div>
</header>

{#if !session.connected && session.lastError}
  <p data-testid="session-error" class="error-line">{session.lastError}</p>
{/if}

<!-- All five views stay mounted and switch with `hidden` (design §3), so
     scroll positions, selections and in-flight fetches survive a switch. -->
<div data-testid="library-view" class="view" hidden={view !== 'library'}>
  <Library active={view === 'library'} bind:this={library} />
</div>
<div data-testid="server-view" class="view" hidden={view !== 'server'}>
  <Server active={view === 'server'} bind:this={server} />
</div>
<div data-testid="downloads-view" class="view" hidden={view !== 'downloads'}>
  <Downloads />
</div>
<div class="view" hidden={view !== 'emulators'}>
  <Emulators active={view === 'emulators'} />
</div>
<div data-testid="settings-view" class="view view-content" hidden={view !== 'settings'}>
  <h2>Settings</h2>
  <p class="placeholder">Coming in a later step</p>
  {#if appUpdate.notice}
    <p class="update-line">
      GRID Launcher {appUpdate.notice.tag} is available
      <button data-testid="app-update-open" onclick={() => api.openReleasePage(appUpdate.notice!.url).catch(() => {})}>Open release</button>
      <button data-testid="app-update-dismiss" onclick={dismiss}>Dismiss</button>
    </p>
  {/if}
</div>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    height: var(--topbar-h);
    padding: 0 16px;
    box-sizing: border-box;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 5;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 0;
    min-width: 0;
    color: var(--text-h);
  }

  .logo {
    color: var(--primary);
    font-size: 18px;
  }

  .wordmark {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.08em;
  }

  .pills {
    display: flex;
    gap: 4px;
    flex: 0 0 auto;
    padding: 3px;
    border-radius: var(--r-pill);
    background: var(--surface);
  }

  .pill {
    font: inherit;
    font-size: 13px;
    padding: 5px 16px;
    border-radius: var(--r-pill);
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--m-fast) ease, color var(--m-fast) ease;
  }

  .pill:hover {
    color: var(--text-h);
  }

  .pill.active {
    background: var(--primary);
    color: #fff;
  }

  .session {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex: 1 1 0;
    min-width: 0;
  }

  .status-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }

  .status-dot.online {
    background: var(--success);
  }

  .chip {
    font: inherit;
    font-size: 13px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .chip:hover {
    border-color: var(--border);
  }

  .update-badge {
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
  }

  .server-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 6;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    min-width: 200px;
    padding: 4px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }

  .server-menu button {
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .server-menu button:hover:not(:disabled) {
    background: var(--surface);
  }

  .server-menu button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .menu-host {
    padding: 6px 10px 4px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-line {
    margin: 0;
    padding: 4px 16px;
    color: var(--danger);
    font-size: 11px;
  }

  .view {
    flex: 1 1 auto;
    min-height: 0;
  }

  .placeholder {
    color: var(--text-muted);
  }

  .update-line {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-h);
    font-size: 13px;
  }

  .update-line button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }
</style>
```

Two deliberate points. First, `session-chip` changes from a `<span>` to a `<button>`; no spec reads its tag, only its text. Second, the `app-update-*` ids move into the Settings view root rather than disappearing — Task 6 replaces this whole placeholder block with the real Updates page, and no task in between leaves those ids missing.

- [ ] **Step 7: Rename the Emulators ids across the specs (spec §11)**

Five spec files reference the modal. In each, `emulators-open` becomes `nav-emulators` and `emulators-panel` becomes `emulators-view`; `emulators-close` has no replacement — navigate away instead.

`e2e/specs/emulators.spec.ts:63-64`, `e2e/specs/cloud-saves.spec.ts:127-128`, `e2e/specs/emulator-catalog.spec.ts:116-122` and `e2e/specs/launch.spec.ts:69-75`, `e2e/specs/firmware.spec.ts:90-93` all follow this shape — apply it wherever it appears:

```ts
    await $(testId('nav-emulators')).click();
    await $(testId('emulators-view')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators view never rendered',
    });
```

`e2e/specs/launch.spec.ts:77-83`'s `closeEmulators` helper becomes a navigation back to the Server view (the view it came from):

```ts
  async function closeEmulators() {
    await $(testId('nav-server')).click();
    await $(testId('emulators-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the emulators view never went away',
    });
  }
```

Note `waitForDisplayed` with `reverse: true`, not `waitForExist`: the view stays in the DOM and is only `hidden`. Apply the same replacement to `e2e/specs/emulator-catalog.spec.ts:124-131`'s `closeEmulators` and to the inline close in `e2e/specs/cloud-saves.spec.ts:138-143`.

`e2e/specs/emulators.spec.ts:258-268` — the "(none) survives closing and reopening" case — becomes a navigation round trip:

```ts
  it('the (none) choice survives leaving and re-entering the view', async () => {
    // Re-entering re-runs list_platforms (Emulators.svelte's load effect is
    // gated on `active`), which is where the autoconfig backfill used to
    // re-assign RetroArch over a cleared default.
    await $(testId('nav-server')).click();
    await $(testId('emulators-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the emulators view never went away',
    });
    await $(testId('nav-emulators')).click();
    await $(testId('default-select-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the per-platform defaults list never rendered after re-entering',
    });
```

Leave the rest of that `it` body (the three assertions after line 268) exactly as it is.

- [ ] **Step 8: Run the affected E2E groups**

Run from `rewrite/`: `scripts/e2e.sh emulators launch emulator-catalog cloud-saves firmware`
Expected: all five groups PASS. If a group fails on the emulators content itself (not on navigation), that is product breakage from step 5 — fix `Emulators.svelte`, do not weaken the spec.

- [ ] **Step 9: Run the rest of the gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src/lib/shell.ts app/src/lib/shell.test.ts app/src/lib/Shell.svelte \
  app/src/lib/Emulators.svelte e2e/specs/emulators.spec.ts e2e/specs/launch.spec.ts \
  e2e/specs/cloud-saves.spec.ts e2e/specs/emulator-catalog.spec.ts e2e/specs/firmware.spec.ts
git commit -m "rewrite: five-view shell with pill navigation and an emulators view"
```

---

### Task 4: Downloads view and the 28px footer strip

**Files:**
- Modify: `app/src/lib/downloads/format.ts:135` (append `footerLine`) and `app/src/lib/downloads/format.test.ts` (append a describe)
- Create: `app/src/lib/DownloadsFooter.svelte`
- Modify: `app/src/lib/Downloads.svelte:1-156` (drop the footer, the drawer wrapper, the toggle and the emulators button) and its `<style>` block
- Modify: `app/src/lib/Shell.svelte` (mount the footer, drop the `downloads-view` padding gap)
- Modify: `e2e/specs/install-a.spec.ts:41-44`, `firmware.spec.ts:55-58`, `native.spec.ts:66-69`, `ps3-install.spec.ts:57-60`, `downloads.spec.ts:58-61`, `content.spec.ts:73-76`, `updates.spec.ts:120-124`, `emulator-catalog.spec.ts:209-213`

**Interfaces:**
- Consumes: `downloads` store (`entries`, `hasLive`) from `stores/downloads.svelte`; `formatSize`, `percent` from `downloads/format`; `Shell.svelte`'s exported `show(view)` (Task 3).
- Produces, used by Tasks 6 and 7:
  - `export function footerLine(entries: DownloadEntry[]): string | null` in `app/src/lib/downloads/format.ts`.
  - `DownloadsFooter.svelte` with props `{ onOpen: () => void }`, root test id `downloads-footer`, hidden when nothing is live.
  - `Downloads.svelte` takes no props and renders only the list (root test id stays on `downloads-view`, owned by `Shell.svelte`).

- [ ] **Step 1: Write the failing test**

Append to `app/src/lib/downloads/format.test.ts`, adding `footerLine` to the file's existing import from `./format` (line 3). It reuses the `entry()` fixture already defined at the top of that file (line 5), whose defaults are `title: 'Game'`, `status: 'queued'` and zeros everywhere else:

```ts
describe('footerLine', () => {
  it('is null when nothing is live, so the strip can hide', () => {
    expect(footerLine([])).toBeNull();
    expect(footerLine([entry({ status: 'completed' })])).toBeNull();
    expect(footerLine([entry({ status: 'failed' })])).toBeNull();
  });

  it('shows the downloading transfer with percent and speed', () => {
    const line = footerLine([
      entry({ title: 'Chrono Trigger', status: 'downloading', downloaded_bytes: 512, total_bytes: 1024, speed_bps: 2048 }),
    ]);
    expect(line).toBe('⬇ Chrono Trigger · 50% · 2.0 KB/s');
  });

  it('shows an em dash for an unknown total', () => {
    const line = footerLine([
      entry({ title: 'Chrono Trigger', status: 'downloading', downloaded_bytes: 512, speed_bps: 0 }),
    ]);
    expect(line).toBe('⬇ Chrono Trigger · — · 0 B/s');
  });

  it('prefers a downloading entry over an installing one', () => {
    const line = footerLine([
      entry({ id: 1, title: 'Installing One', status: 'installing', install_processed_bytes: 1, install_total_bytes: 2 }),
      entry({ id: 2, title: 'Downloading One', status: 'downloading', downloaded_bytes: 1, total_bytes: 4, speed_bps: 1024 }),
    ]);
    expect(line).toBe('⬇ Downloading One · 25% · 1.0 KB/s');
  });

  it('reports the phase instead of a speed for installing and queued work', () => {
    expect(
      footerLine([entry({ title: 'A', status: 'installing', install_processed_bytes: 3, install_total_bytes: 4 })]),
    ).toBe('⬇ A · 75% · Installing');
    expect(footerLine([entry({ title: 'A', status: 'queued' })])).toBe('⬇ A · — · Queued');
    expect(footerLine([entry({ title: 'A', status: 'cancelling' })])).toBe('⬇ A · — · Cancelling');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/downloads/format.test.ts`
Expected: FAIL — `footerLine is not exported by ./format`.

- [ ] **Step 3: Implement `footerLine`**

Append to `app/src/lib/downloads/format.ts`:

```ts
/**
 * The 28px status strip's one line (design §3):
 * `⬇ <title> · <percent> · <speed>`, or `null` when nothing is live and the
 * strip hides itself.
 *
 * "The current transfer" is the first downloading entry, else the first
 * installing one, else the first entry in any other live state — the same
 * precedence the old drawer footer's progress bar used. An unmeasurable
 * percent renders as an em dash rather than a fake `0%`, and the speed slot
 * carries the phase word when there is no byte rate to show (an install
 * reads local bytes, and a queued job has not started).
 */
export function footerLine(entries: DownloadEntry[]): string | null {
  const live = entries.filter((e) => LIVE_STATUSES.includes(e.status));
  if (live.length === 0) return null;
  const current =
    live.find((e) => e.status === 'downloading') ??
    live.find((e) => e.status === 'installing') ??
    live[0];

  const dash = '—';
  let pct = dash;
  let speed: string;
  switch (current.status) {
    case 'downloading':
      if (current.total_bytes > 0) pct = `${percent(current.downloaded_bytes, current.total_bytes)}%`;
      speed = `${formatSize(current.speed_bps)}/s`;
      break;
    case 'installing':
      if (current.install_total_bytes > 0) {
        pct = `${percent(current.install_processed_bytes, current.install_total_bytes)}%`;
      }
      speed = 'Installing';
      break;
    case 'cancelling':
      speed = 'Cancelling';
      break;
    default:
      speed = 'Queued';
      break;
  }
  return `⬇ ${current.title} · ${pct} · ${speed}`;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/downloads/format.test.ts`
Expected: PASS, including the pre-existing `aggregate`/`entryDetail` cases.

- [ ] **Step 5: Create the footer strip**

Create `app/src/lib/DownloadsFooter.svelte`:

```svelte
<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { footerLine } from './downloads/format';

  let { onOpen }: { onOpen: () => void } = $props();

  let line = $derived(footerLine(downloads.entries));
</script>

<!-- Always mounted, hidden when nothing is live (design §3). Clicking
     anywhere on the strip opens the Downloads view. -->
<footer
  data-testid="downloads-footer"
  class="strip"
  hidden={line === null}
  role="button"
  tabindex="0"
  onclick={onOpen}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onOpen();
    }
  }}
>
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
  <!-- Plan 4 puts the 60-sample sparkline here; the slot reserves its
       120×18 footprint now so the strip's height never changes later. -->
  <span class="sparkline-slot" aria-hidden="true"></span>
  <span class="open-link">Open Downloads</span>
</footer>

<style>
  .strip {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--footer-h);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    background: var(--surface-2);
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    z-index: 10;
  }

  .strip[hidden] {
    display: none;
  }

  .strip:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .line {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-h);
  }

  .sparkline-slot {
    flex: none;
    width: 120px;
    height: 18px;
  }

  .open-link {
    flex: none;
    color: var(--primary);
    text-decoration: underline;
    white-space: nowrap;
  }
</style>
```

`[hidden]` needs the explicit `display: none` because `.strip` sets `display: flex`, which otherwise wins.

- [ ] **Step 6: Strip `Downloads.svelte` down to the list**

In `app/src/lib/Downloads.svelte`:

(a) Delete the `slide` import (line 2), the props line (line 7), `let open = $state(false);` (line 9), `toggle` (lines 13-15), `toggleOnKey` (17-22), `openEmulatorsClick` (24-27), `stopKeydownPropagation` (32-34), `footerProgress` (83-91) and the two derived lines (93-94).

(b) Delete the whole `<footer …>` element (lines 97-113) and replace the drawer wrapper (line 116 and its closing `{/if}`/`</div>` at 155-156) so the file's markup begins:

```svelte
<section class="downloads view-content" aria-label="Downloads">
  {#if downloads.entries.length === 0}
    <p class="empty">No downloads yet</p>
  {:else}
```

and ends (replacing lines 154-156):

```svelte
    {/each}
  {/if}
</section>
```

(c) In `<style>`, delete `.downloads-footer`, `.downloads-footer:focus-visible`, `.label`, `.emulators-btn`, `.emulators-btn:hover, .emulators-btn:focus-visible` and `.drawer`, and add:

```css
  .downloads {
    padding: 24px 24px 60px;
    box-sizing: border-box;
  }
```

Keep `.bar-track`, `.bar-fill`, the indeterminate keyframes, `.empty`, `.row*`, `.title*`, `.kind`, `.platform` and `.detail` exactly as they are — plan 4 restyles them.

(d) `rowProgress` stays; it is now used only by the rows.

- [ ] **Step 7: Mount the footer in the shell**

In `app/src/lib/Shell.svelte`, add the import beside the other component imports:

```ts
  import DownloadsFooter from './DownloadsFooter.svelte';
```

change the Downloads view root to pass nothing:

```svelte
<div data-testid="downloads-view" class="view" hidden={view !== 'downloads'}>
  <Downloads />
</div>
```

(`Downloads.svelte`'s own `<section>` carries `view-content`, so the wrapper must not repeat it.)

and add the strip as the last element before `<style>`:

```svelte
<DownloadsFooter onOpen={() => (view = 'downloads')} />
```

- [ ] **Step 8: Point the specs at the Downloads view**

Eight specs open the drawer by clicking the footer. The strip is now hidden while nothing is live, so clicking it is no longer a reliable way in — every one of them switches to the pill. In `install-a.spec.ts:41-44`, `firmware.spec.ts:55-58`, `native.spec.ts:66-69`, `ps3-install.spec.ts:57-60`, `downloads.spec.ts:58-61`, `content.spec.ts:73-76`, `updates.spec.ts:120-124` and `emulator-catalog.spec.ts:209-213`, replace the pair

```ts
    await $(testId('downloads-footer')).click();
    await $(testId('downloads-drawer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads drawer never opened',
    });
```

with

```ts
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
```

(keeping each file's own `timeoutMsg` wording where it differs). Any spec that afterwards clicks a Library or Server card must now navigate back with `nav-library`/`nav-server` first — `updates.spec.ts:126` already does exactly that, and `emulator-catalog.spec.ts` stays on the Emulators view via `openEmulators()`. Check each file after editing: the views no longer stack, so a spec that assumed the drawer floated over the grid needs an explicit pill click.

Then give the strip its own coverage in `downloads.spec.ts`, which is the one group with a real throttled in-flight download. Add this `it` directly after that file's existing "a second install queues behind the first" case:

```ts
  it('shows the live transfer on the footer strip and opens the view from it', async () => {
    const strip = $(testId('downloads-footer'));
    await strip.waitForDisplayed({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the downloads strip never appeared for a live transfer',
    });
    expect(await strip.getText()).toContain('⬇ ');
    await $(testId('nav-library')).click();
    await strip.click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking the strip did not open the Downloads view',
    });
  });
```

- [ ] **Step 9: Run the affected E2E groups**

Run from `rewrite/`: `scripts/e2e.sh downloads install content native ps3-install firmware emulator-catalog updates`
Expected: PASS. `updates` still fails its self-update case at this point only if step 8 was applied to the wrong lines — the banner block still exists (it moved into the Settings root in Task 3 and is replaced in Task 6), so `app-update-*` must still resolve. If `updates` fails on `app-update-banner`, that is expected and is fixed in Task 6; note it and continue.

- [ ] **Step 10: Run the rest of the gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

- [ ] **Step 11: Commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src/lib/downloads/format.ts app/src/lib/downloads/format.test.ts \
  app/src/lib/DownloadsFooter.svelte app/src/lib/Downloads.svelte app/src/lib/Shell.svelte \
  e2e/specs/install-a.spec.ts e2e/specs/firmware.spec.ts e2e/specs/native.spec.ts \
  e2e/specs/ps3-install.spec.ts e2e/specs/downloads.spec.ts e2e/specs/content.spec.ts \
  e2e/specs/updates.spec.ts e2e/specs/emulator-catalog.spec.ts
git commit -m "rewrite: downloads view plus a 28px status strip"
```

---

### Task 5: Background art

**Files:**
- Create: `app/src/lib/background.ts`, `app/src/lib/background.test.ts`, `app/src/lib/stores/lastViewed.svelte.ts`, `app/src/lib/BackgroundArt.svelte`
- Modify: `app/src/lib/Library.svelte:19-25` (details open), `:63-70` (card hover)
- Modify: `app/src/lib/Server.svelte:56-58` (details open), `:140` (card hover)
- Modify: `app/src/lib/Shell.svelte` (mount `BackgroundArt`, seed on the installed list)

**Interfaces:**
- Consumes: `uiSettings.backgroundFade` (Task 2); `installed` store from `stores/installed.svelte`; `api.ensureImage(url)` and `convertFileSrc`, as `Image.svelte` uses them.
- Produces, used by Task 6's live preview:
  - `export function startupCover(rows: InstalledGame[]): string | null` in `app/src/lib/background.ts`.
  - `app/src/lib/stores/lastViewed.svelte.ts`: `export const lastViewed` with getter `coverUrl: string | null`; `export function noteViewed(coverUrl: string | null | undefined): void`; `export function seedLastViewed(rows: InstalledGame[]): void`.
  - `BackgroundArt.svelte`, no props, root test id `background-art`.
  - `HOVER_DELAY_MS = 500` exported from `app/src/lib/background.ts`.

- [ ] **Step 1: Write the failing test**

Create `app/src/lib/background.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { InstalledGame } from './api';
import { HOVER_DELAY_MS, startupCover } from './background';

function row(overrides: Partial<InstalledGame>): InstalledGame {
  // Only the three fields `startupCover` reads are meaningful; the rest are
  // filled from the registry's own "blank, never null" convention.
  return {
    title: 'Chrono Trigger', platform: 'SNES', rom_id: 42, rom_file_name: '', archive_path: '',
    extracted_path: '', extracted_dir: '', multi_file_game_dir: '', description: '', rating: '',
    genres: '', regions: '', languages: '', tags: '', revision: '', companies: '',
    first_release_date: '', filesize_bytes: 0, server_updated_at: '', installed_at: 0,
    cover_small_path: '', cover_large_path: '', screenshot_urls: '', native_executable_path: '',
    native_launch_parameters: '', native_compat_tool: '', native_wineprefix: '',
    native_game_dir: '', included_dlc: '', ps3_trophy_paths: '', ps3_game_id: '',
    ps3_iso_path: '', ps4_game_id: '', ps4_content: '', ra_id: '',
    ...overrides,
  };
}

describe('startupCover', () => {
  it('is null when there is nothing installed', () => {
    expect(startupCover([])).toBeNull();
  });

  it('picks the newest row that actually has a large cover', () => {
    expect(
      startupCover([
        row({ installed_at: 100, cover_large_path: 'https://romm/old.png' }),
        row({ installed_at: 300, cover_large_path: 'https://romm/newest.png' }),
        row({ installed_at: 200, cover_large_path: 'https://romm/middle.png' }),
      ]),
    ).toBe('https://romm/newest.png');
  });

  it('skips cover-less rows rather than returning a blank', () => {
    expect(
      startupCover([
        row({ installed_at: 900, cover_large_path: '' }),
        row({ installed_at: 100, cover_large_path: 'https://romm/only.png' }),
      ]),
    ).toBe('https://romm/only.png');
  });

  it('is null when no row has a cover at all', () => {
    expect(startupCover([row({ installed_at: 5 }), row({ installed_at: 6 })])).toBeNull();
  });

  it('holds the 500ms hover dwell from design section 3', () => {
    expect(HOVER_DELAY_MS).toBe(500);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/background.test.ts`
Expected: FAIL — `Failed to resolve import "./background"`.

- [ ] **Step 3: Write `background.ts`**

Create `app/src/lib/background.ts`:

```ts
// Pure selection logic for the shell's background art (design §3).
import type { InstalledGame } from './api';

/** Design §3: a card must be hovered for MORE than half a second before it
 *  becomes the background. Shorter dwells are pointer travel, not interest. */
export const HOVER_DELAY_MS = 500;

/**
 * The cover the shell starts with, before the user has viewed anything.
 *
 * The design asks for "the most recently played installed game". The
 * registry records no play timestamp — nothing in grid-core stores a
 * `last_played` — so the newest `installed_at` stands in for it: the game
 * a user just added is the one they are about to play. Revisit this when a
 * play-time column exists.
 *
 * Rows without a large cover are skipped rather than returned blank: the
 * caller would otherwise render an empty layer over a perfectly good
 * candidate further down the list.
 */
export function startupCover(rows: InstalledGame[]): string | null {
  let best: InstalledGame | null = null;
  for (const row of rows) {
    if (row.cover_large_path.trim() === '') continue;
    if (best === null || row.installed_at > best.installed_at) best = row;
  }
  return best?.cover_large_path ?? null;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/background.test.ts`
Expected: PASS.

- [ ] **Step 5: Write the `lastViewed` store**

Create `app/src/lib/stores/lastViewed.svelte.ts`:

```ts
// The cover the background art is showing. Module scoped so it survives a
// Shell remount, like `appUpdate.svelte.ts`.
import type { InstalledGame } from '../api';
import { startupCover } from '../background';

const state = $state<{ coverUrl: string | null; seeded: boolean }>({
  coverUrl: null,
  seeded: false,
});

export const lastViewed = {
  get coverUrl(): string | null {
    return state.coverUrl;
  },
};

/** A details popup opened, or a card was hovered past the dwell. Blank and
 *  missing covers are ignored: keeping the previous art beats a blank frame. */
export function noteViewed(coverUrl: string | null | undefined): void {
  if (typeof coverUrl !== 'string') return;
  const trimmed = coverUrl.trim();
  if (trimmed === '') return;
  state.coverUrl = trimmed;
  state.seeded = true;
}

/** The startup fallback. Runs once, and never overwrites a real view. */
export function seedLastViewed(rows: InstalledGame[]): void {
  if (state.seeded) return;
  const cover = startupCover(rows);
  if (cover === null) return;
  state.coverUrl = cover;
  state.seeded = true;
}
```

- [ ] **Step 6: Write `BackgroundArt.svelte`**

Create `app/src/lib/BackgroundArt.svelte`:

```svelte
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './api';
  import { lastViewed } from './stores/lastViewed.svelte';
  import { uiSettings } from './stores/uiSettings.svelte';

  // Two layers so a change cross-fades rather than popping (design §3:
  // 360ms). `front` is the visible one; a new cover loads into the other
  // layer and the two swap once its file path has resolved.
  let front = $state<string | null>(null);
  let back = $state<string | null>(null);
  let frontIsA = $state(true);

  $effect(() => {
    const url = lastViewed.coverUrl;
    if (url === null) return;
    let cancelled = false;
    api
      .ensureImage(url)
      .then((path) => {
        if (cancelled) return;
        const src = convertFileSrc(path);
        if (src === front) return;
        back = src;
        front = src;
        frontIsA = !frontIsA;
      })
      .catch(() => {
        // Offline or missing: keep whatever is already showing.
      });
    return () => {
      cancelled = true;
    };
  });

  // 0–60 in the config, 0–0.6 as an opacity.
  let opacity = $derived(uiSettings.backgroundFade / 100);
</script>

<div data-testid="background-art" class="art" aria-hidden="true" style={`--art-opacity: ${opacity}`}>
  <div class="layer" class:visible={frontIsA} style={frontIsA && front ? `background-image: url("${front}")` : ''}></div>
  <div class="layer" class:visible={!frontIsA} style={!frontIsA && back ? `background-image: url("${back}")` : ''}></div>
</div>

<style>
  .art {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }

  .layer {
    position: absolute;
    /* Overscan: a 40px blur samples past the element's own edges and would
       otherwise fade to the page background at every side. */
    inset: -60px;
    background-position: center;
    background-size: cover;
    filter: blur(40px);
    opacity: 0;
    transition: opacity var(--m-slow) ease;
  }

  .layer.visible {
    opacity: var(--art-opacity);
  }
</style>
```

- [ ] **Step 7: Feed it from the two grids**

In `app/src/lib/Library.svelte`, add the imports:

```ts
  import { HOVER_DELAY_MS } from './background';
  import { noteViewed } from './stores/lastViewed.svelte';
```

extend `openDetails` (lines 19-21):

```ts
  function openDetails(row: InstalledGame) {
    subject = fromInstalled(row);
    noteViewed(row.cover_large_path);
  }
```

and add the dwell timer plus its handlers below `closeDetails`:

```ts
  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than half a second.
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;

  function startHover(row: InstalledGame) {
    if (hoverTimer !== null) clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => {
      hoverTimer = null;
      noteViewed(row.cover_large_path);
    }, HOVER_DELAY_MS);
  }

  function endHover() {
    if (hoverTimer === null) return;
    clearTimeout(hoverTimer);
    hoverTimer = null;
  }
```

then add the two handlers to the card div (currently lines 63-70), leaving every other attribute alone:

```svelte
          onmouseenter={() => startHover(row)}
          onmouseleave={endHover}
```

Apply the same four changes to `app/src/lib/Server.svelte`: the imports, `openDetails` (line 56) becoming

```ts
  function openDetails(game: GameSummary) {
    detailsGame = game;
    noteViewed(game.path_cover_large);
  }
```

the identical `hoverTimer` / `startHover` / `endHover` block (typed `startHover(game: GameSummary)` and reading `game.path_cover_large`), and the two handlers on the card at line 140.

- [ ] **Step 8: Mount and seed it in the shell**

In `app/src/lib/Shell.svelte`, add the imports:

```ts
  import BackgroundArt from './BackgroundArt.svelte';
  import { installed } from './stores/installed.svelte';
  import { seedLastViewed } from './stores/lastViewed.svelte';
```

add `<BackgroundArt />` as the very first element of the markup, above `<header data-testid="shell-topbar">`, and add this effect after the `refreshInstalled()` one:

```ts
  // Startup fallback for the background art. Re-runs as the registry loads;
  // `seedLastViewed` is idempotent and never overwrites a real view.
  $effect(() => {
    seedLastViewed(installed.list);
  });
```

Finally, give the shell's own chrome a stacking context above the art. `.topbar` already has `position: sticky; z-index: 5;` and needs no change; add to the `.view` rule in `Shell.svelte`'s `<style>`:

```css
    position: relative;
    z-index: 1;
```

so the views paint above `BackgroundArt`'s `z-index: 0` fixed layer.

- [ ] **Step 9: Run the gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

Run from `rewrite/`: `scripts/e2e.sh library images`
Expected: PASS. `images` is the group that asserts real `<img>` loading, so it is the one most likely to notice a stacking or pointer-events mistake in the art layer.

- [ ] **Step 10: Commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src/lib/background.ts app/src/lib/background.test.ts \
  app/src/lib/stores/lastViewed.svelte.ts app/src/lib/BackgroundArt.svelte \
  app/src/lib/Library.svelte app/src/lib/Server.svelte app/src/lib/Shell.svelte
git commit -m "rewrite: blurred background art from the last viewed cover"
```

---

### Task 6: Settings view with the Appearance page

**Files:**
- Create: `app/src/lib/settings.ts`, `app/src/lib/settings.test.ts`, `app/src/lib/Settings.svelte`
- Modify: `app/src/lib/Shell.svelte` (replace the `settings-view` placeholder block with `<Settings />`)
- Modify: `e2e/specs/updates.spec.ts:158-175` (the self-update case)

**Interfaces:**
- Consumes: `uiSettings`, `setTheme`, `previewBackgroundFade`, `commitBackgroundFade` (Task 2); `FADE_MAX` from `lib/theme`; `appUpdate`, `dismiss` from `stores/appUpdate.svelte`; `api.openReleasePage(url)`.
- Produces, used by Task 7 and plan 5:
  - `app/src/lib/settings.ts`: `export const SETTINGS_PAGES = ['connection','cloud-saves','retroachievements','updates','appearance'] as const`, `export type SettingsPage = (typeof SETTINGS_PAGES)[number]`, `export function settingsPageLabel(page: SettingsPage): string`, `export const LATER_STEP_TEXT = 'Coming in a later step'`.
  - `Settings.svelte`, no props, rendering `settings-nav-<page>` rail buttons, `theme-select`, `background-fade`, `app-update-open`, `app-update-dismiss`.

- [ ] **Step 1: Write the failing test**

Create `app/src/lib/settings.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { LATER_STEP_TEXT, SETTINGS_PAGES, settingsPageLabel } from './settings';

describe('settings rail', () => {
  it('lists the five pages of design §10, in order', () => {
    expect([...SETTINGS_PAGES]).toEqual([
      'connection',
      'cloud-saves',
      'retroachievements',
      'updates',
      'appearance',
    ]);
  });

  it('labels every page', () => {
    expect(settingsPageLabel('connection')).toBe('Connection');
    expect(settingsPageLabel('cloud-saves')).toBe('Cloud saves');
    expect(settingsPageLabel('retroachievements')).toBe('RetroAchievements');
    expect(settingsPageLabel('updates')).toBe('Updates');
    expect(settingsPageLabel('appearance')).toBe('Appearance');
  });

  it('holds the placeholder copy verbatim', () => {
    expect(LATER_STEP_TEXT).toBe('Coming in a later step');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run from `rewrite/app`: `npx vitest run src/lib/settings.test.ts`
Expected: FAIL — `Failed to resolve import "./settings"`.

- [ ] **Step 3: Write `settings.ts`**

Create `app/src/lib/settings.ts`:

```ts
// The Settings rail (design §10). Only Appearance is built in this plan;
// plan 5 fills in the other four pages.

export const SETTINGS_PAGES = [
  'connection',
  'cloud-saves',
  'retroachievements',
  'updates',
  'appearance',
] as const;

export type SettingsPage = (typeof SETTINGS_PAGES)[number];

const LABELS: Record<SettingsPage, string> = {
  connection: 'Connection',
  'cloud-saves': 'Cloud saves',
  retroachievements: 'RetroAchievements',
  updates: 'Updates',
  appearance: 'Appearance',
};

export function settingsPageLabel(page: SettingsPage): string {
  return LABELS[page];
}

/** The exact line an unbuilt page shows. Asserted by settings.test.ts so it
 *  cannot drift while five call sites reference it. */
export const LATER_STEP_TEXT = 'Coming in a later step';
```

- [ ] **Step 4: Run the test to verify it passes**

Run from `rewrite/app`: `npx vitest run src/lib/settings.test.ts`
Expected: PASS.

- [ ] **Step 5: Write `Settings.svelte`**

Create `app/src/lib/Settings.svelte`:

```svelte
<script lang="ts">
  import { api } from './api';
  import { appUpdate, dismiss } from './stores/appUpdate.svelte';
  import {
    commitBackgroundFade,
    previewBackgroundFade,
    setTheme,
    uiSettings,
  } from './stores/uiSettings.svelte';
  import { FADE_MAX, type ThemeChoice } from './theme';
  import { LATER_STEP_TEXT, SETTINGS_PAGES, settingsPageLabel, type SettingsPage } from './settings';

  let page = $state<SettingsPage>('appearance');

  function onThemeChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as ThemeChoice;
    setTheme(value).catch(() => {
      // The attribute is already applied; a failed save is not worth a
      // blocking error in a settings pane.
    });
  }
</script>

<div class="settings">
  <nav class="rail" aria-label="Settings pages">
    {#each SETTINGS_PAGES as p (p)}
      <button
        data-testid={`settings-nav-${p}`}
        class="rail-item"
        class:active={page === p}
        aria-current={page === p ? 'page' : undefined}
        onclick={() => (page = p)}
      >
        {settingsPageLabel(p)}
      </button>
    {/each}
  </nav>

  <section class="pane" aria-label={settingsPageLabel(page)}>
    <h2>{settingsPageLabel(page)}</h2>

    {#if page === 'appearance'}
      <div class="field">
        <label for="theme-select">Theme</label>
        <select data-testid="theme-select" id="theme-select" value={uiSettings.theme} onchange={onThemeChange}>
          <option value="system">Follow system</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </div>

      <div class="field">
        <label for="background-fade">Background art fade</label>
        <!-- `oninput` previews live behind this pane (the art reads the same
             store); `onchange` is what reaches config.toml. -->
        <input
          data-testid="background-fade"
          id="background-fade"
          type="range"
          min="0"
          max={FADE_MAX}
          step="1"
          value={uiSettings.backgroundFade}
          oninput={(e) => previewBackgroundFade(Number((e.currentTarget as HTMLInputElement).value))}
          onchange={(e) => {
            commitBackgroundFade(Number((e.currentTarget as HTMLInputElement).value)).catch(() => {});
          }}
        />
        <span class="value">{uiSettings.backgroundFade}%</span>
      </div>
    {:else if page === 'updates'}
      {#if appUpdate.notice}
        <p data-testid="app-update-notice" class="update-line">
          GRID Launcher {appUpdate.notice.tag} is available
          <button data-testid="app-update-open" onclick={() => api.openReleasePage(appUpdate.notice!.url).catch(() => {})}>
            Open release
          </button>
          <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
        </p>
      {/if}
      <p class="placeholder">{LATER_STEP_TEXT}</p>
    {:else}
      <p class="placeholder">{LATER_STEP_TEXT}</p>
    {/if}
  </section>
</div>

<style>
  .settings {
    display: flex;
    gap: 24px;
    padding: 24px;
    box-sizing: border-box;
  }

  .rail {
    flex: 0 0 200px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rail-item {
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 8px 12px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rail-item:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .rail-item.active {
    background: var(--surface);
    color: var(--text-h);
    font-weight: 600;
  }

  .pane {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-h);
  }

  .field {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--text-h);
  }

  .field label {
    flex: 0 0 180px;
  }

  .field input[type='range'] {
    flex: 1 1 auto;
    max-width: 320px;
    accent-color: var(--primary);
  }

  .value {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .placeholder {
    margin: 0;
    color: var(--text-muted);
  }

  .update-line {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .update-line button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .update-line button.secondary {
    border-color: transparent;
    color: var(--text-muted);
  }
</style>
```

- [ ] **Step 6: Swap the placeholder out of the shell**

In `app/src/lib/Shell.svelte`, add the import:

```ts
  import Settings from './Settings.svelte';
```

replace the whole `settings-view` block written in Task 3 with:

```svelte
<div data-testid="settings-view" class="view" hidden={view !== 'settings'}>
  <Settings />
</div>
```

and remove the now-unused `dismiss` import and the `.placeholder` / `.update-line` / `.update-line button` style rules from `Shell.svelte` (`appUpdate` is still read by the badge, so keep that import).

- [ ] **Step 7: Update the self-update E2E case**

Replace the `it('announces a newer launcher release and lets it be dismissed', …)` body in `e2e/specs/updates.spec.ts` (lines 158-175) with:

```ts
  it('badges a newer launcher release and lets it be dismissed from Settings', async () => {
    // The banner strip is gone (design §3): the notice is a badge on the
    // server menu plus an entry under Settings › Updates.
    await $(testId('app-update-badge')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the self-update badge never appeared for the mock forge release',
    });
    await $(testId('app-update-badge')).click();
    await $(testId('settings-nav-updates')).click();
    const notice = $(testId('app-update-notice'));
    await notice.waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Settings › Updates never showed the stored notice',
    });
    expect(await notice.getText()).toContain(SELF_UPDATE_TAG);

    // `app-update-open` is deliberately NOT clicked: it hands the URL to the
    // OS opener, which would spawn a real browser out of the headless run.
    await $(testId('app-update-dismiss')).click();
    await $(testId('app-update-badge')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the self-update badge survived Dismiss',
    });
  });
```

Any later `it` in that file that expects to be on the Library view must click `nav-library` first — this case now ends on Settings. Read the cases after it and add the click where needed.

- [ ] **Step 8: Run the gate**

Run from `rewrite/app`: `npm run check && npx vitest run`
Expected: PASS.

Run from `rewrite/`: `scripts/e2e.sh updates`
Expected: PASS, including the rewritten self-update case.

- [ ] **Step 9: Commit**

```bash
cd rewrite
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
git add app/src/lib/settings.ts app/src/lib/settings.test.ts app/src/lib/Settings.svelte \
  app/src/lib/Shell.svelte e2e/specs/updates.spec.ts
git commit -m "rewrite: settings view with the appearance page and the update badge"
```

---

### Task 7: Full E2E sweep

**Files:**
- Modify: whichever `e2e/specs/*.spec.ts` files still fail (test-only changes)

**Interfaces:**
- Consumes: every id produced by Tasks 3–6 — `nav-library`, `nav-server`, `nav-downloads`, `nav-emulators`, `nav-settings`, `library-view`, `server-view`, `downloads-view`, `emulators-view`, `settings-view`, `downloads-footer`, `settings-nav-<page>`, `theme-select`, `background-fade`, `background-art`, `app-update-badge`, `app-update-notice`, `app-update-open`, `app-update-dismiss`.
- Produces: nothing new. This task's deliverable is a green suite.

- [ ] **Step 1: Confirm no renamed id survives anywhere**

Run from `rewrite/`:

```bash
grep -rn "emulators-open\|emulators-panel\|emulators-close\|downloads-drawer\|app-update-banner" e2e/ app/src/ | grep -v node_modules
```

Expected: no output. Every hit is a rename Tasks 3, 4 or 6 missed — fix it before running the suite.

- [ ] **Step 2: Run every group**

Run from `rewrite/`: `scripts/e2e.sh`
Expected: all 15 groups PASS (`connect`, `connect-restore`, `library`, `install`, `downloads`, `emulators`, `launch`, `emulator-catalog`, `cloud-saves`, `images`, `ps3-install`, `content`, `native`, `firmware`, `updates`).

- [ ] **Step 3: Triage each failure before touching anything**

For every failing group, decide which of two kinds it is, and say which in the commit message:

- **Test breakage** — the spec drives the old shell: it clicks a removed control, waits for a modal that is now a view, or assumes the drawer floats over a grid. Fix the spec: add the `nav-*` click, switch `waitForExist` to `waitForDisplayed` for a `hidden`-switched view, or reorder the steps.
- **Product breakage** — the app genuinely stopped doing something (an emulator no longer saves, an install never completes, a cover never loads). Do NOT weaken the spec. Fix the component, or escalate if the fix reaches outside this plan's files.

Two `hidden`-specific traps to expect: `waitForExist({ reverse: true })` never fires for a view that is only hidden — use `waitForDisplayed`; and `getText()` on a hidden element returns `''` in WebDriver, so a text assertion must follow a navigation click.

- [ ] **Step 4: Re-run until green**

Run from `rewrite/`: `scripts/e2e.sh <the groups you changed>`, then `scripts/e2e.sh` once more in full.
Expected: all 15 groups PASS in one uninterrupted run.

- [ ] **Step 5: Commit**

```bash
cd rewrite
git add e2e/
git commit -m "rewrite: bring the E2E suite onto the five-view shell"
```

(If step 3 required a component fix, add that file to the same commit and name the behavior in the subject.)

---

### Task 8: Documentation

**Files:**
- Modify: `SPEC.md:5-24` (Top Bar and Main Sections)
- Modify: `rewrite/README.md:114-138` (the residual manual checklist)

**Interfaces:**
- Consumes: nothing. Documentation only.
- Produces: nothing code reads.

- [ ] **Step 1: Update the SPEC.md shell description**

In `SPEC.md`, replace the `# Top Bar` section (lines 5-7) with:

```markdown
# Top Bar
The top bar is 58px tall: the GRID logo and wordmark on the left, a centred pill
group with the five views (Library, Server, Downloads, Emulators, Settings) in that
order, and on the right a connection status dot, the server menu (Reconnect,
Disconnect, Open RomM in browser) and the app-update badge when a release notice is
stored. `Ctrl+1`..`Ctrl+5` switch views from the keyboard.

Views stay mounted and switch with the `hidden` attribute, so scroll positions and
selections survive a switch. Behind the content sits blurred background art — the
cover of the last game viewed, at the opacity set in Settings › Appearance.

A 28px download strip sits at the bottom of the window. It is hidden while nothing is
transferring; otherwise it reads `⬇ <title> · <percent> · <speed>` and opens the
Downloads view when clicked.
```

and append this bullet to the `# Main Sections` list, after the Settings bullet (line 24):

```markdown
- **Appearance** (under Settings) chooses the theme — follow the OS, dark, or light —
  and the background-art fade from 0 to 60 percent. Both are stored under `[ui]` in
  `config.toml` as `theme` and `background_fade`.
```

- [ ] **Step 2: Add the manual checklist rows**

In `rewrite/README.md`, append to the "Residual manual checklist" list (after the "Explicit (none)" bullet at line 137):

```markdown
- **Theme override**: with the OS in dark mode, set Settings › Appearance › Theme to
  Light, confirm the shell repaints immediately and stays light after a relaunch;
  set it back to Follow system and confirm it tracks an OS theme change live.
- **Background art**: hover a Library card for more than half a second and confirm the
  blurred art behind the content cross-fades to that cover; drag the fade slider and
  confirm the art responds while dragging and the value survives a relaunch.
- **Server menu**: open the server name menu and confirm "Open RomM in browser" opens
  the configured server. With a basic-auth server URL, confirm the menu item does
  nothing rather than opening a URL carrying the password.
```

- [ ] **Step 3: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add SPEC.md rewrite/README.md
git commit -m "rewrite: document the redesigned desktop shell and appearance settings"
```

---

## Self-review notes

Checked against the spec after writing:

- **§3 Shell** — 58px bar, logo, centred pills, status dot, server menu, update badge: Task 3. `Ctrl+1..5`: Task 3 (`viewForDigit`). Background art with the 500ms dwell, 40px blur, fade and 360ms cross-fade: Task 5. 28px footer strip with the exact text format and "Open Downloads": Task 4. Banner removal: Task 6. `hidden`-switched views: Task 3. **`Ctrl+F` is deliberately not implemented** — no view owns a search box until plan 2 (Library/Server toolbars) and plan 4; it belongs to the task that adds the first one.
- **§4 Theme tokens** — every listed colour, the type scale, the 4px spacing note, the five radii and the three durations: Task 2's `app.css`. Resolution via `prefers-color-scheme` with a `ui.theme` override: Tasks 1 and 2.
- **§10 Settings, Appearance only** — rail with all five entries, theme select, fade slider with live preview: Task 6. The four unbuilt pages render the verbatim placeholder line.
- **§11 Test ids** — every rename lands in the task that makes it (Task 3 for the emulators trio, Task 4 for the drawer, Task 6 for the banner). `nav-settings`, `settings-nav-<page>`, `theme-select` are new here; `emu-nav-<page>`, `library-rail-<key>`, `server-rail-<id>`, `details-tab-<name>`, `media-viewer`, `downloads-seg-<name>`, `download-graph-<id>` belong to plans 2–5.
- **§13 Out of scope** — nothing here touches TV mode, collections, achievements, notes, controller navigation, or core downloads.
- **Type consistency** — `ThemeChoice`/`ResolvedTheme` (Task 2) are the same names Task 6 imports; `uiSettings.backgroundFade` is read identically by `BackgroundArt.svelte` (Task 5) and `Settings.svelte` (Task 6); `View`/`VIEWS`/`viewForDigit` (Task 3) are the only navigation vocabulary; `footerLine` (Task 4) is the strip's single formatter; `noteViewed`/`seedLastViewed` (Task 5) are used with exactly those names in Library, Server and Shell.
