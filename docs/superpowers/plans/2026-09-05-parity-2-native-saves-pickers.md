# Parity 2 — native PC games, native pickers, install-blocked reasons

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the PC (native Windows/Linux) game experience in the details popup to parity with the Python app, add native file/folder pickers everywhere the Python app had them, surface install-blocked reasons as tooltips, and stop claiming an emulator launch target for a native game.

**Architecture:** Three independent seams, sequenced so each one lands with its own tests.
(1) **Display seam** — the details popup's launch-target line, the Native Game Settings dialog's missing rows/strings, and the cloud panel's native save-location strings. Every string is ruled verbatim from the Python original and lives in a pure TypeScript module (`details/nativePaths.ts`, `details/blocked.ts`) with vitest coverage; the `.svelte` file is a dumb renderer, because this repo has no component test harness.
(2) **Backend seam** — the native save-path list gains a persisted per-title suppression list (a new config key), each row gains its host-resolved expanded path, and the long-standing `wine_prefix: None` bug in both `CloudContext` construction sites is fixed by threading the game's `native_wineprefix` through. All three are Rust-testable.
(3) **Picker seam** — `tauri-plugin-dialog` is added once and wrapped in one tiny module, `app/src/lib/pickers.ts`. No component ever imports the plugin directly. Every `Browse…` button is **additive**: the existing text input stays, keeps its test id and remains the path E2E drives, so no spec needs a real OS dialog.

**Tech Stack:** Rust (grid-core + the Tauri `app` crate), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/porting/06-cloud-saves.md` ("Restore — native games", "Upload — native games", "Block reasons", "Rust port deviations (milestone 6)") and `docs/porting/03-library-install.md` (§12/§13 content block reasons, "Rust port deviations") are the behaviour specs for everything here and are updated by Task 9 where the rewrite now deliberately differs. The parity matrix in the 2026-09-05 research pass names the gaps: section A gaps **4**, **9**, **15** (NativeSettings strings only), the whole of section B (**N1–N14**), and the controller ruling on the launch-target line.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## User decisions / rulings (binding)

1. **Hide "No default emulator" for native games** (user instruction 2026-09-05). A native game launches its own executable through a compat tool, never an emulator entry, so the details popup must render no launch-target line at all for a platform whose name starts with `windows` or `linux` (case-insensitive, trimmed).
2. **Persisted PCGW suppression, not session-only.** Python's remove button (`_pcgw_remove_path_for_game`, `details_view_mixin.py:1218-1230`) only mutates the in-memory `_pcgw_paths_cache` for PCGW rows, so a removed PCGW path returns on the next lookup. The rewrite persists the removal in a new config key. This is a deliberate improvement and is recorded in `docs/porting/06-cloud-saves.md` by Task 9.
3. **Deferred, NOT in this plan:** "Update from Source" for installed emulators (gap 3); the `Enable debug prints` toggle (gap 13); the Windows Documents-redirection resolver (native gap 15); Eden `prod.keys`/firmware file checks. The five per-emulator cloud fields (gap 1), RA login (gap 2), the global toast surface (gap 5), per-emulator notes (gap 6), editable connection (gap 7), Open Config Folder (gap 10), standalone emulator Launch (gap 11), the Connect library-path field (gap 12) and window geometry (gap 14) belong to other plans.
4. **Browse buttons are additive.** Task 8 adds a `Browse…` button next to each existing text input. The input keeps its test id and its behaviour. No existing E2E flow may change.
5. **The Connect library-path Browse button assumes another plan lands first.** Task 8 Step 5 assumes gap 12's plan has added a text input with test id `connect-library-path` to `app/src/lib/Connect.svelte`. If that input does not exist when this plan runs, that step is **skipped** and reported as skipped — it must not create the field itself.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. Nothing in this plan touches a token, and nothing added here may print a config file verbatim — a real `config.toml` on the dev machine sits next to credentials.
- **Only `app.css` tokens for colours**; `--m-*` motion tokens for animation. The files this plan edits still carry raw literals (`#e5484d` in `details/CloudPanel.svelte:437,495,592-593` and `details/NativeSettings.svelte:272,295`; `#e5a53a` in `CloudPanel.svelte:447`). Every one of those literals that this plan's tasks touch is replaced with `var(--danger)` / `var(--warning)` — both tokens exist at `app/src/app.css:26-27` and are not re-declared per theme, so they are safe in light and dark.
- **Every test id E2E asserts today stays:** `details-install`, `details-game-settings`, `native-settings`, `native-settings-exe`, `native-settings-params`, `native-settings-save`, `native-settings-prefix`, `cloud-panel`, `cloud-upload`, `cloud-native-path-input`, `cloud-native-path-add`, every `cloud-record-*`/`cloud-restore-*`/`cloud-delete-*` id, `library-path-input`, `library-path-save`, `emu-form-path`. `data-testid="details-emulator"` is *conditionally rendered* by Task 1 rather than removed; Task 1 Step 1 greps to prove no spec asserts it exists for a native game.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` (record the warning count in Task 1 Step 0 as the baseline; no NEW warnings after that) and `npx vitest run` green. Then a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted pure module with vitest tests, plus `npm run check`, plus E2E. Never fabricate a component test.
- The final task runs the E2E groups `native`, `cloud-saves`, `install`, `library`, `connect` (`rewrite/scripts/e2e.sh native cloud-saves install library connect`, detached, log to a file) and they must be green.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/details/cloud.ts` | new `isNativeLaunchPlatform` (windows OR linux), display-only |
| `app/src/lib/details/header.ts` | `launchTargetLine` returns `''` for a native platform |
| `app/src/lib/details/header.test.ts`, `details/cloud.test.ts` | tests for both |
| `app/src/lib/Details.svelte` | conditional `details-emulator` line; install-blocked `title`s; content-button `title`s |
| `app/src-tauri/src/commands/specials.rs` | `NativeGameSettings.install_dir`; new `install_block_reason` command |
| `crates/grid-core/src/launch/selection.rs` | new pure `install_block_reason` |
| `app/src/lib/details/NativeSettings.svelte` | Install Directory row, Custom Launch Parameters section + hint, wine-prefix row hidden on a Windows host, colour tokens |
| `app/src/lib/details/nativePaths.ts` (+ `.test.ts`) | new pure module: native save-location status line, empty label, upload tooltip/enablement |
| `app/src/lib/details/CloudPanel.svelte` | native save-location strings, expanded-path tooltips, remove button on EVERY row, Browse…, colour tokens |
| `crates/grid-core/src/config.rs` | new `native_removed_save_paths` config key + round-trip test |
| `app/src-tauri/src/cloud_service.rs` | expanded-path DTO, suppression list, dedupe, `wine_prefix` threading (N14) |
| `app/src-tauri/src/commands/cloud.rs`, `app/src-tauri/src/lib.rs` | command signature + registration |
| `app/src/lib/api.ts` | `NativeSavePaths` entry shape, `nativeRemoveSavePath`, `installBlockReason`, `NativeGameSettings.install_dir` |
| `app/src/lib/details/blocked.ts` (+ `.test.ts`) | new pure module: PS4 / Xbox 360 content block reasons |
| `app/src/lib/pickers.ts` | `pickFolder` / `pickFile` wrappers over `@tauri-apps/plugin-dialog` |
| `app/src-tauri/Cargo.toml`, `app/package.json`, `app/src-tauri/capabilities/default.json` | dialog plugin + `dialog:allow-open` |
| `app/src/lib/Server.svelte`, `app/src/lib/emulators/EmulatorForm.svelte`, `app/src/lib/Connect.svelte` | `Browse…` buttons |
| `docs/porting/06-cloud-saves.md`, `docs/porting/03-library-install.md` | behaviour docs updated |
| `e2e/specs/native.spec.ts` | new native save-location + launch-target-line cases |

---

### Task 1: Hide the launch-target line for native platforms

**Files:**
- Modify: `app/src/lib/details/cloud.ts` (add after `isNativeExecutablePlatform`, `:91-93`)
- Modify: `app/src/lib/details/header.ts:6` (import) and `:98-107` (`launchTargetLine`)
- Modify: `app/src/lib/details/cloud.test.ts:163-179`
- Modify: `app/src/lib/details/header.test.ts:127-153`
- Modify: `app/src/lib/Details.svelte:594-596`

**Interfaces:**
- Produces: `export function isNativeLaunchPlatform(platform: string): boolean` in `details/cloud.ts`.
- Changes: `launchTargetLine(defaults: LaunchDefaults | null, platformName: string): string` now returns `''` for a native platform. `''` means "render no line".
- Consumes: existing `savedDefaultFor`, `isRetroarchName`, `NO_EMULATOR_MARKER` from `emulators/defaults.ts` (`app/src/lib/emulators/defaults.ts:14,36`).

- [ ] **Step 0 (baseline):** from `app/`, run `npm run check` and record the exact warning count and the files they name in the commit message body. That count is the "no new warnings" baseline for every later task.

- [ ] **Step 1: Prove no spec depends on the line existing for a native game:** `grep -rn "details-emulator" e2e/specs app/src` — expected: `app/src/lib/Details.svelte` only. If a spec reads it, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: Write the failing predicate tests** in `app/src/lib/details/cloud.test.ts` — add `isNativeLaunchPlatform` to the import list at the top, then append:

```ts
describe('isNativeLaunchPlatform', () => {
  it('accepts Windows and Linux platform names case-insensitively', () => {
    expect(isNativeLaunchPlatform('Windows')).toBe(true);
    expect(isNativeLaunchPlatform('  WINDOWS  ')).toBe(true);
    expect(isNativeLaunchPlatform('Linux')).toBe(true);
    expect(isNativeLaunchPlatform('linux')).toBe(true);
    expect(isNativeLaunchPlatform('Windows PC')).toBe(true);
  });

  it('rejects emulated platforms', () => {
    expect(isNativeLaunchPlatform('SNES')).toBe(false);
    expect(isNativeLaunchPlatform('Not Windows')).toBe(false);
    expect(isNativeLaunchPlatform('')).toBe(false);
  });

  // The scope predicate mirrors grid-core and must stay windows-only.
  it('is wider than isNativeExecutablePlatform, which stays windows-only', () => {
    expect(isNativeExecutablePlatform('Linux')).toBe(false);
    expect(isNativeLaunchPlatform('Linux')).toBe(true);
  });
});
```

- [ ] **Step 3: Write the failing line tests** in `app/src/lib/details/header.test.ts`, inside the existing `describe('launchTargetLine', ...)` block:

```ts
  it('renders no line for a native Windows game', () => {
    expect(launchTargetLine(defaults({ windows: 'Wine' }), 'Windows')).toBe('');
  });

  it('renders no line for a native Linux game', () => {
    expect(launchTargetLine(null, 'Linux')).toBe('');
  });

  it('still names the emulator for an emulated platform', () => {
    expect(launchTargetLine(defaults({ snes: 'Snes9x' }), 'SNES')).toBe('Snes9x');
  });
```

- [ ] **Step 4: Run** `npx vitest run details` — the new cases fail (`isNativeLaunchPlatform` is not exported).

- [ ] **Step 5: Implement the predicate** in `details/cloud.ts`, directly under `isNativeExecutablePlatform`:

```ts
/**
 * Whether `platform` names a platform whose games run as native HOST
 * executables — trimmed, case-folded, starting with "windows" or "linux".
 *
 * Deliberately NOT the same function as [`isNativeExecutablePlatform`]
 * above. That one is a mirror of grid-core's
 * `cloud::scope::is_native_executable_platform`
 * (`crates/grid-core/src/cloud/scope.rs:68`), which is windows-only and
 * decides the cloud save SCOPE and BLOCK REASONS the backend computes;
 * widening it would desync the panel from the answers the backend gives it.
 * This one is display-only: it decides whether the details popup claims an
 * emulator launch target at all (user ruling 2026-09-05 — a native game
 * launches its own executable through a compat tool, so "No default
 * emulator" was never a true statement about it).
 */
export function isNativeLaunchPlatform(platform: string): boolean {
  const folded = platform.trim().toLowerCase();
  return folded.startsWith('windows') || folded.startsWith('linux');
}
```

- [ ] **Step 6: Implement the line change** in `details/header.ts`. Add to the imports at `:5-6`:

```ts
import { isNativeLaunchPlatform } from './cloud';
```

(`details/cloud.ts` imports only types from `../api`, so this introduces no cycle — verified at `app/src/lib/details/cloud.ts:11`.)

Then make `launchTargetLine` start with:

```ts
export function launchTargetLine(defaults: LaunchDefaults | null, platformName: string): string {
  // User ruling 2026-09-05: a native (Windows/Linux) game launches its own
  // executable through a compat tool, never an emulator entry, so the popup
  // must not claim a launch target for one. `''` means "render no line" —
  // Details.svelte drops the whole <p> rather than showing an empty row.
  if (isNativeLaunchPlatform(platformName)) return '';
  const name = savedDefaultFor(defaults?.default_emulators, platformName).trim();
  if (name === '' || name === NO_EMULATOR_MARKER) return 'No default emulator';
  if (!isRetroarchName(name)) return name;
  const cores = defaults?.retroarch_cores ?? {};
  const folded = platformName.trim().toLowerCase();
  const key = Object.keys(cores).find((k) => k.toLowerCase() === folded);
  const core = (key ? cores[key] : '').trim();
  return core === '' ? `${name} · no core` : `${name} · ${core}`;
}
```

Update the function's doc comment (`:91-97`) to end with: "Returns `''` for a native platform (`isNativeLaunchPlatform`), which the caller renders as no line at all."

- [ ] **Step 7: Render conditionally** in `app/src/lib/Details.svelte`. Add next to the other derived values (after `let isNative = ...` at `:257`):

```svelte
  // `''` from `launchTargetLine` means "no launch target to state" — today
  // only a native platform, whose game runs its own executable.
  let launchTarget = $derived(launchTargetLine(launchDefaults, subject.platformName));
```

and replace `:594-596` with:

```svelte
        {#if launchTarget !== ''}
          <p class="meta-line" data-testid="details-emulator">{launchTarget}</p>
        {/if}
```

- [ ] **Step 8: Run** `npx vitest run` (green) and, from `app/`, `npm run check` (no new warnings).

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/details/cloud.ts app/src/lib/details/cloud.test.ts app/src/lib/details/header.ts app/src/lib/details/header.test.ts app/src/lib/Details.svelte
git commit -m "rewrite: do not claim an emulator launch target for a native game"
```

---

### Task 2: Native Game Settings — install directory, section title, hint, wine-prefix row

**Files:**
- Modify: `app/src-tauri/src/commands/specials.rs:88-139` (`NativeGameSettings` + `native_game_settings`)
- Modify: `app/src/lib/api.ts` (the `NativeGameSettings` type)
- Modify: `app/src/lib/details/NativeSettings.svelte:44, 111-180, 264-298`

**Interfaces:**
- Produces: `NativeGameSettings` gains `pub install_dir: String` — the game's install directory as a full path, `""` when none resolves. Frontend type gains `install_dir: string`.
- Consumes: `grid_core::library::specials::native::install_dir(&row, &archives)` (already called at `specials.rs:122` and today discarded).
- Consumes: `installDirOf(candidates)` and `isWindowsHost(navigator.platform)` from `details/actions.ts:112,140` (unchanged).

Python anchors, ported verbatim: `grid_launcher/ui/dialogs.py:211-214` (Install Directory row and the relative executable labels), `:249-251` (wine-prefix row is built only when `sys.platform != "win32"`, placeholder `"(will be created at install)"`, label `"Wine Prefix (read-only)"`), `:255-275` (`"Custom Launch Parameters"` section title, `"Parameters"` field label, hint `"Arguments are optional and appended when launching this game."`).

- [ ] **Step 1: Write the failing Rust test** in `app/src-tauri/src/commands/specials.rs`'s test module (create `#[cfg(test)] mod tests` at the end of the file if none exists; the file's other unit test target is `normalize_compat_for_host`). The command itself needs app state, so the test pins the DTO shape instead:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_game_settings_carries_the_install_directory() {
        let dto = NativeGameSettings {
            executable: "/games/My Game/game/MyGame/mygame.exe".to_string(),
            parameters: "--fullscreen".to_string(),
            compat_tool: "wine".to_string(),
            wineprefix: "/games/My Game/prefix".to_string(),
            install_dir: "/games/My Game/game".to_string(),
            candidates: vec!["/games/My Game/game/MyGame/mygame.exe".to_string()],
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["install_dir"], "/games/My Game/game");
    }

    #[test]
    fn compat_tool_is_dropped_on_a_windows_host() {
        assert_eq!(normalize_compat_for_host("Windows", "GE-Proton"), "");
        assert_eq!(normalize_compat_for_host("Linux", "GE-Proton"), "GE-Proton");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p app specials::` — the first test fails to compile (no `install_dir` field).

- [ ] **Step 3: Implement the DTO field.** In `specials.rs`, add to `NativeGameSettings` after `wineprefix`:

```rust
    /// The game's install directory (`native::install_dir`), as a full path;
    /// `""` when neither a live extracted directory nor a candidate archive
    /// resolves one. Read-only display: the dialog shows it as its own row
    /// and labels every executable candidate relative to it
    /// (`grid_launcher/ui/dialogs.py:211-214`).
    pub install_dir: String,
```

and rewrite the body of the `spawn_blocking` closure in `native_game_settings` from `:122` so the resolved directory is kept:

```rust
        let dir = native::install_dir(&row, &archives);
        let candidates = match dir.as_ref() {
            Some(dir) => native::executable_candidates(dir),
            None => Vec::new(),
        };
        let executable = native::resolved_executable(&row, &candidates)
            .map(path_string)
            .unwrap_or_default();
        Ok(NativeGameSettings {
            executable,
            parameters: row.native_launch_parameters.clone(),
            compat_tool: row.native_compat_tool.clone(),
            wineprefix: row.native_wineprefix.clone(),
            install_dir: dir.map(path_string).unwrap_or_default(),
            candidates: candidates.iter().map(|p| path_string(p.clone())).collect(),
        })
```

- [ ] **Step 4: Run** `cargo test -p app specials::` — green. Run `cargo fmt` and both clippy commands.

- [ ] **Step 5: Extend the frontend type** in `app/src/lib/api.ts`: add `install_dir: string;` to the `NativeGameSettings` type, directly after `wineprefix`, with the comment `/** The install directory the executable candidates are labelled relative to; '' when none resolved. */`.

- [ ] **Step 6: Implement the dialog changes** in `app/src/lib/details/NativeSettings.svelte`.

Replace the `installDir` derived at `:44` with:

```svelte
  // The backend's own answer when it has one; the shallowest candidate's
  // directory otherwise, so a row built before `install_dir` existed still
  // labels its options (`installDirOf`, details/actions.ts).
  let installDir = $derived(settings?.install_dir || installDirOf(settings?.candidates ?? []));
```

Insert the Install Directory row immediately before the Executable `<label>` (currently `:133`):

```svelte
        <div class="row">
          <span class="row-label">Install Directory</span>
          <p data-testid="native-settings-install-dir" class="row-value">{installDir || '(not found)'}</p>
        </div>
```

Wrap the parameters field in its section (replacing `:143-146`):

```svelte
      <div class="section">
        <h4 class="section-title">Custom Launch Parameters</h4>
        <label>
          Parameters
          <input data-testid="native-settings-params" bind:value={parameters} />
        </label>
        <p data-testid="native-settings-params-hint" class="hint">
          Arguments are optional and appended when launching this game.
        </p>
      </div>
```

Gate the wine-prefix row on the host and change the placeholder (replacing `:161-164`):

```svelte
      <!-- dialogs.py:249-251: the prefix row is built ONLY on a non-Windows
           host — a Windows host runs the .exe directly and has no prefix —
           and reads "(will be created at install)" before one exists. -->
      {#if !windowsHost}
        <div class="row">
          <span class="row-label">Wine Prefix (read-only)</span>
          <p data-testid="native-settings-prefix" class="row-value">
            {settings.wineprefix || '(will be created at install)'}
          </p>
        </div>
      {/if}
```

- [ ] **Step 7: CSS.** Rename the `.prefix`/`.prefix-label`/`.prefix-value` rules at `:276-292` to `.row`/`.row-label`/`.row-value` (same declarations), and add:

```css
  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }

  .section-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-h);
  }
```

Replace the two raw colour literals in this file: `.error-hint` (`:272`) and `.error` (`:295`) become `color: var(--danger);`.

- [ ] **Step 8: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings. `native-settings-prefix` still exists on Linux (the E2E host), so nothing regresses there.

- [ ] **Step 9: Commit**

```bash
git add app/src-tauri/src/commands/specials.rs app/src/lib/api.ts app/src/lib/details/NativeSettings.svelte
git commit -m "rewrite: show the install directory, parameters section and host-correct wine prefix in Game Settings"
```

---

### Task 3: Native save-location strings — a pure module (N3–N7)

**Files:**
- Create: `app/src/lib/details/nativePaths.ts`
- Create: `app/src/lib/details/nativePaths.test.ts`
- Modify: `app/src/lib/details/CloudPanel.svelte:129-145, 246-247, 316-346, 434-448`

**Interfaces:**
- Produces, in `details/nativePaths.ts`:
  - `export type NativePathsPhase = 'loading' | 'loaded';`
  - `export function nativePathsStatusLine(phase: NativePathsPhase, count: number): string`
  - `export function nativePathsEmptyLabel(phase: NativePathsPhase): string`
  - `export function nativeUploadTooltip(phase: NativePathsPhase, count: number, hasRomId: boolean): string`
  - `export function nativeUploadEnabled(phase: NativePathsPhase, count: number, hasRomId: boolean, pending: boolean): boolean`
- Consumes: nothing (no imports). `CloudPanel.svelte` deletes its inline `nativeUploadEnabled`/`nativeUploadTooltip` deriveds and calls these instead.

Python anchor: `_refresh_native_save_panel`, `grid_launcher/ui/mixins/details_view_mixin.py:1143-1185`.

- [ ] **Step 1: Write the failing tests** in `app/src/lib/details/nativePaths.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  nativePathsEmptyLabel,
  nativePathsStatusLine,
  nativeUploadEnabled,
  nativeUploadTooltip,
} from './nativePaths';

describe('nativePathsStatusLine', () => {
  it('names PCGamingWiki while the lookup is running', () => {
    expect(nativePathsStatusLine('loading', 0)).toBe('Looking up save locations on PCGamingWiki…');
  });

  it('names PCGamingWiki when the lookup found nothing', () => {
    expect(nativePathsStatusLine('loaded', 0)).toBe('No save locations found on PCGamingWiki.');
  });

  it('counts the configured locations, keeping the original "(s)" wording', () => {
    expect(nativePathsStatusLine('loaded', 1)).toBe('1 save location(s) configured.');
    expect(nativePathsStatusLine('loaded', 3)).toBe('3 save location(s) configured.');
  });
});

describe('nativePathsEmptyLabel', () => {
  it('says what is being fetched while loading', () => {
    expect(nativePathsEmptyLabel('loading')).toBe('Fetching save locations from PCGamingWiki…');
  });

  it('is blank once loaded, because the list itself is shown', () => {
    expect(nativePathsEmptyLabel('loaded')).toBe('');
  });
});

describe('nativeUploadTooltip', () => {
  it('explains the wait while the lookup runs', () => {
    expect(nativeUploadTooltip('loading', 0, true)).toBe('Waiting for save location lookup…');
  });

  it('asks for a location when there are none', () => {
    expect(nativeUploadTooltip('loaded', 0, true)).toBe('Add a save location to enable uploads.');
  });

  it('names the missing rom id ahead of the happy path', () => {
    expect(nativeUploadTooltip('loaded', 2, false)).toBe('Missing ROM id for this game.');
  });

  it('describes the upload when everything is in place', () => {
    expect(nativeUploadTooltip('loaded', 2, true)).toBe('Upload save files from the listed locations.');
  });
});

describe('nativeUploadEnabled', () => {
  it('is disabled while loading, while pending, with no paths, and with no rom id', () => {
    expect(nativeUploadEnabled('loading', 2, true, false)).toBe(false);
    expect(nativeUploadEnabled('loaded', 2, true, true)).toBe(false);
    expect(nativeUploadEnabled('loaded', 0, true, false)).toBe(false);
    expect(nativeUploadEnabled('loaded', 2, false, false)).toBe(false);
  });

  it('is enabled with at least one path, a rom id and nothing in flight', () => {
    expect(nativeUploadEnabled('loaded', 1, true, false)).toBe(true);
  });
});
```

- [ ] **Step 2: Run** `npx vitest run nativePaths` — fails (module missing).

- [ ] **Step 3: Implement** `app/src/lib/details/nativePaths.ts`:

```ts
// Pure strings and enablement rules for the details cloud panel's native
// (PC game) save-location section. No API/store imports so this stays
// trivially unit-testable — CloudPanel.svelte owns the fetching.
//
// Every string here is ruled VERBATIM from `_refresh_native_save_panel`
// (grid_launcher/ui/mixins/details_view_mixin.py:1143-1185), including the
// "(s)" plural form and the ellipsis character, which the Python original
// spells as U+2026 in the two lookup messages.

/** Whether the PCGamingWiki lookup for this game has answered yet. */
export type NativePathsPhase = 'loading' | 'loaded';

/**
 * The section's status line (`details_cloud_status_label`, :1160/:1174/:1178).
 * `count` is PCGW rows plus manual rows, after de-duplication.
 */
export function nativePathsStatusLine(phase: NativePathsPhase, count: number): string {
  if (phase === 'loading') return 'Looking up save locations on PCGamingWiki…';
  if (count <= 0) return 'No save locations found on PCGamingWiki.';
  return `${count} save location(s) configured.`;
}

/**
 * The placeholder shown where the list will be (`details_cloud_empty_label`,
 * :1163). `''` once loaded: the list — or the status line's own
 * "No save locations found on PCGamingWiki." — says it instead.
 */
export function nativePathsEmptyLabel(phase: NativePathsPhase): string {
  return phase === 'loading' ? 'Fetching save locations from PCGamingWiki…' : '';
}

/**
 * The upload button's tooltip (:1162, :1176, :1181-1183). Order matters:
 * the lookup, then "no paths", then the missing rom id, then the happy path.
 */
export function nativeUploadTooltip(
  phase: NativePathsPhase,
  count: number,
  hasRomId: boolean
): string {
  if (phase === 'loading') return 'Waiting for save location lookup…';
  if (count <= 0) return 'Add a save location to enable uploads.';
  return hasRomId ? 'Upload save files from the listed locations.' : 'Missing ROM id for this game.';
}

/** The upload button's enablement, the same four gates as the tooltip (:1161-1180). */
export function nativeUploadEnabled(
  phase: NativePathsPhase,
  count: number,
  hasRomId: boolean,
  pending: boolean
): boolean {
  return phase === 'loaded' && count > 0 && hasRomId && !pending;
}
```

- [ ] **Step 4: Run** `npx vitest run nativePaths` — green.

- [ ] **Step 5: Wire CloudPanel.svelte.** Add to the imports at `:11-22`:

```ts
  import {
    nativePathsEmptyLabel,
    nativePathsStatusLine,
    nativeUploadEnabled,
    nativeUploadTooltip,
    type NativePathsPhase,
  } from './nativePaths';
```

Replace the inline block at `:129-145` with:

```svelte
  // The lookup is "loading" until the first `native_save_paths` answer
  // lands; a failed PCGW fetch still answers (with an empty pcgw list), so
  // this never sticks (`pcgw_paths_for_title`, cloud_service.rs:206-216).
  let nativePhase = $derived<NativePathsPhase>(
    nativePathsLoading || nativePaths === null ? 'loading' : 'loaded'
  );
  let nativePathCount = $derived((nativePaths?.pcgw.length ?? 0) + (nativePaths?.manual.length ?? 0));
  let nativeUploadIsEnabled = $derived(
    nativeUploadEnabled(nativePhase, nativePathCount, game.rom_id !== null, uploadPending)
  );
  let nativeUploadHint = $derived(
    nativeUploadTooltip(nativePhase, nativePathCount, game.rom_id !== null)
  );

  let uploadEnabled = $derived(
    nativeSave ? nativeUploadIsEnabled : !nativeStateBlocked && panelInfo.supported && !uploadPending
  );
  let uploadTooltip = $derived(
    nativeSave ? nativeUploadHint : panelInfo.supported ? '' : panelInfo.block_reason
  );
```

- [ ] **Step 6: N7 — the state message gains its second line.** Replace `:246-247` with:

```svelte
  {#if nativeStateBlocked}
    <p data-testid="cloud-native-states-unsupported" class="hint">Save states are not supported for native games.</p>
    <p data-testid="cloud-native-states-note" class="hint">Only save file backups are supported for native games.</p>
```

- [ ] **Step 7: N3/N4/N5 — the section's status line and empty label.** Replace the `{#if nativePathsLoading} … {/if}` block at `:320-346` with:

```svelte
          <p data-testid="cloud-native-status" class="hint">
            {nativePathsStatusLine(nativePhase, nativePathCount)}
          </p>
          {#if nativePhase === 'loading'}
            <p data-testid="cloud-native-fetching" class="hint">{nativePathsEmptyLabel(nativePhase)}</p>
          {:else if nativePaths}
            <ul class="path-list">
              {#each nativePaths.pcgw as path (path)}
                <li data-testid={`cloud-native-path-pcgw-${path}`}>{path}</li>
              {/each}
              {#each nativePaths.manual as path (path)}
                <li data-testid={`cloud-native-path-manual-${path}`}>
                  <span>{path}</span>
                  <button
                    data-testid={`cloud-native-path-remove-${path}`}
                    class="remove"
                    disabled={manualPathPending}
                    onclick={() => handleRemoveManualPath(path)}
                    aria-label={`Remove ${path}`}
                  >
                    ×
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
```

(The list itself becomes entry-shaped and grows a remove button on both loops in Task 5 — this step only lands the strings and keeps the file compiling.)

- [ ] **Step 8: Colour tokens.** In this file's `<style>`: `.error` (`:437`) and `.remove` (`:495`) become `color: var(--danger);`; `.message.warn` (`:447`) becomes `color: var(--warning);`; `.record-actions button.danger` (`:592-593`) becomes `color: var(--danger); border-color: var(--danger);`.

- [ ] **Step 9: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 10: Commit**

```bash
git add app/src/lib/details/nativePaths.ts app/src/lib/details/nativePaths.test.ts app/src/lib/details/CloudPanel.svelte
git commit -m "rewrite: restore the native save-location status, lookup and upload-tooltip strings"
```

---

### Task 4: Backend — suppression list, expanded paths, and the wine-prefix bug (N1, N2, N14)

**Files:**
- Modify: `crates/grid-core/src/config.rs:186-190` (new key) and its test module (new round-trip test)
- Modify: `app/src-tauri/src/cloud_service.rs:450-525` (native path methods), `:885-901` and `:999-1015` (`CloudContext` sites), `:1436-1440` (DTO), test module
- Modify: `app/src-tauri/src/commands/cloud.rs:116-149`
- Modify: `app/src-tauri/src/lib.rs:313-315`

**Interfaces:**
- Produces: `Config.native_removed_save_paths: BTreeMap<String, Vec<String>>` (`#[serde(default)]`), keyed exactly like `native_manual_save_paths` (`grid_core::cloud::ops::native::manual_paths_key`).
- Produces:
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct NativeSavePathEntryDto { pub raw: String, pub expanded: String }
  #[derive(Debug, Clone, Serialize)]
  pub struct NativeSavePathsDto {
      pub pcgw: Vec<NativeSavePathEntryDto>,
      pub manual: Vec<NativeSavePathEntryDto>,
  }
  ```
- Produces: `CloudService::native_save_paths(&self, install: Arc<InstallService>, config_path: &Path, game: CloudGameInput) -> Result<NativeSavePathsDto, String>` (new first argument).
- Produces: `CloudService::native_remove_save_path(&self, config_path: &Path, game: CloudGameInput, path: String) -> Result<(), String>` (renamed from `native_remove_manual_save_path`).
- Produces: `Inputs::wine_prefix_for(&self, game: &CloudGame) -> Option<PathBuf>` and `Inputs::context<'a>(&'a self, pcgw_paths: &'a [String], wine_prefix: Option<&'a Path>) -> CloudContext<'a>` (new second argument).
- Produces: `CloudService::run_auto_upload` gains a `wine_prefix: Option<PathBuf>` parameter.
- Consumes: `grid_core::cloud::native::{resolve_native_save_dir, normalize_manual_save_path}` (`crates/grid-core/src/cloud/native.rs:211,272`), `grid_core::cloud::state::games_match_identity`, `InstalledGame.native_wineprefix` (`crates/grid-core/src/library/registry.rs:210`).

- [ ] **Step 1: Write the failing config round-trip test** in `crates/grid-core/src/config.rs`'s test module, modelled on `default_emulators_round_trip` (`:645`):

```rust
    #[test]
    fn native_removed_save_paths_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut removed = BTreeMap::new();
        removed.insert(
            "my game|windows".to_string(),
            vec!["%APPDATA%\\MyGame\\saves".to_string()],
        );
        let cfg = Config {
            native_removed_save_paths: removed,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.native_removed_save_paths.get("my game|windows"),
            Some(&vec!["%APPDATA%\\MyGame\\saves".to_string()])
        );
    }

    #[test]
    fn native_removed_save_paths_defaults_to_empty_for_an_older_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "schema_version = 1\n").unwrap();
        let cfg = Config::load(&path).unwrap();
        assert!(cfg.native_removed_save_paths.is_empty());
    }
```

- [ ] **Step 2: Run** `cargo test -p grid-core native_removed` — compile failure (no such field).

- [ ] **Step 3: Add the config key** in `config.rs`, immediately after `native_manual_save_paths` (`:190`):

```rust
    /// Save paths the user removed from a native game's save-location list,
    /// keyed exactly like `native_manual_save_paths`. Filtered out of the
    /// PCGamingWiki list every time it is read, so a removed PCGW row does
    /// not come back on the next lookup.
    ///
    /// Deliberate improvement over the reference, which mutated only the
    /// in-memory `_pcgw_paths_cache` (`_pcgw_remove_path_for_game`,
    /// details_view_mixin.py:1218-1230) and therefore forgot the removal as
    /// soon as the cache was rebuilt. Adding a path back through
    /// `native_add_manual_save_path` clears it from here, so a removal is
    /// never permanent.
    #[serde(default)]
    pub native_removed_save_paths: BTreeMap<String, Vec<String>>,
```

- [ ] **Step 4: Run** `cargo test -p grid-core config::` — green.

- [ ] **Step 5: Write the failing `cloud_service` tests** in `app/src-tauri/src/cloud_service.rs`'s test module (`:1450+`), next to `record_dtos_sort_newest_first_regardless_of_server_order`:

```rust
    #[test]
    fn visible_native_paths_drops_suppressed_rows_and_de_duplicates_manual_ones() {
        let pcgw = vec![
            "%APPDATA%\\Game\\saves".to_string(),
            "%USERPROFILE%\\Documents\\Game".to_string(),
        ];
        let manual = vec![
            // Same raw string as a PCGW row: the reference lists it once
            // (`_native_save_paths_for_game`, details_view_mixin.py:1060-1065).
            "%APPDATA%\\Game\\saves".to_string(),
            "D:\\Extra\\Saves".to_string(),
        ];
        let removed = vec!["%USERPROFILE%\\Documents\\Game".to_string()];

        let (visible_pcgw, visible_manual) = visible_native_paths(&pcgw, &manual, &removed);

        assert_eq!(visible_pcgw, vec!["%APPDATA%\\Game\\saves".to_string()]);
        assert_eq!(visible_manual, vec!["D:\\Extra\\Saves".to_string()]);
    }

    #[test]
    fn visible_native_paths_keeps_everything_when_nothing_is_suppressed() {
        let pcgw = vec!["%APPDATA%\\Game".to_string()];
        let manual = vec!["D:\\Saves".to_string()];
        let (visible_pcgw, visible_manual) = visible_native_paths(&pcgw, &manual, &[]);
        assert_eq!(visible_pcgw, pcgw);
        assert_eq!(visible_manual, manual);
    }

    #[test]
    fn wine_prefix_for_reads_the_matching_installed_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut row = InstalledGame::default();
        row.title = "My Game".to_string();
        row.platform = "Windows".to_string();
        row.rom_id = Some(701);
        row.native_wineprefix = "/library/Windows/My Game/prefix".to_string();

        let inputs = Inputs {
            config: Config::default(),
            profiles: &[],
            all_games: vec![cloud_game_from_installed(&row)],
            installed: vec![row],
            config_dir: dir.path().to_path_buf(),
            active_sessions: Vec::new(),
            now: 1_800_000_000.0,
        };

        let game = CloudGame {
            title: "My Game".to_string(),
            platform: "Windows".to_string(),
            rom_id: "701".to_string(),
            ..Default::default()
        };
        assert_eq!(
            inputs.wine_prefix_for(&game),
            Some(PathBuf::from("/library/Windows/My Game/prefix"))
        );

        let other = CloudGame {
            title: "Other".to_string(),
            platform: "Windows".to_string(),
            rom_id: "702".to_string(),
            ..Default::default()
        };
        assert_eq!(inputs.wine_prefix_for(&other), None);
    }

    #[test]
    fn context_threads_the_wine_prefix_into_the_cloud_context() {
        let dir = tempfile::tempdir().unwrap();
        let inputs = Inputs {
            config: Config::default(),
            profiles: &[],
            all_games: Vec::new(),
            installed: Vec::new(),
            config_dir: dir.path().to_path_buf(),
            active_sessions: Vec::new(),
            now: 1_800_000_000.0,
        };
        let pcgw: Vec<String> = Vec::new();
        let prefix = PathBuf::from("/prefix");
        let ctx = inputs.context(&pcgw, Some(prefix.as_path()));
        assert_eq!(ctx.wine_prefix, Some(prefix.as_path()));
    }
```

If `InstalledGame` does not implement `Default`, build the row with a struct literal listing every field instead — check with `grep -n "derive" -B2 -A2 "pub struct InstalledGame" crates/grid-core/src/library/registry.rs` before writing the test, and use whichever form compiles.

- [ ] **Step 6: Run** `cargo test -p app cloud_service::` — the four new tests fail to compile.

- [ ] **Step 7: Implement the DTO and the pure filter** in `cloud_service.rs`. Replace `NativeSavePathsDto` (`:1436-1440`) with:

```rust
/// One row of a native game's save-location list.
#[derive(Debug, Clone, Serialize)]
pub struct NativeSavePathEntryDto {
    /// The stored, unexpanded path (`%APPDATA%\Game\saves`). This is the
    /// row's label AND the value `native_remove_save_path` takes back.
    pub raw: String,
    /// `raw` resolved for THIS host and this game's wine prefix — the row's
    /// tooltip. Ports `os.path.expandvars(raw_path)`
    /// (details_view_mixin.py:1097), widened to the same
    /// `resolve_native_save_dir` the upload/restore paths use, so the
    /// tooltip states the directory that would really be read.
    pub expanded: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativeSavePathsDto {
    pub pcgw: Vec<NativeSavePathEntryDto>,
    pub manual: Vec<NativeSavePathEntryDto>,
}

/// The rows a native game's save-location list shows: `pcgw` minus every
/// suppressed path, then `manual` minus every suppressed path AND minus any
/// path the PCGW list already carries (`_native_save_paths_for_game`,
/// details_view_mixin.py:1060-1065, which appends only manual paths "not in
/// cached"). De-duplication matters now that both lists render a remove
/// button: two rows for one raw path would collide on their test ids and
/// would double the "N save location(s) configured." count.
fn visible_native_paths(
    pcgw: &[String],
    manual: &[String],
    removed: &[String],
) -> (Vec<String>, Vec<String>) {
    let visible_pcgw: Vec<String> = pcgw
        .iter()
        .filter(|p| !removed.iter().any(|r| r == *p))
        .cloned()
        .collect();
    let visible_manual: Vec<String> = manual
        .iter()
        .filter(|p| !removed.iter().any(|r| r == *p))
        .filter(|p| !visible_pcgw.iter().any(|q| q == *p))
        .cloned()
        .collect();
    (visible_pcgw, visible_manual)
}

/// Builds the DTO rows for `paths`, resolving each one for display.
fn native_path_entries(paths: &[String], wine_prefix: Option<&Path>) -> Vec<NativeSavePathEntryDto> {
    paths
        .iter()
        .map(|raw| NativeSavePathEntryDto {
            raw: raw.clone(),
            expanded: resolve_native_save_dir(raw, None, wine_prefix)
                .to_string_lossy()
                .into_owned(),
        })
        .collect()
}
```

Add `resolve_native_save_dir` to the existing `grid_core::cloud::native` import line and `use std::path::PathBuf;` if not already imported (`Path` is; check `:1-60`).

- [ ] **Step 8: Implement the wine-prefix lookup and thread it (N14).** In `impl Inputs` (`:995-1017`), replace `context` with:

```rust
    /// This game's wine prefix, from the registry row that matches it by
    /// identity. `None` when the game is not installed, or when the row has
    /// no prefix (a Windows host, or a game installed before prefixes were
    /// recorded).
    ///
    /// N14 fix: both `CloudContext` construction sites used to hardcode
    /// `wine_prefix: None`, so `ops/native.rs`'s
    /// `resolve_native_save_dir(raw, _, ctx.wine_prefix)` calls
    /// (`crates/grid-core/src/cloud/ops/native.rs:83,177,237,241`) never
    /// translated `%APPDATA%` and friends into the prefix on Linux — native
    /// upload and restore silently scanned host paths that do not exist.
    fn wine_prefix_for(&self, game: &CloudGame) -> Option<PathBuf> {
        self.installed
            .iter()
            .find(|row| games_match_identity(&cloud_game_from_installed(row), game))
            .map(|row| row.native_wineprefix.trim().to_string())
            .filter(|prefix| !prefix.is_empty())
            .map(PathBuf::from)
    }

    /// `pcgw_paths` is threaded in by the caller (Task 18: [`CloudService::cached_pcgw_paths`],
    /// a cache-only read keyed on the specific game's title — `Inputs`
    /// itself is built once per command with no single game in view, so it
    /// cannot resolve this on its own). `wine_prefix` is threaded in the
    /// same way, from [`Self::wine_prefix_for`].
    fn context<'a>(
        &'a self,
        pcgw_paths: &'a [String],
        wine_prefix: Option<&'a Path>,
    ) -> CloudContext<'a> {
        CloudContext {
            config: &self.config,
            profiles: self.profiles,
            all_games: &self.all_games,
            resolve_ctx: grid_core::cloud::dirs::ResolveContext {
                emulator_dir: None,
                library_dir: &self.config.library_path,
                config_dir: &self.config_dir,
                windows_documents: None,
            },
            active_sessions: &self.active_sessions,
            now: self.now,
            pcgw_paths,
            wine_prefix,
        }
    }
```

Update all seven `inputs.context(...)` call sites (`:278, 305, 347, 382, 601, 681, 1489`). Each of the first six has a `cloud_game` in scope, so each becomes the pair:

```rust
        let wine_prefix = inputs.wine_prefix_for(&cloud_game);
        let ctx = inputs.context(&pcgw_paths, wine_prefix.as_deref());
```

The test at `:1489` becomes `inputs.context(&pcgw, None)`.

- [ ] **Step 9: Fix the auto-upload site (N14, second half).** In `handle_session_finished` (`:757-843`), capture the prefix before `installed_game` is dropped:

```rust
        let wine_prefix = {
            let prefix = installed_game.native_wineprefix.trim();
            if prefix.is_empty() { None } else { Some(PathBuf::from(prefix)) }
        };
```

(place it right after `let cloud_game = cloud_game_from_installed(installed_game);` at `:772`), pass `wine_prefix` into the `run_auto_upload` call in the `pool.trigger` closure, add the parameter to `run_auto_upload`'s signature after `key: String`, and set `wine_prefix: wine_prefix.as_deref(),` in its `CloudContext` literal at `:900`.

- [ ] **Step 10: Implement the path methods.** Replace `cloud_service.rs:452-525` with:

```rust
    pub async fn native_save_paths(
        &self,
        install: Arc<InstallService>,
        config_path: &Path,
        game: CloudGameInput,
    ) -> Result<NativeSavePathsDto, String> {
        let config = blocking_load_config(config_path.to_path_buf()).await?;
        let cloud_game = cloud_game_from_input(&game);
        let key = manual_paths_key(&cloud_game);
        let manual = config
            .native_manual_save_paths
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let removed = config
            .native_removed_save_paths
            .get(&key)
            .cloned()
            .unwrap_or_default();
        // The only call site that ever fetches: see `pcgw_paths_for_title`'s
        // doc comment. A fetch failure degrades to an empty list here (via
        // that method's own `unwrap_or_default`) — never an error, so the
        // panel still allows manual paths.
        let pcgw = self.pcgw_paths_for_title(&cloud_game.title).await;
        let (visible_pcgw, visible_manual) = visible_native_paths(&pcgw, &manual, &removed);

        // The row tooltips must state the directory this host would really
        // read, so they need the same wine prefix the upload/restore paths
        // use (N14).
        let installed = blocking_installed(install).await?;
        let wine_prefix = installed
            .iter()
            .find(|row| games_match_identity(&cloud_game_from_installed(row), &cloud_game))
            .map(|row| row.native_wineprefix.trim().to_string())
            .filter(|prefix| !prefix.is_empty())
            .map(PathBuf::from);

        Ok(NativeSavePathsDto {
            pcgw: native_path_entries(&visible_pcgw, wine_prefix.as_deref()),
            manual: native_path_entries(&visible_manual, wine_prefix.as_deref()),
        })
    }

    pub async fn native_add_manual_save_path(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        path: String,
    ) -> Result<(), String> {
        let normalized = normalize_manual_save_path(Path::new(&path));
        self.mutate_native_paths(config_path, game, move |manual, removed| {
            if !manual.iter().any(|p| p == &normalized) {
                manual.push(normalized.clone());
            }
            // Adding a path back un-suppresses it: without this a removed
            // PCGW row could never be restored by the user.
            removed.retain(|p| p != &normalized);
        })
        .await
    }

    /// Removes ONE row from a native game's save-location list, whichever
    /// list it came from (`_pcgw_remove_path_for_game`,
    /// details_view_mixin.py:1218-1230). Manual paths are deleted; PCGW
    /// paths cannot be deleted at the source, so the path is recorded in
    /// `native_removed_save_paths` and filtered out of every later read.
    pub async fn native_remove_save_path(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        path: String,
    ) -> Result<(), String> {
        self.mutate_native_paths(config_path, game, move |manual, removed| {
            manual.retain(|p| p != &path);
            if !removed.iter().any(|p| p == &path) {
                removed.push(path.clone());
            }
        })
        .await
    }

    async fn mutate_native_paths(
        &self,
        config_path: &Path,
        game: CloudGameInput,
        mutate: impl FnOnce(&mut Vec<String>, &mut Vec<String>) + Send + 'static,
    ) -> Result<(), String> {
        let config_path = config_path.to_path_buf();
        let cloud_game = cloud_game_from_input(&game);
        let key = manual_paths_key(&cloud_game);
        tokio::task::spawn_blocking(move || {
            modify_config(&config_path, |config| {
                let mut manual = config
                    .native_manual_save_paths
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();
                let mut removed = config
                    .native_removed_save_paths
                    .get(&key)
                    .cloned()
                    .unwrap_or_default();
                mutate(&mut manual, &mut removed);
                config.native_manual_save_paths.insert(key.clone(), manual);
                config.native_removed_save_paths.insert(key, removed);
                Ok(())
            })
        })
        .await
        .map_err(|e| format!("native save path save did not finish: {e}"))??;
        self.caches.lock().await.clear();
        Ok(())
    }
```

- [ ] **Step 11: Update the commands.** In `app/src-tauri/src/commands/cloud.rs`, `native_save_paths` gains the install service and `native_remove_manual_save_path` is renamed:

```rust
#[tauri::command]
pub async fn native_save_paths(
    state: State<'_, AppState>,
    game: CloudGameInput,
) -> Result<NativeSavePathsDto, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    state
        .cloud
        .native_save_paths(install, &Config::default_path(), game)
        .await
}

#[tauri::command]
pub async fn native_remove_save_path(
    state: State<'_, AppState>,
    game: CloudGameInput,
    path: String,
) -> Result<(), String> {
    state
        .cloud
        .native_remove_save_path(&Config::default_path(), game, path)
        .await
}
```

In `app/src-tauri/src/lib.rs:315`, change `commands::cloud::native_remove_manual_save_path` to `commands::cloud::native_remove_save_path`.

- [ ] **Step 12: Run** `cargo test --workspace` — green. `cargo fmt`; both clippy commands.

- [ ] **Step 13: Commit**

```bash
git add crates/grid-core/src/config.rs app/src-tauri/src/cloud_service.rs app/src-tauri/src/commands/cloud.rs app/src-tauri/src/lib.rs
git commit -m "rewrite: persist native save-path removals, expose expanded paths, and thread the wine prefix"
```

---

### Task 5: Remove button on every save-location row, expanded-path tooltips (N1, N2 frontend)

**Files:**
- Modify: `app/src/lib/api.ts:253` (`NativeSavePaths`) and `:402-403` (the invoke wrapper)
- Modify: `app/src/lib/details/CloudPanel.svelte:66-70, 107-145, 208-237, 316-346`

**Interfaces:**
- Consumes: `NativeSavePathsDto`/`NativeSavePathEntryDto` and the renamed `native_remove_save_path` command from Task 4.
- Produces (api.ts):
  ```ts
  export type NativeSavePathEntry = { raw: string; expanded: string };
  export type NativeSavePaths = { pcgw: NativeSavePathEntry[]; manual: NativeSavePathEntry[] };
  ```
  and `nativeRemoveSavePath: (game: InstalledGame, path: string) => invoke<void>('native_remove_save_path', { game, path })` replacing `nativeRemoveManualSavePath`.

- [ ] **Step 1: Prove nothing else reads the old shape or the old command:** `grep -rn "nativeRemoveManualSavePath\|nativeSavePaths\|NativeSavePaths" app/src e2e/specs` — expected: `api.ts` and `details/CloudPanel.svelte` only. If an E2E spec invokes `native_remove_manual_save_path` directly, stop and report NEEDS_CONTEXT.

- [ ] **Step 2: Update `api.ts`.** Replace `:253` with:

```ts
/**
 * One native save-location row. `raw` is the stored, unexpanded path (the
 * label, and the value the remove command takes back); `expanded` is that
 * path resolved for this host and this game's wine prefix (the tooltip).
 */
export type NativeSavePathEntry = { raw: string; expanded: string };

/** A native game's save locations: PCGamingWiki rows first, then manual rows. */
export type NativeSavePaths = { pcgw: NativeSavePathEntry[]; manual: NativeSavePathEntry[] };
```

and replace the `nativeRemoveManualSavePath` wrapper at `:402-403` with:

```ts
  /** Removes one row from a native game's save-location list, PCGW or manual. */
  nativeRemoveSavePath: (game: InstalledGame, path: string) =>
    invoke<void>('native_remove_save_path', { game, path }),
```

- [ ] **Step 3: Update CloudPanel's handler.** Rename `handleRemoveManualPath` to `handleRemoveSavePath` (`:225-237`) and call `api.nativeRemoveSavePath(game, path)` inside it. Its `manualPathPending` guard, its `loadNativePaths()`/`loadRecords()` refresh and its error handling are unchanged.

- [ ] **Step 4: Render both lists with a remove button and a tooltip.** Replace the `<ul class="path-list">` block written in Task 3 Step 7 with:

```svelte
            <ul class="path-list">
              {#each nativePaths.pcgw as entry (entry.raw)}
                <li data-testid={`cloud-native-path-pcgw-${entry.raw}`} title={entry.expanded}>
                  <span>{entry.raw}</span>
                  <button
                    data-testid={`cloud-native-path-remove-${entry.raw}`}
                    class="remove"
                    disabled={manualPathPending}
                    onclick={() => handleRemoveSavePath(entry.raw)}
                    aria-label="Remove"
                    title="Remove this path"
                  >
                    ×
                  </button>
                </li>
              {/each}
              {#each nativePaths.manual as entry (entry.raw)}
                <li data-testid={`cloud-native-path-manual-${entry.raw}`} title={entry.expanded}>
                  <span>{entry.raw}</span>
                  <button
                    data-testid={`cloud-native-path-remove-${entry.raw}`}
                    class="remove"
                    disabled={manualPathPending}
                    onclick={() => handleRemoveSavePath(entry.raw)}
                    aria-label="Remove"
                    title="Remove this path"
                  >
                    ×
                  </button>
                </li>
              {/each}
            </ul>
```

The tooltip text and the accessible name are Python's verbatim (`details_view_mixin.py:1104-1116`: `label.setToolTip(expanded)`, `remove_btn.setToolTip("Remove this path")`, `remove_btn.setAccessibleName("Remove")`). The backend de-duplicates the two lists (Task 4's `visible_native_paths`), so `cloud-native-path-remove-<raw>` is unique across both loops.

- [ ] **Step 5: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings. `nativePathCount` (Task 3) still reads `.length` on both arrays, so it needs no change.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/api.ts app/src/lib/details/CloudPanel.svelte
git commit -m "rewrite: allow removing any native save location and show its expanded path"
```

---

### Task 6: Install-blocked reasons as tooltips (gap 9)

**Files:**
- Modify: `crates/grid-core/src/launch/selection.rs` (new `install_block_reason` + tests)
- Modify: `app/src-tauri/src/commands/specials.rs` (new command), `app/src-tauri/src/lib.rs` (registration)
- Modify: `app/src/lib/api.ts` (new wrapper)
- Create: `app/src/lib/details/blocked.ts`, `app/src/lib/details/blocked.test.ts`
- Modify: `app/src/lib/Details.svelte:226-243, 543-561, 577-579`

**Interfaces:**
- Produces (grid-core):
  ```rust
  pub fn install_block_reason(
      platform: &str,
      emulators: &[EmulatorEntry],
      profiles: &[EmulatorProfile],
      cores: CoreResolver<'_>,
  ) -> String
  ```
- Produces (app): `#[tauri::command] pub async fn install_block_reason(platform: String) -> Result<String, String>` in `commands/specials.rs`.
- Produces (api.ts): `installBlockReason: (platform: string) => invoke<string>('install_block_reason', { platform })`.
- Produces (`details/blocked.ts`):
  - `export function isEmulatorsPlatform(platform: string): boolean`
  - `export function ps4ContentBlockReason(platform: string, installed: boolean, romId: number | null, hasContent: boolean): string`
  - `export function xbox360ContentBlockReason(installed: boolean, romId: number | null): string`
  - `export function contentBlockReason(kind: 'update' | 'dlc', platform: string, installed: boolean, romId: number | null, hasContent: boolean): string`
- Consumes: `isContentPlatform`, `isNativePlatform` from `details/actions.ts:15,67`.

Python anchors, ported verbatim: `install_block_reason_for_game` (`grid_launcher/emulator/selection.py:370-390`), `_ps4_content_install_block_reason` and `_xbox360_content_install_block_reason` (`grid_launcher/ui/mixins/install_mixin.py:300-318`), applied as `setToolTip` at `game_views.py:670, 687, 699`.

**Ruling recorded here:** the reasons are rendered as `title` tooltips only. The buttons are **not** disabled, unlike Python (`game_views.py:668`). Disabling `details-install` would change what `install-a`/`install-b`/`native` can click depending on the fixture's emulator configuration; the backend already refuses an impossible install with its own error, which `details-error` surfaces. This is recorded in `docs/porting/03-library-install.md` by Task 9.

- [ ] **Step 1: Write the failing grid-core tests** in `crates/grid-core/src/launch/selection.rs`'s test module, following the existing `compatible_emulator_names_for_platform` tests' construction style (same `EmulatorEntry` builder and `&|_, _| Vec::new()` resolver those tests already use — read them first and reuse them exactly):

```rust
    #[test]
    fn install_block_reason_is_empty_for_native_and_emulators_platforms() {
        assert_eq!(
            install_block_reason("Windows", &[], &[], &|_, _| Vec::new()),
            ""
        );
        assert_eq!(
            install_block_reason("Emulators", &[], &[], &|_, _| Vec::new()),
            ""
        );
    }

    #[test]
    fn install_block_reason_names_a_missing_platform_value() {
        assert_eq!(
            install_block_reason("   ", &[], &[], &|_, _| Vec::new()),
            "Selected game has no platform value and cannot be installed."
        );
    }

    #[test]
    fn install_block_reason_asks_for_an_emulator_when_none_supports_the_platform() {
        assert_eq!(
            install_block_reason("SNES", &[], &[], &|_, _| Vec::new()),
            "No available emulator is configured for platform 'SNES'. \
             Add/configure one in Emulators before installing this game."
        );
    }
```

Add a fourth test that builds one `EmulatorEntry` whose `platforms` field names SNES (copy the construction from the neighbouring `compatible_emulator_names_for_platform` test) and asserts `install_block_reason("SNES", &[entry], &[], &|_, _| Vec::new())` is `""`.

- [ ] **Step 2: Run** `cargo test -p grid-core install_block_reason` — compile failure.

- [ ] **Step 3: Implement** in `selection.rs`, after `default_emulator_name_for_platform`:

```rust
/// Why this game cannot be installed, or `""` when it can
/// (`install_block_reason_for_game`, selection.py:370-390). Both strings are
/// the reference's verbatim.
///
/// Deviation from the reference: Python filters the candidate names through
/// `emulator_entry_has_usable_path` (selection.py:310-315), a filesystem
/// probe this port does not have; the rewrite's test is "a configured
/// emulator supports this platform". A configured-but-missing binary
/// therefore reports no block reason here and fails later at launch, with
/// the launcher's own error. Recorded in
/// `docs/porting/03-library-install.md`.
pub fn install_block_reason(
    platform: &str,
    emulators: &[EmulatorEntry],
    profiles: &[EmulatorProfile],
    cores: CoreResolver<'_>,
) -> String {
    if crate::cloud::scope::is_native_executable_platform(platform)
        || crate::cloud::scope::is_emulators_platform(platform)
    {
        return String::new();
    }
    let trimmed = platform.trim();
    if trimmed.is_empty() {
        return "Selected game has no platform value and cannot be installed.".to_string();
    }
    if !compatible_emulator_names_for_platform(emulators, trimmed, profiles, cores).is_empty() {
        return String::new();
    }
    format!(
        "No available emulator is configured for platform '{trimmed}'. \
         Add/configure one in Emulators before installing this game."
    )
}
```

- [ ] **Step 4: Run** `cargo test -p grid-core install_block_reason` — green.

- [ ] **Step 5: Add the command** in `app/src-tauri/src/commands/specials.rs`, after `content_availability`:

```rust
/// Why the primary Install button cannot install this game, or `""`. Config
/// + profile work only, so the blocking pool; the platform SLUG comes from
/// the process-wide registry through `installed_core_resolver`, which is why
/// the caller only has to send the platform NAME.
#[tauri::command]
pub async fn install_block_reason(platform: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(grid_core::launch::selection::install_block_reason(
            &platform,
            &config.emulators,
            load_profiles(),
            &grid_core::launch::selection::installed_core_resolver,
        ))
    })
    .await
    .map_err(|e| format!("install_block_reason did not finish: {e}"))?
}
```

Register it in `app/src-tauri/src/lib.rs` next to `commands::specials::content_availability`.

- [ ] **Step 6: Add the api.ts wrapper**, next to `contentAvailability` (`:409-410`):

```ts
  /** Why the Install button cannot install this platform's games, or ''. */
  installBlockReason: (platform: string) => invoke<string>('install_block_reason', { platform }),
```

- [ ] **Step 7: Write the failing content-reason tests** in `app/src/lib/details/blocked.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  contentBlockReason,
  isEmulatorsPlatform,
  ps4ContentBlockReason,
  xbox360ContentBlockReason,
} from './blocked';

describe('isEmulatorsPlatform', () => {
  it('matches the synthetic Emulators platform, case-insensitively', () => {
    expect(isEmulatorsPlatform('Emulators')).toBe(true);
    expect(isEmulatorsPlatform('  emulators ')).toBe(true);
    expect(isEmulatorsPlatform('SNES')).toBe(false);
  });
});

describe('ps4ContentBlockReason', () => {
  it('is blank off a PS4 platform', () => {
    expect(ps4ContentBlockReason('SNES', false, null, false)).toBe('');
  });

  it('asks for the base game first', () => {
    expect(ps4ContentBlockReason('PlayStation 4', false, 5, true)).toBe(
      'Install the base PS4 game before applying update or DLC content.'
    );
  });

  it('names the missing rom id', () => {
    expect(ps4ContentBlockReason('PS4', true, null, true)).toBe(
      'This game is missing a ROM id, so update/DLC content cannot be downloaded.'
    );
  });

  it('names the absent content', () => {
    expect(ps4ContentBlockReason('PS4', true, 5, false)).toBe(
      'No update or DLC content is available for this PS4 game on the server.'
    );
  });

  it('is blank when everything is in place', () => {
    expect(ps4ContentBlockReason('PS4', true, 5, true)).toBe('');
  });
});

describe('xbox360ContentBlockReason', () => {
  it('asks for the install, then the rom id, then passes', () => {
    expect(xbox360ContentBlockReason(false, 5)).toBe('Game must be installed before content can be applied.');
    expect(xbox360ContentBlockReason(true, null)).toBe('Game is missing a ROM ID.');
    expect(xbox360ContentBlockReason(true, 5)).toBe('');
  });
});

describe('contentBlockReason', () => {
  it('routes a PS4 platform to the PS4 reasons', () => {
    expect(contentBlockReason('update', 'PS4', true, null, true)).toBe(
      'This game is missing a ROM id, so update/DLC content cannot be downloaded.'
    );
  });

  it('routes an Xbox 360 platform to the Xbox 360 reasons', () => {
    expect(contentBlockReason('dlc', 'Xbox 360', true, null, true)).toBe('Game is missing a ROM ID.');
  });

  it('is blank on a platform with no extra content at all', () => {
    expect(contentBlockReason('update', 'SNES', true, 5, true)).toBe('');
  });
});
```

- [ ] **Step 8: Run** `npx vitest run blocked` — fails (module missing).

- [ ] **Step 9: Implement** `app/src/lib/details/blocked.ts`:

```ts
// Why an install action is blocked, as the details popup's button tooltips.
// Ported verbatim from `_ps4_content_install_block_reason` /
// `_xbox360_content_install_block_reason`
// (grid_launcher/ui/mixins/install_mixin.py:300-318), applied at
// `game_views.py:687,699`. The PRIMARY button's reason is not here: it needs
// the configured emulator list, so it comes from the backend's
// `install_block_reason` command.
//
// No API/store imports so this stays trivially unit-testable.
import { isContentPlatform } from './actions';

/**
 * `is_emulators_platform` (selection.py:138-142 / grid-core
 * `cloud::scope::is_emulators_platform`): trimmed, case-folded platform
 * equal to the literal "emulators".
 */
export function isEmulatorsPlatform(platform: string): boolean {
  return platform.trim().toLowerCase() === 'emulators';
}

function isPs4(platform: string): boolean {
  const normalized = platform.trim().toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
  const compact = normalized.replace(/ /g, '');
  if (normalized === '') return false;
  if (normalized === 'playstation 4' || normalized === 'ps4') return true;
  if (normalized.split(/\s+/).includes('ps4')) return true;
  return compact.includes('playstation4');
}

/** `_ps4_content_install_block_reason` (install_mixin.py:300-309). */
export function ps4ContentBlockReason(
  platform: string,
  installed: boolean,
  romId: number | null,
  hasContent: boolean
): string {
  if (!isPs4(platform)) return '';
  if (!installed) return 'Install the base PS4 game before applying update or DLC content.';
  if (romId === null) return 'This game is missing a ROM id, so update/DLC content cannot be downloaded.';
  if (!hasContent) return 'No update or DLC content is available for this PS4 game on the server.';
  return '';
}

/** `_xbox360_content_install_block_reason` (install_mixin.py:312-318). */
export function xbox360ContentBlockReason(installed: boolean, romId: number | null): string {
  if (!installed) return 'Game must be installed before content can be applied.';
  if (romId === null) return 'Game is missing a ROM ID.';
  return '';
}

/**
 * The tooltip for the Install Update / Install DLC button on `platform`.
 * PS4 platforms answer with the PS4 reasons; every other extra-content
 * platform (Xbox 360) answers with the Xbox 360 reasons; a platform with no
 * extra content at all has no button and therefore no reason.
 */
export function contentBlockReason(
  kind: 'update' | 'dlc',
  platform: string,
  installed: boolean,
  romId: number | null,
  hasContent: boolean
): string {
  void kind; // the reference's reasons do not distinguish update from DLC
  if (!isContentPlatform(platform)) return '';
  if (isPs4(platform)) return ps4ContentBlockReason(platform, installed, romId, hasContent);
  return xbox360ContentBlockReason(installed, romId);
}
```

- [ ] **Step 10: Run** `npx vitest run blocked` — green.

- [ ] **Step 11: Wire the tooltips** in `app/src/lib/Details.svelte`. Add to the imports at `:29`: `import { contentBlockReason } from './details/blocked';`. Add near the other install state (`:226-243`):

```svelte
  // The primary button's reason needs the configured emulator list, so it
  // comes from the backend; a failure leaves it blank rather than guessing.
  let installBlocked = $state('');
  $effect(() => {
    const platform = subject.platformName;
    api
      .installBlockReason(platform)
      .then((reason) => (installBlocked = reason))
      .catch(() => (installBlocked = ''));
  });

  let updateBlocked = $derived(
    contentBlockReason('update', subject.platformName, installedNow, subject.romId, buttons.update)
  );
  let dlcBlocked = $derived(
    contentBlockReason('dlc', subject.platformName, installedNow, subject.romId, buttons.dlc)
  );
```

Add `title={updateBlocked}` to the `details-install-update` button (`:543-551`), `title={dlcBlocked}` to `details-install-dlc` (`:553-561`), and `title={installBlocked}` to `details-install` (`:577-579`).

- [ ] **Step 12: Run** `cargo test --workspace`, `cargo fmt`, both clippy commands, `npx vitest run`, and from `app/` `npm run check` — all green, no new warnings.

- [ ] **Step 13: Commit**

```bash
git add crates/grid-core/src/launch/selection.rs app/src-tauri/src/commands/specials.rs app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/details/blocked.ts app/src/lib/details/blocked.test.ts app/src/lib/Details.svelte
git commit -m "rewrite: explain in a tooltip why an install or content button is blocked"
```

---

### Task 7: The dialog plugin and `pickers.ts`

**Files:**
- Modify: `app/src-tauri/Cargo.toml` (`[dependencies]`)
- Modify: `app/package.json` (`dependencies`)
- Modify: `app/src-tauri/src/lib.rs:104` (plugin chain)
- Modify: `app/src-tauri/capabilities/default.json`
- Create: `app/src/lib/pickers.ts`

**Interfaces:**
- Produces:
  ```ts
  export async function pickFolder(title: string): Promise<string | null>
  export async function pickFile(
    title: string,
    filters?: { name: string; extensions: string[] }[]
  ): Promise<string | null>
  ```
  Both return `null` when the user cancels or when the dialog is unavailable.
- Consumes: `open` from `@tauri-apps/plugin-dialog`.

- [ ] **Step 1: Add the Rust dependency.** From `app/src-tauri/`, run `cargo add tauri-plugin-dialog@2`. The crate must resolve against the pinned `tauri = "2.11.3"` (`app/src-tauri/Cargo.toml:25`); the repo already pins its other plugins to the bare major (`tauri-plugin-log = "2"`, `tauri-plugin-opener = "2"`, `:26-27`), so the same form is used here. Record the resolved version from `Cargo.lock` in the commit message. If the crate cannot be fetched (no network), stop and report NEEDS_CONTEXT — do not hand-write a version into the manifest.

- [ ] **Step 2: Add the npm dependency.** From `app/`, run `npm install @tauri-apps/plugin-dialog@^2`. It must land in `dependencies` (not `devDependencies`), alongside `@tauri-apps/api`. Confirm the installed major matches the Rust crate's.

- [ ] **Step 3: Register the plugin** in `app/src-tauri/src/lib.rs`. The chain at `:103-104` becomes:

```rust
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
```

- [ ] **Step 4: Grant the permission.** `app/src-tauri/capabilities/default.json` becomes:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "enables the default permissions",
  "windows": [
    "main"
  ],
  "permissions": [
    "core:default",
    "dialog:allow-open"
  ]
}
```

Only `dialog:allow-open` — the app never needs `save`, `message`, `ask` or `confirm`, and a narrower capability is a smaller attack surface.

- [ ] **Step 5: Write the wrapper** `app/src/lib/pickers.ts`:

```ts
// The ONE place `@tauri-apps/plugin-dialog` is imported. Components call
// `pickFolder`/`pickFile` instead, for three reasons: the plugin's `open`
// returns a union that every call site would otherwise have to narrow; a
// single seam keeps the capability (`dialog:allow-open`) auditable; and a
// failure to open a dialog — including the E2E build, which has no desktop
// portal behind it — degrades to "the user cancelled" rather than throwing
// into a component's click handler.
//
// Every Browse button in the app is ADDITIVE: the text input beside it stays
// and remains the path E2E drives, so no spec ever needs a real dialog.
import { open } from '@tauri-apps/plugin-dialog';

/** One existing directory, or `null` when the user cancelled. */
export async function pickFolder(title: string): Promise<string | null> {
  try {
    const picked = await open({ directory: true, multiple: false, title });
    return typeof picked === 'string' ? picked : null;
  } catch {
    return null;
  }
}

/**
 * One existing file, or `null` when the user cancelled. `filters` is passed
 * straight through; omit it to offer every file (an emulator entry may point
 * at a bare executable, an AppImage or a downloadable archive).
 */
export async function pickFile(
  title: string,
  filters?: { name: string; extensions: string[] }[]
): Promise<string | null> {
  try {
    const picked = await open({ directory: false, multiple: false, title, filters });
    return typeof picked === 'string' ? picked : null;
  } catch {
    return null;
  }
}
```

- [ ] **Step 6: Prove the E2E build still refuses the automation plugins:** run `scripts/check_secret_hygiene.sh` from `rewrite/` — it must pass (the dialog plugin is a normal dependency, not a feature-gated one, so it may appear in the default tree).

- [ ] **Step 7: Run** `cargo build -p app`, `cargo fmt`, both clippy commands, and from `app/` `npm run check` — green.

- [ ] **Step 8: Commit**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/capabilities/default.json app/src-tauri/src/lib.rs app/src/lib/pickers.ts app/package.json app/package-lock.json ../Cargo.lock
git commit -m "rewrite: add the dialog plugin behind one pickFolder/pickFile seam"
```

(If `Cargo.lock` lives at `rewrite/Cargo.lock` rather than the repo root, adjust the pathspec — check with `ls rewrite/Cargo.lock` first.)

---

### Task 8: Browse… buttons at the four sites (gap 4, N13)

**Files:**
- Modify: `app/src/lib/Server.svelte:355-368`
- Modify: `app/src/lib/details/CloudPanel.svelte` (the `.add-path` row) and its `<style>`
- Modify: `app/src/lib/emulators/EmulatorForm.svelte:126-129` and its `<style>`
- Modify: `app/src/lib/Connect.svelte` — **conditional, see Step 5**

**Interfaces:**
- Consumes: `pickFolder` / `pickFile` from `app/src/lib/pickers.ts` (Task 7).

Python anchors: `dialogs.py:140-146` (library path, `"Browse..."`), `details_view_mixin.py:1129-1141` (save folder, tooltip `"Add a custom save folder for this game"`), `dialogs.py:352-356` (emulator executable/archive). The rewrite spells the label `Browse…` with the ellipsis character, matching every other ellipsis in this UI.

- [ ] **Step 1: Library path (Server.svelte).** Add the import `import { pickFolder } from './pickers';` and a handler next to `saveLibraryPath`:

```svelte
  async function browseLibraryPath() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) libraryPathInput = picked;
  }
```

Insert the button between the input and the Save button inside `library-path-banner`:

```svelte
      <button
        data-testid="library-path-browse"
        class="browse"
        disabled={libraryPathSaving}
        onclick={browseLibraryPath}
      >
        Browse…
      </button>
```

The input keeps `data-testid="library-path-input"` and its `bind:value`, so `connect`/`library` E2E is unaffected.

- [ ] **Step 2: Manual save folder (CloudPanel.svelte).** Add `import { pickFolder } from '../pickers';` and:

```svelte
  async function browseManualPath() {
    const picked = await pickFolder('Select Save Folder');
    if (picked === null) return;
    manualPathInput = picked;
    await handleAddManualPath();
  }
```

Add the button inside the `.add-path` row, after `cloud-native-path-add`:

```svelte
            <button
              data-testid="cloud-native-path-browse"
              disabled={manualPathPending}
              onclick={browseManualPath}
              title="Add a custom save folder for this game"
            >
              Browse…
            </button>
```

Picking a folder adds it immediately, exactly as Python does (`_browse` → `_pcgw_add_manual_path_for_game` → refresh, `details_view_mixin.py:1131-1138`); the text input's own Add button is unchanged.

- [ ] **Step 3: Emulator executable/archive (EmulatorForm.svelte).** Add `import { pickFile } from '../pickers';` and:

```svelte
  // No filter: this one field accepts a bare executable, an AppImage or a
  // downloadable archive — Python labels it "Executable / Archive Path" for
  // a new entry (dialogs.py:355).
  async function browseEmulatorPath() {
    const picked = await pickFile('Select Emulator Executable or Archive');
    if (picked === null) return;
    formPath = picked;
    await autoFillFromPath();
  }
```

Replace the executable `<label>` body with a row:

```svelte
  <label>
    Executable path
    <span class="path-row">
      <input data-testid="emu-form-path" bind:value={formPath} onblur={autoFillFromPath} onkeydown={onPathKeydown} />
      <button data-testid="emu-form-path-browse" type="button" disabled={formPending} onclick={browseEmulatorPath}>
        Browse…
      </button>
    </span>
  </label>
```

`type="button"` is required: the field sits inside a `<form>` whose default submit is `save()`.

Add to the `<style>`:

```css
  .path-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .path-row input {
    flex: 1;
    min-width: 0;
  }
```

- [ ] **Step 4: CSS for the two other sites.** In `Server.svelte`, give `.browse` the same declarations as the existing `library-path-save` button rule (read it and copy; do not invent a new visual). In `CloudPanel.svelte`, the `.add-path button` rule at `:515-523` already styles every button in that row, so the new button needs no CSS.

- [ ] **Step 5 (conditional): Connect library path.** Run `grep -n "connect-library-path" app/src/lib/Connect.svelte`.
  - **No match** → skip this step entirely and report "Task 8 Step 5 skipped: `connect-library-path` does not exist yet (gap 12's plan has not landed)". Do not add the field.
  - **Match** → add `import { pickFolder } from './pickers';`, a `browseLibraryPath` handler identical to Step 1's (writing into whatever state variable that input binds), and a button next to the input:

```svelte
    <button
      data-testid="connect-library-path-browse"
      type="button"
      disabled={session.busy}
      onclick={browseLibraryPath}
    >
      Browse…
    </button>
```

- [ ] **Step 6: Run** `npx vitest run` and, from `app/`, `npm run check` — green, no new warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/Server.svelte app/src/lib/details/CloudPanel.svelte app/src/lib/emulators/EmulatorForm.svelte
git commit -m "rewrite: add Browse buttons beside every path field"
```

(Add `app/src/lib/Connect.svelte` to the pathspec only if Step 5 ran.)

---

### Task 9: Behaviour docs

**Files:**
- Modify: `docs/porting/06-cloud-saves.md` — the native save-panel behaviour and the "Rust port deviations (milestone 6)" list
- Modify: `docs/porting/03-library-install.md` — the block-reason paragraph in §12 (~line 923) and the "Rust port deviations (milestone 8)" list

- [ ] **Step 1: `docs/porting/06-cloud-saves.md`.** Under "### Upload — native games" (line 713), add a short subsection "Save-location panel" stating, with the Python anchors this plan used:
  - the panel lists PCGamingWiki rows then manual rows, de-duplicated, each row carrying its expanded path as a tooltip and a remove button (`details_view_mixin.py:1096-1126`);
  - the status line, empty label and upload tooltip strings and the order they are chosen in (`:1143-1185`);
  - that removing a row persists into `native_removed_save_paths`, and that adding the same path back clears the suppression.

- [ ] **Step 2:** Append two numbered items to "## Rust port deviations (milestone 6)" (continuing the existing numbering after D11):
  - **"Native save-path removals are persisted, not session-only."** Python's `_pcgw_remove_path_for_game` (`details_view_mixin.py:1218-1230`) edits only the in-memory `_pcgw_paths_cache` for a PCGW row, so the row reappears after the next lookup. The rewrite records it in `Config::native_removed_save_paths` (`crates/grid-core/src/config.rs`) and filters it out of every read in `CloudService::native_save_paths` (`app/src-tauri/src/cloud_service.rs`). Reason: a removal the user made deliberately must survive a restart. Adding the path back through the manual field or Browse clears the suppression, so nothing is unrecoverable.
  - **"Native save-path row tooltips use `resolve_native_save_dir`, not `expandvars`."** Python's tooltip is `os.path.expandvars(raw)` (`:1097`), which on Linux leaves a `%APPDATA%` path unchanged. The rewrite resolves it through the same `resolve_native_save_dir(raw, None, wine_prefix)` the upload and restore paths use, so the tooltip names the directory that would really be read.

- [ ] **Step 3:** In the same deviations list, add the N14 fix as a **defect note** rather than a deviation: state that `CloudContext.wine_prefix` was hardcoded `None` at both construction sites, that native upload/restore on Linux therefore never translated `%APPDATA%`/`%LOCALAPPDATA%`/`%USERPROFILE%` into the prefix despite `ops/native.rs:83,177,237,241` consuming it correctly, and that the prefix is now threaded from the matching registry row's `native_wineprefix` (`crates/grid-core/src/library/registry.rs:210`).

- [ ] **Step 4: `docs/porting/03-library-install.md`.** Extend the block-reason sentence at ~line 923 with the rewrite's mapping: the PS4 and Xbox 360 reasons are rendered as `title` tooltips on `details-install-update` / `details-install-dlc` by `app/src/lib/details/blocked.ts`; the primary button's reason comes from the `install_block_reason` command (`crates/grid-core/src/launch/selection.rs`).

- [ ] **Step 5:** Append two items to "## Rust port deviations (milestone 8)" (continuing after the existing numbering):
  - **"Install-blocked reasons are shown, not enforced."** Python disables the primary and content buttons while a block reason is non-empty (`game_views.py:668,687,699`). The rewrite renders the reason as a `title` tooltip and leaves the button enabled; the backend refuses an impossible install with its own error, which the popup already surfaces. Reason: the button's enabled state would otherwise depend on the machine's emulator configuration, which is not a property of the game.
  - **"`emulator_entry_has_usable_path` is not ported."** `install_block_reason` tests only whether a configured emulator supports the platform (`compatible_emulator_names_for_platform`), not whether its binary exists on disk (`selection.py:310-315`). A configured-but-missing emulator therefore reports no block reason and fails at launch instead.

- [ ] **Step 6: Commit**

```bash
git add ../docs/porting/06-cloud-saves.md ../docs/porting/03-library-install.md
git commit -m "rewrite: document the native save-panel behaviour and the install block-reason rulings"
```

---

### Task 10: E2E coverage and gate

**Files:**
- Modify: `e2e/specs/native.spec.ts` (new cases; the stale comment at `:126-130`)

**Interfaces:**
- Consumes: `configPath()` from `e2e/helpers/env.ts:29`, already used by `cloud-saves.spec.ts`.

**Mock-server ruling (verified before writing):** there is no PCGamingWiki mock. `grid-core`'s `PCGW_API_BASE` is the live `https://www.pcgamingwiki.com/w/api.php` (`crates/grid-core/src/pcgw.rs:36`) with no override, and `pcgw_paths_for_title` degrades a failed fetch to an empty list (`cloud_service.rs:211-213`). The fixture title "My Game" has no PCGamingWiki article either way, so **the pcgw list is empty in E2E whether the runner has network or not**. Every assertion below is therefore written against manual rows and the config file, never against a PCGW row, and the status-line assertion uses a regex so a surprise PCGW hit cannot fail it.

- [ ] **Step 1: Fix the stale comment** at `e2e/specs/native.spec.ts:126-130`: Task 2 makes the option label relative to the backend's `install_dir`, not to `installDirOf(candidates)`. Reword it to say the visible label is the candidate's path relative to the backend's reported install directory, and that the assertion below pins the option's VALUE because that is what identifies the file.

- [ ] **Step 2: Add the launch-target case** after the `'labels the primary button "Install App" for a Windows game'` case:

```ts
  it('states no emulator launch target for a native game', async () => {
    // User ruling 2026-09-05: a Windows/Linux game runs its own executable
    // through a compat tool, so the popup renders no launch-target line at
    // all rather than "No default emulator" (details/header.ts).
    await expect($(testId('details-emulator'))).not.toExist();
    // The line beside it is still rendered, so this proves the aside itself
    // is present and only the emulator row is gone.
    await expect($(testId('details-last-played'))).toExist();
  });
```

- [ ] **Step 3: Add the save-location cases** after the Game Settings case (rom 701 is installed by then, and the details overlay is open on the server view):

```ts
  /** Opens the Saves tab's native save panel for the currently open game. */
  async function openNativeSavePanel() {
    await $(testId('details-tab-saves')).click();
    await $(testId('details-cloud-save-toggle')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Manage Saves toggle never appeared for the native game',
    });
    await $(testId('details-cloud-save-toggle')).click();
    await $(testId('cloud-native-status')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the native save-location section never rendered',
    });
  }

  it('adds, lists and removes a manual native save location', async () => {
    await openNativeSavePanel();

    const manual = path.join(dataDir(), 'manual-saves');
    await $(testId('cloud-native-path-input')).setValue(manual);
    await $(testId('cloud-native-path-add')).click();

    const row = $(`[data-testid="cloud-native-path-manual-${manual}"]`);
    await row.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the manual save location never appeared in the list',
    });

    // "N save location(s) configured." — the count is asserted as a regex
    // because a live PCGamingWiki answer would legitimately raise it.
    const status = await $(testId('cloud-native-status')).getText();
    expect(status).toMatch(/^\d+ save location\(s\) configured\.$/);
    expect(Number(status.split(' ')[0])).toBeGreaterThanOrEqual(1);

    await $(`[data-testid="cloud-native-path-remove-${manual}"]`).click();
    await row.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the manual save location never disappeared after Remove',
    });
  });

  it('persists the removal across a popup reopen', async () => {
    const manual = path.join(dataDir(), 'manual-saves');

    // The suppression list is what makes a removal survive: it is written to
    // the config, not just to the open panel's state.
    await browser.waitUntil(
      () => readFileSync(configPath(), 'utf-8').includes('native_removed_save_paths'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the removal never reached native_removed_save_paths in the config',
      },
    );
    expect(readFileSync(configPath(), 'utf-8')).toContain(manual);

    await closeDetails();
    await openDetails(701);
    await openNativeSavePanel();
    await expect($(`[data-testid="cloud-native-path-manual-${manual}"]`)).not.toExist();
    await expect($(testId('cloud-native-status'))).toHaveText('No save locations found on PCGamingWiki.');
  });
```

Add `configPath` to the `../helpers/env.js` import list at the top of the file.

- [ ] **Step 4: Reconcile the last assertion with reality.** The final `toHaveText` assumes an empty PCGW list. Before committing, run the `native` group once (Step 5) and read the actual text; if a live PCGW answer made it a count line, replace that single assertion with the same `/^\d+ save location\(s\) configured\.$/` regex plus an explicit `not.toExist()` on the removed row (which is the real subject of the test) and note the reason in the spec's comment.

- [ ] **Step 5: Run the gate.** From `rewrite/`, detached with a log:

```bash
nohup scripts/e2e.sh native cloud-saves install library connect > /tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/e2e-parity2.log 2>&1 &
```

then poll the log until the summary line appears. `native` covers the launch-target line, the Game Settings dialog and the save-location section; `cloud-saves` proves the `cloud_panel_info` native block reason and the emulator save flows still hold after the `CloudContext` change; `install` and `library` cover the install buttons and the library-path banner; `connect` covers the first-run form.

- [ ] **Step 6:** All five groups green. If one fails, read the failure, fix the cause within this plan's scope, re-run that group, and commit the fix with a `rewrite: ` subject.

- [ ] **Step 7:** Report the per-group result lines verbatim.

- [ ] **Step 8: Commit**

```bash
git add e2e/specs/native.spec.ts
git commit -m "rewrite: cover the native save-location panel and the missing launch-target line in E2E"
```

---

## Self-review notes

**Spec coverage.** Every item in the stated scope maps to a task: gap 4 → Tasks 7-8 (library path, manual save folder, emulator executable/archive, Connect conditional). Gap 9 → Task 6. Gap 15 (NativeSettings strings only) → Task 2. N1 → Tasks 4+5. N2 → Tasks 4+5. N3/N4/N5/N6/N7 → Task 3. N8/N9/N10 → Task 2. N13 → Task 8 Step 2. N14 → Task 4 Steps 8-9. Controller ruling (hide "No default emulator") → Task 1. Docs → Task 9. E2E → Task 10. The deferred items in the rulings (gap 3, gap 13, native gap 15, Eden file checks) appear in no task.

**Placeholder scan.** No step says "similar to", "add appropriate" or "TODO". Two steps are deliberately conditional and say exactly what to do in each branch: Task 8 Step 5 (`connect-library-path` may not exist yet — skip and report) and Task 10 Step 4 (the final PCGW-dependent assertion is reconciled against a real run before commit). Task 4 Step 5 says to check whether `InstalledGame` derives `Default` and to use a struct literal otherwise; Task 6 Step 1 says to reuse the neighbouring tests' `EmulatorEntry` builder. Task 7 Steps 1-2 use `cargo add` / `npm install` rather than inventing a version number, and stop if the crate cannot be fetched.

**Type consistency.** `NativeSavePathEntryDto { raw, expanded }` (Task 4) is mirrored exactly by `NativeSavePathEntry` in `api.ts` (Task 5), and `CloudPanel.svelte` reads `entry.raw`/`entry.expanded` only after that change lands — Task 3 leaves the list rendering on the old `string` shape so the file compiles between the two commits, and Task 5 replaces it wholesale. `NativePathsPhase` is produced and consumed only inside Task 3's module and `CloudPanel.svelte`. `Inputs::context`'s new second argument is applied at all seven call sites in Task 4 Step 8, including the existing unit test at `cloud_service.rs:1489`. `run_auto_upload`'s new parameter is passed at its single call site. The renamed `native_remove_save_path` is changed in the service, the command, `lib.rs`'s invoke handler and `api.ts` in the same commit. `NativeGameSettings.install_dir` is added to the Rust struct and the TS type in the same task. `install_block_reason` exists in grid-core (Task 6 Step 3), as a command (Step 5), and in `api.ts` (Step 6) before `Details.svelte` calls it (Step 11).

**Two corrections made during review, both folded into the tasks above.**
1. The research suggested extending `isNativeExecutablePlatform` to cover "linux". That function is a mirror of grid-core's `cloud::scope::is_native_executable_platform` (`crates/grid-core/src/cloud/scope.rs:68`) and drives `Details.svelte:257`'s `isNative`, which selects the whole native cloud-panel mode. Widening it would make the frontend claim a native save panel for Linux games while the backend still answers them as emulator games. Task 1 therefore adds a separate, display-only `isNativeLaunchPlatform` and says why in the code comment.
2. The research suggested returning the content block reasons from `content_availability`. Every input those reasons need (platform, installed, rom id, availability flags) is already on the client, so Task 6 keeps them in a pure TS module instead — one fewer IPC round trip and a module vitest can test directly. The primary button's reason genuinely needs the emulator list, so that one is a backend command.

**One thing this plan changes that no test previously covered:** `visible_native_paths` now de-duplicates a manual path that equals a PCGW path. Without it, Task 5's two `{#each}` loops would emit the same `cloud-native-path-remove-<raw>` test id twice. It is covered by a Rust unit test (Task 4 Step 5) and matches Python's own `_native_save_paths_for_game` (`details_view_mixin.py:1060-1065`).
