# Parity 1 — Emulators, Settings, first-run Connect and the shell

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Python→rewrite UI parity gaps that belong to the Emulators view, the Settings view, the first-run Connect screen and the shell: research gaps **1, 2, 5, 6, 7, 10, 11, 12, 14, 15 (partial)**.

**Architecture:** Five tasks are pure frontend — a global toast surface plus one new pure TypeScript module per feature (`stores/toasts.svelte.ts`, `emulators/form.ts`, `emulators/notes.ts`, additions to `settings/connection.ts` and `emulators/retroachievements.ts`), each with a vitest suite, and a thin `.svelte` binding that `npm run check` and E2E verify. Four tasks add backend surface: one new grid-core module (`retroachievements.rs`, the `dorequest.php?r=login` client), two new spawn helpers in `launch/spawn.rs`, three new Tauri commands (`open_config_folder`, `launch_emulator`, `retroachievements_login`) and one Tauri plugin (`tauri-plugin-window-state`). The RetroAchievements token never leaves the keyring path that already exists: `AppState.ra_store` (`app/src-tauri/src/commands.rs:36-37`, `crates/grid-core/src/secrets.rs:36-41`), and the login command returns only a username and the fan-out rows.

**Tech Stack:** Rust (grid-core + the Tauri app), Svelte 5 runes + TypeScript + vitest for pure modules only, WebdriverIO E2E against the mock RomM server.

**Spec:** the parity research produced 2026-09-05 (`/tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/research-parity.md`, sections "Controller rulings" and "A. General UI parity gaps") is the gap list; `docs/porting/*.md` are the behaviour specs (01 §RetroAchievements login, 02 §config keys, 04 §emulator launch, 05 §autoconfig, 08 §RALoginWorker) and must be updated by the tasks that change what they describe; `SPEC.md` and `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` remain the design frame.

All paths below are relative to `rewrite/` unless they start with `docs/` or `grid_launcher/`.

## User decisions / rulings (binding)

1. **In scope:** research gaps 1 (five per-emulator cloud fields on the emulator form), 2 (RetroAchievements username+password login), 5 (global toast surface), 6 (per-emulator controller notes), 7 (editable connection in Settings), 10 (Open Config Folder), 11 (standalone emulator Launch), 12 (library path on first-run Connect), 14 (window geometry), and **part of** 15: only the EmulatorForm `%rom%, %core%, %ps3_launch_target%` label and the CloudSavesPage auto-sync hint.
2. **Out of scope, explicitly:** gap 4 (folder/file pickers — a separate plan). Gap 12 therefore adds the library-path field to `Connect.svelte` as a **plain text input only**, with no `Browse…` button. The gap-15 `NativeSettings.svelte` strings ("Custom Launch Parameters" title + hint, Install Directory row) belong to the native-saves plan and are not touched here. Gaps 3, 9, 13 and everything in section B are other plans.
3. **Deferred by the controller rulings, stays out:** "Update from Source" for installed emulators; the `Enable debug prints` toggle; the Windows Documents-redirection resolver; the Eden `prod.keys` and Switch-firmware presence notes (`emulator_ui_mixin.py:729-748`) — those need new backend probes. Only the five **static** per-emulator notes are ported (`emulator_ui_mixin.py:714, 723, 752, 761, 770`).
4. **The two existing component-local toasts stay exactly as they are.** `details-update-toast` (`app/src/lib/Details.svelte:587`) and `emulator-ps3-firmware-toast` (`app/src/lib/Emulators.svelte:543`) are asserted verbatim by `e2e/specs/updates.spec.ts:238` and `e2e/specs/firmware.spec.ts:175-181`. The new global surface carries only the NEW messages (added emulator, the four RetroAchievements messages, a standalone-launch failure). Re-routing the two old ones is a follow-up, not this plan.
5. **The Arguments field keeps its empty default.** Python seeds `%rom%` (`grid_launcher/ui/dialogs.py:360, 505`); the rewrite must not, because `e2e/specs/emulators.spec.ts` asserts `emu-form-args` opens with value `''` and both auto-fills are gated on args being blank (`EmulatorForm.svelte:44, 66`). Only the **label** changes (gap 15).
6. **Save Strategy writes the selected value verbatim,** matching `entry_payload` (`dialogs.py:537`, `... or "auto"`). Writing `"auto"` where the field was previously blank is behaviourally identical — `normalize_save_strategy` maps `""` and `"auto"` to the same result (`crates/grid-core/src/autoconfig/entry.rs:87-95`).
7. **The standalone launch does not re-run the autoconfig sync.** Python calls `_ensure_emulator_sync_settings` before spawning (`emulator_ui_mixin.py:1653`); the rewrite runs that sync at add/install time only (D1 call site B, `commands.rs:706-725`), and `launch_game` does not run it either. Deliberate deviation, recorded in `docs/porting/04-emulator-launch.md`. Python's 500 ms `_warn_if_process_exited_early` modal (`emulator_ui_mixin.py:1662`) is also not ported — there is no modal warning surface and no session row for a ROM-less launch.
8. **`tauri-plugin-window-state` is registered only in non-`e2e` builds.** The plugin writes `.window-state.json` under Tauri's AppConfig directory, which `GRID_LAUNCHER_DATA_DIR` does not redirect and `scripts/e2e.sh` does not sandbox (it redirects `XDG_DATA_HOME`/`XDG_RUNTIME_DIR`/`XDG_CACHE_HOME` only, `scripts/e2e.sh:196-198`). Gating it keeps the harness's "no real-home writes" property and keeps the Xvfb window size deterministic across stage groups.
9. **Verbatim strings.** The five controller notes, `Added emulator '<name>'.`, the four RetroAchievements messages, `Open Config Folder`, `Could not open config folder: <e>`, the launch error texts and the Python emulator-form labels are copied character for character from the reference (including `→`, `·` and `—`).

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. The RetroAchievements token goes to `AppState.ra_store` (`commands.rs:36-37`) and nowhere else; `RaStatus` keeps carrying a bare boolean (`commands.rs:1035-1041`). The RetroAchievements **password** is never stored, never logged, never echoed in an error, and never put in a `Result::Err` — the login URL carries it as a query parameter, so every `reqwest::Error` is passed through `.without_url()` (the pattern at `crates/grid-core/src/romm/mod.rs:73, 92`). Any task that touches secrets also runs `scripts/check_secret_hygiene.sh`.
- **Only `app.css` tokens for colours**; `--m-*` motion tokens. The one literal `rgba()` allowed in this plan is the drop shadow `0 12px 32px rgba(0, 0, 0, 0.35)`, copied verbatim from `Shell.svelte`'s `.server-menu`.
- **Every test id E2E asserts today stays.** In particular `emulator-row-*`, `emulator-edit-*`, `emulator-delete-*`, `emulator-ps3-firmware-note-*`, `emulator-ps3-firmware-*`, `emulator-ps3-firmware-toast`, `emu-form-*`, `emu-nav-*`, `emu-page-*`, `default-select-*`, `default-core-*`, `connect-*`, `settings-nav-*`, `settings-connection-*`, `cloud-settings-*`, `ra-*`, `details-update-toast`. New markup must not introduce a second element matching `[data-testid^="emulator-row-"] .name` (the selector `emulators.spec.ts`'s `rowNames()` uses) — the controller notes use class `.note`.
- **No component test harness exists** (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted pure module with vitest tests, plus `npm run check`, plus E2E — never by a fabricated component test.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` (record the baseline warning count by running it once before Task 1 — the previous plan's baseline was 3: two in `Details.svelte`, one in `DownloadsFooter.svelte`; no new ones) and `npx vitest run` green. Then a commit whose subject starts `rewrite: `.
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- The final task runs the E2E groups `connect`, `connect-restore`, `emulators`, `launch`, `emulator-catalog`, `firmware`, `ps3-install`, `cloud-saves`, `updates` (`rewrite/scripts/e2e.sh connect connect-restore emulators launch emulator-catalog firmware ps3-install cloud-saves updates`, detached, log to a file) and they must be green.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/stores/toasts.svelte.ts`, `stores/toasts.test.ts` | the toast queue (pure `appendToast`/`removeToast` + the `$state` store) |
| `app/src/lib/Toast.svelte` | the bottom-centred toast surface, mounted once in `Shell.svelte` |
| `app/src/lib/Shell.svelte` | mounts `<Toast />` outside the view roots |
| `app/src/lib/emulators/form.ts`, `emulators/form.test.ts` | save-strategy normalisation, form seeding/patching, the `Added emulator '<name>'.` text, the Arguments label |
| `app/src/lib/emulators/EmulatorForm.svelte` | the five cloud fields, the new Arguments label, the add toast |
| `app/src/lib/emulators/notes.ts`, `emulators/notes.test.ts` | the five static per-emulator setup notes and their name matching |
| `app/src/lib/Emulators.svelte` | renders the notes and the per-row Launch button |
| `app/src/lib/settings/CloudSavesPage.svelte` | the auto-sync hint (gap 15) |
| `app/src/lib/Connect.svelte` | the first-run Library Path text input (gap 12) |
| `app/src/lib/settings/connection.ts`, `settings/connection.test.ts` | `canConnect`, `OPEN_CONFIG_FOLDER_LABEL` |
| `app/src/lib/settings/ConnectionPage.svelte` | Open Config Folder button + the Edit connection disclosure |
| `app/src/lib/emulators/retroachievements.ts`, `emulators/retroachievements.test.ts` | `canLogin` and the four RetroAchievements toast texts |
| `app/src/lib/settings/RetroAchievementsPage.svelte` | Password field + Log In button, toasts |
| `app/src/lib/api.ts` | IPC wrappers for the three new commands |
| `crates/grid-core/src/retroachievements.rs`, `crates/grid-core/src/lib.rs` | the `dorequest.php?r=login` client |
| `crates/grid-core/tests/ra_login.rs` | wiremock integration test for the login client |
| `crates/grid-core/src/launch/spawn.rs` | `prepare_standalone_emulator_launch` + `spawn_standalone_emulator` |
| `app/src-tauri/src/commands.rs` | `config_dir_for`, `open_config_folder`, `launch_emulator`, `retroachievements_login`, `store_ra_credentials` |
| `app/src-tauri/src/lib.rs` | command registration + the window-state plugin |
| `app/src-tauri/Cargo.toml` | `tauri-plugin-window-state` |
| `scripts/check_secret_hygiene.sh` | allowlists `retroachievements.rs` for `expose_secret` |
| `e2e/specs/emulators.spec.ts`, `e2e/specs/connect.spec.ts` | new E2E cases |
| `docs/porting/01-romm-api.md`, `02-config-and-secrets.md`, `04-emulator-launch.md` | behaviour docs updated |

---

### Task 1: Global toast surface

**Files:**
- Create: `app/src/lib/stores/toasts.svelte.ts`
- Create: `app/src/lib/stores/toasts.test.ts`
- Create: `app/src/lib/Toast.svelte`
- Modify: `app/src/lib/Shell.svelte` (import block `:1-20`, the `<DownloadsFooter />` mount site `:205`)

**Interfaces:**
- Produces: `export type ToastLevel = 'success' | 'error'`
- Produces: `export type Toast = { id: number; text: string; level: ToastLevel }`
- Produces: `export const TOAST_LIMIT = 3`, `export const TOAST_DURATION_MS = 4000`
- Produces: `export function appendToast(list: Toast[], next: Toast, limit?: number): Toast[]`
- Produces: `export function removeToast(list: Toast[], id: number): Toast[]`
- Produces: `export const toasts: { readonly list: Toast[] }`
- Produces: `export function pushToast(text: string, level?: ToastLevel): number | null`
- Produces: `export function dismissToast(id: number): void`

- [ ] **Step 1: Write the failing test** `app/src/lib/stores/toasts.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { appendToast, removeToast, TOAST_LIMIT, type Toast } from './toasts.svelte';

const toast = (id: number, text: string): Toast => ({ id, text, level: 'success' });

describe('appendToast', () => {
  it('appends to the end so the newest toast is last', () => {
    const list = appendToast([toast(1, 'first')], toast(2, 'second'));
    expect(list.map((t) => t.text)).toEqual(['first', 'second']);
  });

  it('drops the oldest once the limit is reached', () => {
    let list: Toast[] = [];
    for (let i = 1; i <= TOAST_LIMIT + 2; i += 1) list = appendToast(list, toast(i, `t${i}`));
    expect(list).toHaveLength(TOAST_LIMIT);
    expect(list[0].text).toBe(`t${3}`);
  });

  it('honours an explicit limit', () => {
    const list = appendToast([toast(1, 'a'), toast(2, 'b')], toast(3, 'c'), 2);
    expect(list.map((t) => t.text)).toEqual(['b', 'c']);
  });

  it('ignores a blank message, matching ToastWidget.show_message', () => {
    const before = [toast(1, 'a')];
    expect(appendToast(before, toast(2, '   '))).toBe(before);
  });

  it('does not mutate the input list', () => {
    const before = [toast(1, 'a')];
    appendToast(before, toast(2, 'b'));
    expect(before).toHaveLength(1);
  });
});

describe('removeToast', () => {
  it('removes only the matching id', () => {
    const list = removeToast([toast(1, 'a'), toast(2, 'b')], 1);
    expect(list.map((t) => t.id)).toEqual([2]);
  });

  it('is a no-op for an unknown id', () => {
    const list = removeToast([toast(1, 'a')], 99);
    expect(list.map((t) => t.id)).toEqual([1]);
  });
});
```

- [ ] **Step 2: Run** from `app/`: `npx vitest run src/lib/stores/toasts.test.ts` — expect failure (module missing).

- [ ] **Step 3: Implement the store** `app/src/lib/stores/toasts.svelte.ts`:

```ts
// The app-wide transient message surface — the port of `show_toast`
// (grid_launcher/ui/toast.py:97) and its `ToastWidget` (toast.py:7-95).
// Module-scoped `$state` so any component can push without prop drilling,
// mirroring `stores/appUpdate.svelte.ts`.
//
// The `append*`/`remove*` pair below is pure and exported for vitest; the
// `$state` wrapper under it is the part components read. Same split as
// `stores/updates.svelte.ts`'s `labelFor`.

export type ToastLevel = 'success' | 'error';

export type Toast = { id: number; text: string; level: ToastLevel };

/**
 * Python shows exactly one toast at a time (a single reused `ToastWidget`,
 * toast.py:100-105). The rewrite keeps a short stack instead so a fan-out
 * that reports twice in quick succession does not lose the first line; three
 * is the most that fits above the download strip without covering content.
 */
export const TOAST_LIMIT = 3;

/**
 * `ToastWidget.__init__`'s `duration_ms` default is 2400 (toast.py:15). The
 * rewrite uses 4000: a WebDriver round trip plus a command's own latency
 * eats a visible slice of 2400 ms, and E2E asserts the "Added emulator"
 * text. Deliberate, documented deviation.
 */
export const TOAST_DURATION_MS = 4000;

/** `show_message` ignores a blank message (toast.py:64-66); so does this. */
export function appendToast(list: Toast[], next: Toast, limit: number = TOAST_LIMIT): Toast[] {
  if (next.text.trim() === '') return list;
  return [...list, next].slice(-limit);
}

export function removeToast(list: Toast[], id: number): Toast[] {
  return list.filter((t) => t.id !== id);
}

const state = $state<{ list: Toast[] }>({ list: [] });

let nextId = 0;

export const toasts = {
  get list() {
    return state.list;
  },
};

/**
 * Shows `text` for [`TOAST_DURATION_MS`]. Returns the new toast's id, or
 * `null` when the message was blank and nothing was shown.
 */
export function pushToast(text: string, level: ToastLevel = 'success'): number | null {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  nextId += 1;
  const id = nextId;
  state.list = appendToast(state.list, { id, text: trimmed, level });
  setTimeout(() => dismissToast(id), TOAST_DURATION_MS);
  return id;
}

export function dismissToast(id: number): void {
  state.list = removeToast(state.list, id);
}
```

- [ ] **Step 4: Implement the surface** `app/src/lib/Toast.svelte`:

```svelte
<script lang="ts">
  // The single toast surface, mounted once by Shell.svelte. Bottom-centred
  // above the download strip, exactly where `ToastWidget._reposition`
  // (toast.py:90-95) puts Python's: horizontally centred, 24px above the
  // bottom edge. Pointer-events are off, matching
  // `WA_TransparentForMouseEvents` (toast.py:27) — a toast never blocks a
  // click on what is under it, and there is nothing to dismiss by hand.
  import { toasts } from './stores/toasts.svelte';
</script>

{#if toasts.list.length > 0}
  <div data-testid="toast-region" class="toasts" role="status" aria-live="polite">
    {#each toasts.list as toast (toast.id)}
      <p data-testid="toast" class="toast" class:error={toast.level === 'error'}>{toast.text}</p>
    {/each}
  </div>
{/if}

<style>
  .toasts {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    /* Above the fixed download strip, the same clearance `.view` uses. */
    bottom: calc(var(--footer-h) + 24px);
    /* Over the details dialog (z 20), so a toast raised from inside it is
       still visible. */
    z-index: 30;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: none;
  }

  .toast {
    margin: 0;
    max-width: 480px;
    padding: 10px 14px;
    box-sizing: border-box;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    text-align: center;
    overflow-wrap: anywhere;
    /* The one literal rgba in this plan: copied from Shell.svelte's
       `.server-menu`, the other floating panel in the shell. */
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    animation: toast-in var(--m-base) ease;
  }

  .toast.error {
    color: var(--danger);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
```

- [ ] **Step 5: Mount it in `Shell.svelte`.** Add `import Toast from './Toast.svelte';` after the `import Settings from './Settings.svelte';` line (`:7`), and add the element directly after the `<DownloadsFooter … />` line (`:205`), with this comment:

```svelte
<!-- Mounted here for the same reason as the footer strip: `position: fixed`
     global chrome inside a `hidden` view root would vanish with that view. -->
<Toast />
```

- [ ] **Step 6: Run** from `app/`: `npx vitest run` (green) and `npm run check` (no new warnings).

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/stores/toasts.svelte.ts app/src/lib/stores/toasts.test.ts app/src/lib/Toast.svelte app/src/lib/Shell.svelte
git commit -m "rewrite: add the app-wide toast surface"
```

---

### Task 2: The emulator form's five cloud fields, the Arguments label and the add toast

**Files:**
- Create: `app/src/lib/emulators/form.ts`
- Create: `app/src/lib/emulators/form.test.ts`
- Modify: `app/src/lib/emulators/EmulatorForm.svelte` (script `:1-102`, markup `:105-136`, styles `:138-215`)

**Interfaces:**
- Consumes: `EmulatorEntry` (`app/src/lib/api.ts:137-151`) — `save_strategy`, `ignore_files`, `ignore_extensions`, `save_paths`, `state_paths` already round-trip through `save_emulator` (`app/src-tauri/src/commands.rs:649-723`, `crates/grid-core/src/config.rs:34-48`).
- Consumes: `pushToast` from Task 1.
- Produces: `export const SAVE_STRATEGIES: readonly ['auto', 'single_file', 'folder']`
- Produces: `export type SaveStrategy = (typeof SAVE_STRATEGIES)[number]`
- Produces: `export const ARGS_LABEL: string`
- Produces: `export type EmulatorFormValues = { name: string; path: string; args: string; saveStrategy: SaveStrategy; ignoreFiles: string; ignoreExtensions: string; savePaths: string; statePaths: string }`
- Produces: `export function normalizeSaveStrategy(raw: string | null | undefined): SaveStrategy`
- Produces: `export function emulatorFormValues(entry: EmulatorEntry | null): EmulatorFormValues`
- Produces: `export function entryPatch(values: EmulatorFormValues): Pick<EmulatorEntry, 'name' | 'path' | 'args' | 'save_strategy' | 'ignore_files' | 'ignore_extensions' | 'save_paths' | 'state_paths'>`
- Produces: `export function addedEmulatorToast(name: string): string`

- [ ] **Step 1: Write the failing test** `app/src/lib/emulators/form.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { EmulatorEntry } from '../api';
import {
  addedEmulatorToast,
  ARGS_LABEL,
  emulatorFormValues,
  entryPatch,
  normalizeSaveStrategy,
  SAVE_STRATEGIES,
} from './form';

describe('SAVE_STRATEGIES', () => {
  it('is the reference dialog list, in order', () => {
    expect([...SAVE_STRATEGIES]).toEqual(['auto', 'single_file', 'folder']);
  });
});

describe('ARGS_LABEL', () => {
  it('is the reference label verbatim', () => {
    expect(ARGS_LABEL).toBe('Arguments (%rom%, %core%, %ps3_launch_target%)');
  });
});

describe('normalizeSaveStrategy', () => {
  it('maps blank, unknown and undefined to auto', () => {
    expect(normalizeSaveStrategy('')).toBe('auto');
    expect(normalizeSaveStrategy('   ')).toBe('auto');
    expect(normalizeSaveStrategy('not-a-strategy')).toBe('auto');
    expect(normalizeSaveStrategy(undefined)).toBe('auto');
    expect(normalizeSaveStrategy(null)).toBe('auto');
  });

  it('maps every single_file alias', () => {
    for (const alias of ['singlefile', 'single_file', 'single-file', 'single file', 'file']) {
      expect(normalizeSaveStrategy(alias)).toBe('single_file');
      expect(normalizeSaveStrategy(`  ${alias.toUpperCase()}  `)).toBe('single_file');
    }
  });

  it('maps every folder alias', () => {
    for (const alias of ['folder', 'directory', 'folder_per_game', 'folder-per-game']) {
      expect(normalizeSaveStrategy(alias)).toBe('folder');
    }
  });
});

describe('emulatorFormValues', () => {
  it('seeds every field empty for an add', () => {
    expect(emulatorFormValues(null)).toEqual({
      name: '',
      path: '',
      args: '',
      saveStrategy: 'auto',
      ignoreFiles: '',
      ignoreExtensions: '',
      savePaths: '',
      statePaths: '',
    });
  });

  it('seeds from an entry and normalizes the stored strategy alias', () => {
    const entry: EmulatorEntry = {
      name: 'DuckStation',
      path: '/opt/duckstation',
      args: '%rom%',
      save_strategy: 'single-file',
      ignore_files: 'a.bin;b.bin',
      ignore_extensions: '.tmp;.log',
      save_paths: 'memcards',
      state_paths: 'savestates',
    };
    expect(emulatorFormValues(entry)).toEqual({
      name: 'DuckStation',
      path: '/opt/duckstation',
      args: '%rom%',
      saveStrategy: 'single_file',
      ignoreFiles: 'a.bin;b.bin',
      ignoreExtensions: '.tmp;.log',
      savePaths: 'memcards',
      statePaths: 'savestates',
    });
  });

  it('seeds a missing optional field as blank', () => {
    const entry: EmulatorEntry = { name: 'Bare', path: '/x', args: '' };
    const values = emulatorFormValues(entry);
    expect(values.saveStrategy).toBe('auto');
    expect(values.ignoreFiles).toBe('');
    expect(values.statePaths).toBe('');
  });
});

describe('entryPatch', () => {
  const values = {
    name: '  RetroArch  ',
    path: '/opt/retroarch',
    args: '-L "%core%" "%rom%"',
    saveStrategy: 'folder' as const,
    ignoreFiles: '  a.bin;b.bin  ',
    ignoreExtensions: '  .tmp  ',
    savePaths: '  saves  ',
    statePaths: '  states  ',
  };

  it('trims the name and the four semicolon lists', () => {
    expect(entryPatch(values)).toEqual({
      name: 'RetroArch',
      path: '/opt/retroarch',
      args: '-L "%core%" "%rom%"',
      save_strategy: 'folder',
      ignore_files: 'a.bin;b.bin',
      ignore_extensions: '.tmp',
      save_paths: 'saves',
      state_paths: 'states',
    });
  });

  it('leaves the path and args exactly as typed', () => {
    const patch = entryPatch({ ...values, path: '  /spaced/path  ', args: '  %rom%  ' });
    expect(patch.path).toBe('  /spaced/path  ');
    expect(patch.args).toBe('  %rom%  ');
  });

  it('always writes a strategy, never a blank', () => {
    expect(entryPatch({ ...values, saveStrategy: 'auto' }).save_strategy).toBe('auto');
  });
});

describe('addedEmulatorToast', () => {
  it('is the reference toast verbatim', () => {
    expect(addedEmulatorToast('RetroArch (Multi-System)')).toBe(
      "Added emulator 'RetroArch (Multi-System)'.",
    );
  });

  it('uses the trimmed name, as the backend stores it', () => {
    expect(addedEmulatorToast('  Dolphin  ')).toBe("Added emulator 'Dolphin'.");
  });
});
```

- [ ] **Step 2: Run** from `app/`: `npx vitest run src/lib/emulators/form.test.ts` — expect failure (module missing).

- [ ] **Step 3: Implement** `app/src/lib/emulators/form.ts`:

```ts
// Pure helpers for the manual add/edit emulator form: the five per-emulator
// cloud fields the reference dialog has (parity gap 1) and the Arguments
// label (parity gap 15). No store or API imports, so this stays trivially
// unit-testable — the rule `catalog.ts` and `retroachievements.ts` follow.
import type { EmulatorEntry } from '../api';

/** `EmulatorConfigDialog._save_strategy_values` (dialogs.py:314). */
export const SAVE_STRATEGIES = ['auto', 'single_file', 'folder'] as const;

export type SaveStrategy = (typeof SAVE_STRATEGIES)[number];

/** The Arguments row's label, verbatim from dialogs.py:362. */
export const ARGS_LABEL = 'Arguments (%rom%, %core%, %ps3_launch_target%)';

export type EmulatorFormValues = {
  name: string;
  path: string;
  args: string;
  saveStrategy: SaveStrategy;
  ignoreFiles: string;
  ignoreExtensions: string;
  savePaths: string;
  statePaths: string;
};

/**
 * The frontend half of `normalize_save_strategy`
 * (crates/grid-core/src/autoconfig/entry.rs:87-95): the alias table collapsed
 * to the three values the select offers, so an entry whose stored strategy
 * is an alias (`"single-file"`, written by an older config or by a profile)
 * still selects the right option instead of silently falling back to `auto`.
 */
export function normalizeSaveStrategy(raw: string | null | undefined): SaveStrategy {
  switch ((raw ?? '').trim().toLowerCase()) {
    case 'singlefile':
    case 'single_file':
    case 'single-file':
    case 'single file':
    case 'file':
      return 'single_file';
    case 'folder':
    case 'directory':
    case 'folder_per_game':
    case 'folder-per-game':
      return 'folder';
    default:
      return 'auto';
  }
}

/**
 * `_apply_emulator_values` (dialogs.py:488-519), minus its `%rom%` default
 * for a blank Arguments field: the rewrite opens the add form with an empty
 * Arguments box, which is what both auto-fills gate on and what
 * `e2e/specs/emulators.spec.ts` asserts.
 */
export function emulatorFormValues(entry: EmulatorEntry | null): EmulatorFormValues {
  return {
    name: entry?.name ?? '',
    path: entry?.path ?? '',
    args: entry?.args ?? '',
    saveStrategy: normalizeSaveStrategy(entry?.save_strategy),
    ignoreFiles: entry?.ignore_files ?? '',
    ignoreExtensions: entry?.ignore_extensions ?? '',
    savePaths: entry?.save_paths ?? '',
    statePaths: entry?.state_paths ?? '',
  };
}

/**
 * `entry_payload` (dialogs.py:527-539): the name and the four
 * semicolon-separated lists are trimmed; the strategy is always written
 * (`... or "auto"`, dialogs.py:537) rather than left blank, which is
 * behaviourally identical because `normalize_save_strategy` maps `""` and
 * `"auto"` to the same result. `path` and `args` are passed through exactly
 * as typed — the form has always done that, and the E2E specs set them
 * literally.
 */
export function entryPatch(
  values: EmulatorFormValues,
): Pick<
  EmulatorEntry,
  | 'name'
  | 'path'
  | 'args'
  | 'save_strategy'
  | 'ignore_files'
  | 'ignore_extensions'
  | 'save_paths'
  | 'state_paths'
> {
  return {
    name: values.name.trim(),
    path: values.path,
    args: values.args,
    save_strategy: values.saveStrategy,
    ignore_files: values.ignoreFiles.trim(),
    ignore_extensions: values.ignoreExtensions.trim(),
    save_paths: values.savePaths.trim(),
    state_paths: values.statePaths.trim(),
  };
}

/** `_show_toast` on a new manual entry (emulator_ui_mixin.py:1591), verbatim. */
export function addedEmulatorToast(name: string): string {
  return `Added emulator '${name.trim()}'.`;
}
```

- [ ] **Step 4: Rewrite `EmulatorForm.svelte`'s script state and `save()`.** Replace the three `let form* = $state(...)` lines (`:29-34`) with a seeded values object, and replace `save()` (`:76-102`):

```ts
  import { api, type EmulatorEntry, type ProfileSummary } from '../api';
  import { matchProfileByName, shouldAutoFillFromName } from './catalog';
  import {
    addedEmulatorToast,
    ARGS_LABEL,
    emulatorFormValues,
    entryPatch,
    SAVE_STRATEGIES,
  } from './form';
  import { pushToast } from '../stores/toasts.svelte';
```

```ts
  // Seeded from `entry` once, on purpose: the fields are then the user's to
  // edit. A parent that switches which entry is edited remounts this with
  // `{#key entry.name}` rather than expecting the fields to track the prop.
  // svelte-ignore state_referenced_locally
  const seed = emulatorFormValues(entry);
  let formName = $state(seed.name);
  let formPath = $state(seed.path);
  let formArgs = $state(seed.args);
  let formSaveStrategy = $state<string>(seed.saveStrategy);
  let formIgnoreFiles = $state(seed.ignoreFiles);
  let formIgnoreExtensions = $state(seed.ignoreExtensions);
  let formSavePaths = $state(seed.savePaths);
  let formStatePaths = $state(seed.statePaths);
  let formError = $state<string | null>(null);
  let formPending = $state(false);
  let autofillMatch = $state<ProfileSummary | null>(null);
```

```ts
  async function save() {
    // `originalName` is what `save_emulator` uses to find-and-replace a
    // renamed entry; blank means "insert". The fields the form does not
    // show (install provenance, autoconfig paths) are spread back from
    // `entry` untouched instead of being dropped on save.
    const originalName = mode === 'add' ? '' : (entry?.name ?? '');
    const patch = entryPatch({
      name: formName,
      path: formPath,
      args: formArgs,
      saveStrategy: normalizeSaveStrategy(formSaveStrategy),
      ignoreFiles: formIgnoreFiles,
      ignoreExtensions: formIgnoreExtensions,
      savePaths: formSavePaths,
      statePaths: formStatePaths,
    });
    const next: EmulatorEntry = {
      ...(mode === 'edit' && entry ? entry : {}),
      ...patch,
    };
    formError = null;
    formPending = true;
    try {
      await api.saveEmulator(originalName, next);
      // `_save_emulator_entry` toasts only for a NEW manual entry
      // (emulator_ui_mixin.py:1590-1591) — an edit stays silent.
      if (mode === 'add') pushToast(addedEmulatorToast(patch.name));
      onSaved();
    } catch (err) {
      formError = errorMessage(err);
    } finally {
      formPending = false;
    }
  }
```

Add `normalizeSaveStrategy` to the `./form` import list.

- [ ] **Step 5: Replace the Arguments row and add the five fields** in the markup, between the existing Arguments label (`:129`) and the `{#if formError}` line (`:130`):

```svelte
  <label>
    <span data-testid="emu-form-args-label">{ARGS_LABEL}</span>
    <input data-testid="emu-form-args" bind:value={formArgs} />
  </label>
  <label>
    Save Strategy
    <select data-testid="emu-form-save-strategy" bind:value={formSaveStrategy}>
      {#each SAVE_STRATEGIES as strategy (strategy)}
        <option value={strategy}>{strategy}</option>
      {/each}
    </select>
  </label>
  <label>
    Ignore Files (; separated)
    <input data-testid="emu-form-ignore-files" bind:value={formIgnoreFiles} />
  </label>
  <label>
    Ignore Extensions (; separated)
    <input data-testid="emu-form-ignore-extensions" bind:value={formIgnoreExtensions} />
  </label>
  <label>
    Save Dirs (; separated)
    <input data-testid="emu-form-save-paths" bind:value={formSavePaths} />
  </label>
  <label>
    State Dirs (; separated)
    <input data-testid="emu-form-state-paths" bind:value={formStatePaths} />
  </label>
```

(The five labels are `dialogs.py:369, 373, 377, 381, 385` verbatim.) The old one-line `<label>Arguments <input data-testid="emu-form-args" …></label>` is deleted.

- [ ] **Step 6: Style the new select** — add after the `input:focus-visible` rule (`:169`):

```css
  select {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  select:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
```

- [ ] **Step 7: Run** from `app/`: `npx vitest run` and `npm run check` — green, no new warnings.

- [ ] **Step 8: Commit**

```bash
git add app/src/lib/emulators/form.ts app/src/lib/emulators/form.test.ts app/src/lib/emulators/EmulatorForm.svelte
git commit -m "rewrite: edit the five per-emulator cloud fields and toast a manual add"
```

---

### Task 3: Per-emulator controller setup notes

**Files:**
- Create: `app/src/lib/emulators/notes.ts`
- Create: `app/src/lib/emulators/notes.test.ts`
- Modify: `app/src/lib/Emulators.svelte` (import block `:19-41`, the installed row `:509-547`, styles near `.args` `:961-967`)

**Interfaces:**
- Produces: `export type EmulatorNote = { key: string; text: string }`
- Produces: `export function emulatorNotes(name: string): EmulatorNote[]`

- [ ] **Step 1: Write the failing test** `app/src/lib/emulators/notes.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { emulatorNotes } from './notes';

describe('emulatorNotes', () => {
  it('returns the Azahar note verbatim', () => {
    expect(emulatorNotes('Azahar')).toEqual([
      {
        key: 'azahar',
        text: 'Controller setup: Settings → Controls → Auto Map  ·  Press Esc to close emulator',
      },
    ]);
  });

  it('returns the Eden note verbatim', () => {
    expect(emulatorNotes('Eden')).toEqual([
      { key: 'eden', text: 'Controller setup: Controls → Configure → Map Controller' },
    ]);
  });

  it('returns the xemu note verbatim', () => {
    expect(emulatorNotes('xemu')).toEqual([
      {
        key: 'xemu',
        text: 'Controller setup: required to connect a controller first — layout is auto-detected',
      },
    ]);
  });

  it('returns the DuckStation note verbatim', () => {
    expect(emulatorNotes('DuckStation')).toEqual([
      {
        key: 'duckstation',
        text: 'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
      },
    ]);
  });

  it('returns the RPCS3 note verbatim', () => {
    expect(emulatorNotes('RPCS3')).toEqual([
      { key: 'rpcs3', text: 'Controller setup: Configure controllers via Config → Pads' },
    ]);
  });

  it('matches case-insensitively anywhere in the name, like the reference token test', () => {
    expect(emulatorNotes('My DuckStation build').map((n) => n.key)).toEqual(['duckstation']);
    expect(emulatorNotes('  rpcs3-nightly  ').map((n) => n.key)).toEqual(['rpcs3']);
  });

  it('returns nothing for an emulator with no note', () => {
    expect(emulatorNotes('RetroArch (Multi-System)')).toEqual([]);
    expect(emulatorNotes('')).toEqual([]);
  });

  it('keeps the reference order when a name matches more than one token', () => {
    expect(emulatorNotes('Eden and xemu combo').map((n) => n.key)).toEqual(['eden', 'xemu']);
  });
});
```

- [ ] **Step 2: Run** from `app/`: `npx vitest run src/lib/emulators/notes.test.ts` — expect failure (module missing).

- [ ] **Step 3: Implement** `app/src/lib/emulators/notes.ts`:

```ts
// The static per-emulator setup notes the reference renders under each
// Installed row (emulator_ui_mixin.py:712-720, 721-728, 749-757, 758-766,
// 767-775). Text is verbatim, including the arrows, the middle dot and the
// em dash.
//
// Matching: a case-folded SUBSTRING test of the token against the entry
// name. That is the second half of `_emulator_matches_tokens`
// (cloud_mixin.py:1349-1363, ported at
// crates/grid-core/src/autoconfig/mod.rs:232-239). The first half — the
// autoprofile `match_tokens` lookup — is not available here: the frontend's
// `ProfileSummary` carries only `{ name, args }` (api.ts:153), no token
// list. Every catalog install names its entry after its profile, so the
// substring test covers them; the same simplification is already in
// Emulators.svelte's `isRpcs3`.
//
// The reference's dynamic Eden notes (prod.keys and Switch firmware
// presence, emulator_ui_mixin.py:729-748) are deliberately NOT here: they
// need backend file probes that do not exist yet, and are deferred by the
// 2026-09-05 controller rulings.

export type EmulatorNote = { key: string; text: string };

/** Token → note, in the order the reference emits them. */
const NOTES: readonly EmulatorNote[] = [
  {
    key: 'azahar',
    text: 'Controller setup: Settings → Controls → Auto Map  ·  Press Esc to close emulator',
  },
  { key: 'eden', text: 'Controller setup: Controls → Configure → Map Controller' },
  {
    key: 'xemu',
    text: 'Controller setup: required to connect a controller first — layout is auto-detected',
  },
  {
    key: 'duckstation',
    text: 'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
  },
  { key: 'rpcs3', text: 'Controller setup: Configure controllers via Config → Pads' },
];

export function emulatorNotes(name: string): EmulatorNote[] {
  const haystack = name.trim().toLowerCase();
  if (haystack === '') return [];
  return NOTES.filter((note) => haystack.includes(note.key));
}
```

- [ ] **Step 4: Render them in `Emulators.svelte`.** Add to the import block (after the `EmulatorForm` import, `:41`):

```ts
  import { emulatorNotes } from './emulators/notes';
```

Insert directly after the closing `</div>` of `.row-main` (`:528`), before the `{#if isRpcs3(e.name) && rpcs3Status.get(e.name)}` block:

```svelte
                      {#each emulatorNotes(e.name) as note (note.key)}
                        <p data-testid={`emulator-note-${note.key}-${sanitizeName(e.name)}`} class="note">
                          {note.text}
                        </p>
                      {/each}
```

- [ ] **Step 5: Style the note** — add after the `.args, .meta` rule (`:961-967`):

```css
  /* Deliberately NOT `.hint` and never `.name`: `emulators.spec.ts`'s
     `rowNames()` reads `[data-testid^="emulator-row-"] .name`, and a second
     match per row would break it. Wraps, unlike `.args`, because the notes
     are sentences. */
  .note {
    margin: 0;
    color: var(--text-muted);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
```

- [ ] **Step 6: Run** from `app/`: `npx vitest run` and `npm run check` — green, no new warnings. Then confirm nothing else claims the new ids: `grep -rn "emulator-note-" e2e/specs app/src` — only `Emulators.svelte` and `notes.test.ts`.

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/emulators/notes.ts app/src/lib/emulators/notes.test.ts app/src/lib/Emulators.svelte
git commit -m "rewrite: show the per-emulator controller setup notes on installed rows"
```

---

### Task 4: The cloud auto-sync hint and the first-run library path

**Files:**
- Modify: `app/src/lib/settings/CloudSavesPage.svelte` (markup after the retention row `:89-99`, styles `:169-174`)
- Modify: `app/src/lib/Connect.svelte` (script `:1-6`, submit handler `:8-20`, markup `:21-31`)

**Interfaces:**
- Consumes: `api.setLibraryPath(path: string): Promise<void>` (`app/src/lib/api.ts:356` → `set_library_path`, `app/src-tauri/src/commands.rs:335-346`).
- Consumes: `connect(serverUrl, username, secret, useToken)` (`app/src/lib/stores/session.svelte.ts:19`).

- [ ] **Step 1: Add the auto-sync hint.** In `CloudSavesPage.svelte`, directly after the "Save retention limit" `<label>` block (ends `:99`) and before the `{#if cloudSettingsError}` line, insert:

```svelte
    <!-- grid-launcher.py:1733-1738, verbatim. -->
    <p data-testid="cloud-settings-autosync-hint" class="hint">
      Auto-sync applies to emulator-based games and uses the latest server save record only.
    </p>
```

The existing `.hint` rule (`:169-174`) already styles it; no CSS change.

- [ ] **Step 2: Add the library path to `Connect.svelte`.** Replace the script block (`:1-6`) with:

```svelte
<script lang="ts">
  import { api } from './api';
  import { session, connect } from './stores/session.svelte';
  let serverUrl = $state('');
  let username = $state('');
  let secret = $state('');
  let useToken = $state(true);
  // `FirstRunDialog` asks for the library path alongside the server details
  // (dialogs.py:133-146). A folder picker is a separate plan; this is the
  // free-text half only.
  let libraryPath = $state('');

  async function submit() {
    // Written BEFORE the connect, exactly as `FirstRunDialog`'s "Save and
    // Continue" persists all three values before the app tries the server
    // (grid-launcher.py:1689): a rejected credential must not lose the path
    // the user just typed. Safe in either order — `SessionManager::connect`
    // re-reads config.toml and overwrites only `server_url`/`username`
    // (crates/grid-core/src/session.rs:124-127).
    const path = libraryPath.trim();
    if (path !== '') {
      try {
        await api.setLibraryPath(path);
      } catch {
        // Best-effort: a path that cannot be stored must not block the
        // connect, and Settings can set it afterwards.
      }
    }
    // Token auth identifies the account by itself; the username input only
    // exists for Basic mode, so never send a stale one alongside a token.
    await connect(serverUrl, useToken ? '' : username, secret, useToken);
    secret = ''; // never keep the plain secret in frontend state
  }
</script>
```

Replace the `onsubmit` handler (`:10-19`) with:

```svelte
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
```

and add the field after the `Use API token` label (`:28`):

```svelte
  <label>
    Library Path
    <input data-testid="connect-library-path" bind:value={libraryPath} placeholder="/home/you/Games" />
  </label>
```

- [ ] **Step 3: Widen the input selector.** `Connect.svelte`'s style block styles `input:not([type])` and `input[type='password']` (`:69-77`); the new field has no `type`, so it is already covered. No CSS change — confirm by reading the rule.

- [ ] **Step 4: Run** from `app/`: `npm run check` (no new warnings) and `npx vitest run` (green).

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/settings/CloudSavesPage.svelte app/src/lib/Connect.svelte
git commit -m "rewrite: restore the cloud auto-sync hint and the first-run library path field"
```

---

### Task 5: Open Config Folder

**Files:**
- Modify: `app/src-tauri/src/commands.rs` (new `config_dir_for` + `open_config_folder` after `open_server_page` `:431-441`; test module near `:2011`)
- Modify: `app/src-tauri/src/lib.rs` (`invoke_handler` list, after `commands::open_server_page` `:286`)
- Modify: `app/src/lib/api.ts` (after `openServerPage`)
- Modify: `app/src/lib/settings/connection.ts`
- Modify: `app/src/lib/settings/connection.test.ts`
- Modify: `app/src/lib/settings/ConnectionPage.svelte` (script `:1-8`, `.actions` block `:38-58`)

**Interfaces:**
- Produces (Rust): `pub fn config_dir_for(config_path: &Path) -> PathBuf` — the parent of the config file, or `"."` when it has none.
- Produces (Rust): `#[tauri::command] pub async fn open_config_folder(app: tauri::AppHandle) -> Result<(), String>`
- Produces (TS): `openConfigFolder: () => Promise<void>` on `api`
- Produces (TS): `export const OPEN_CONFIG_FOLDER_LABEL: string`
- Consumes: `tauri_plugin_opener::OpenerExt` — already imported at `commands.rs:30` and the plugin is already registered (`lib.rs:104`, `app/src-tauri/Cargo.toml:26`). It is a **Rust-side** call, so `capabilities/default.json`'s `core:default` is sufficient — `open_server_page` already proves this.

- [ ] **Step 1: Write the failing Rust test.** Add to `commands.rs`'s test area (a new `#[cfg(test)] mod config_dir_tests` beside `mod retroachievements_tests` at `:2011`):

```rust
#[cfg(test)]
mod config_dir_tests {
    use super::config_dir_for;
    use std::path::{Path, PathBuf};

    #[test]
    fn config_dir_is_the_config_files_parent() {
        assert_eq!(
            config_dir_for(Path::new("/home/six/.config/grid-launcher/config.toml")),
            PathBuf::from("/home/six/.config/grid-launcher")
        );
    }

    #[test]
    fn config_dir_falls_back_to_the_current_directory() {
        assert_eq!(config_dir_for(Path::new("config.toml")), PathBuf::from("."));
    }
}
```

- [ ] **Step 2: Write the failing TS test.** Append to `app/src/lib/settings/connection.test.ts`:

```ts
import { OPEN_CONFIG_FOLDER_LABEL } from './connection';

describe('OPEN_CONFIG_FOLDER_LABEL', () => {
  it('is the reference button text verbatim', () => {
    expect(OPEN_CONFIG_FOLDER_LABEL).toBe('Open Config Folder');
  });
});
```

(Keep the file's existing `import { describe, expect, it } from 'vitest';` and merge the new symbol into the existing `./connection` import rather than adding a second import statement.)

- [ ] **Step 3: Run** `cargo test -p app config_dir` and, from `app/`, `npx vitest run src/lib/settings/connection.test.ts` — both fail (missing items).

- [ ] **Step 4: Implement the command.** In `commands.rs`, directly after `open_server_page` (`:441`):

```rust
/// The directory `config.toml` lives in — `_config_dir()`'s answer
/// (grid-launcher.py:3163). Split out from [`open_config_folder`] so the
/// path rule is unit-testable without a keyring, a webview or an opener.
pub fn config_dir_for(config_path: &std::path::Path) -> std::path::PathBuf {
    match config_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}

/// Reveals the config directory in the desktop file manager
/// (`_open_config_folder`, grid-launcher.py:3162-3172). Takes NO path
/// argument, for the same reason [`open_server_page`] takes no URL: the
/// frontend cannot choose what gets opened. The directory is created first,
/// matching Python's `mkdir(parents=True, exist_ok=True)`, so a first run
/// that has not written a config yet still opens something.
#[tauri::command]
pub async fn open_config_folder(app: tauri::AppHandle) -> Result<(), String> {
    let dir = tokio::task::spawn_blocking(|| {
        let dir = config_dir_for(&Config::default_path());
        std::fs::create_dir_all(&dir).map_err(|e| format!("Could not open config folder: {e}"))?;
        Ok::<std::path::PathBuf, String>(dir)
    })
    .await
    .map_err(|e| format!("open_config_folder did not finish: {e}"))??;
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| format!("Could not open config folder: {e}"))
}
```

Register it in `lib.rs` after `commands::open_server_page,`:

```rust
            commands::open_config_folder,
```

- [ ] **Step 5: Implement the frontend side.** In `api.ts`, after the `openServerPage` entry:

```ts
  /** Reveals the config directory in the desktop file manager. Takes no path. */
  openConfigFolder: () => invoke<void>('open_config_folder'),
```

In `settings/connection.ts`, append:

```ts
/** `_open_config_folder`'s button text (grid-launcher.py:3163), verbatim. */
export const OPEN_CONFIG_FOLDER_LABEL = 'Open Config Folder';
```

In `ConnectionPage.svelte`, extend the script imports:

```ts
  import { api } from '../api';
  import { credentialStatusLabel, OPEN_CONFIG_FOLDER_LABEL, reconnectEnabled, serverLine } from './connection';
```

and add local state plus the handler at the end of the script block:

```ts
  let configFolderError = $state<string | null>(null);

  async function handleOpenConfigFolder() {
    configFolderError = null;
    try {
      await api.openConfigFolder();
    } catch (err) {
      configFolderError = err instanceof Error ? err.message : String(err);
    }
  }
```

Add the button as the last child of `.actions` (after the Disconnect button, `:57`):

```svelte
  <button
    data-testid="settings-open-config-folder"
    class="secondary"
    onclick={() => {
      handleOpenConfigFolder();
    }}
  >
    {OPEN_CONFIG_FOLDER_LABEL}
  </button>
```

and the error line directly after the closing `</div>` of `.actions`:

```svelte
{#if configFolderError}
  <p data-testid="settings-config-folder-error" class="error" role="alert">{configFolderError}</p>
{/if}
```

- [ ] **Step 6: Run** `cargo test -p app`, both clippy commands, `cargo fmt`; from `app/`: `npx vitest run` and `npm run check` — all green, no new warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/settings/connection.ts app/src/lib/settings/connection.test.ts app/src/lib/settings/ConnectionPage.svelte
git commit -m "rewrite: add Open Config Folder to Settings > Connection"
```

---

### Task 6: Standalone emulator launch

**Files:**
- Modify: `crates/grid-core/src/launch/spawn.rs` (imports `:7-13`, new functions after `prepare_emulator_launch` `:94`, tests `:117+`)
- Modify: `app/src-tauri/src/commands.rs` (import block `:18`, new command after `delete_emulator` `:736`)
- Modify: `app/src-tauri/src/lib.rs` (`invoke_handler`, after `commands::delete_emulator` `:298`)
- Modify: `app/src/lib/api.ts` (after `deleteEmulator` `:368`)
- Modify: `app/src/lib/Emulators.svelte` (script handlers near `:445`, `.row-actions` `:516-527`)
- Modify: `docs/porting/04-emulator-launch.md`

**Interfaces:**
- Produces: `pub fn prepare_standalone_emulator_launch(emulator_name: &str, entry: Option<&EmulatorEntry>) -> Result<(Vec<String>, PathBuf), String>` — argv is the resolved executable and nothing else; the working directory is its parent.
- Produces: `pub fn spawn_standalone_emulator(argv: &[String], working_dir: &Path) -> Result<(), String>`
- Produces: `#[tauri::command] pub async fn launch_emulator(name: String) -> Result<(), String>`
- Produces (TS): `launchEmulator: (name: string) => Promise<void>`
- Consumes: `clean_env()` (`crates/grid-core/src/launch/spawn.rs:109`), `expand_home` (`crates/grid-core/src/library/paths.rs`, imported at `spawn.rs:11`), `emulator_entry_by_name` (`crates/grid-core/src/launch/selection.rs`, already imported at `commands.rs:14`), `pushToast` from Task 1.

- [ ] **Step 1: Write the failing Rust tests** in `spawn.rs`'s `mod tests`:

```rust
    #[test]
    fn standalone_launch_rejects_an_unknown_entry() {
        assert_eq!(
            prepare_standalone_emulator_launch("Ghost", None).unwrap_err(),
            "Emulator 'Ghost' was not found."
        );
    }

    #[test]
    fn standalone_launch_rejects_a_blank_path() {
        let e = entry("   ", "%rom%");
        assert_eq!(
            prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap_err(),
            "Emulator 'Dolphin' has no executable path configured."
        );
    }

    #[test]
    fn standalone_launch_rejects_a_missing_executable() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let e = entry(missing.to_str().unwrap(), "%rom%");
        assert_eq!(
            prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap_err(),
            format!("Emulator executable not found:\n{}", missing.display())
        );
    }

    #[test]
    fn standalone_launch_drops_every_argument_and_uses_the_executables_parent() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("dolphin");
        std::fs::write(&exe, b"").unwrap();
        // Args that would normally be templated: a ROM-less launch takes none.
        let e = entry(exe.to_str().unwrap(), "-b \"%rom%\"");

        let (argv, working_dir) = prepare_standalone_emulator_launch("Dolphin", Some(&e)).unwrap();
        assert_eq!(argv, vec![exe.to_string_lossy().into_owned()]);
        assert_eq!(working_dir, dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn spawn_standalone_runs_the_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let exe = dir.path().join("stub.sh");
        std::fs::write(&exe, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

        let argv = vec![exe.to_string_lossy().into_owned()];
        spawn_standalone_emulator(&argv, dir.path()).unwrap();

        // The reaper thread owns the child; poll for the marker rather than
        // waiting on a handle this API deliberately does not hand back.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(marker.exists(), "the stub never ran");
    }

    #[test]
    fn spawn_standalone_reports_a_failed_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not-there");
        let argv = vec![missing.to_string_lossy().into_owned()];
        let err = spawn_standalone_emulator(&argv, dir.path()).unwrap_err();
        assert!(
            err.starts_with("Failed to launch emulator:\n"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn spawn_standalone_rejects_an_empty_argv() {
        assert_eq!(
            spawn_standalone_emulator(&[], Path::new(".")).unwrap_err(),
            "Failed to launch emulator:\nno executable to run"
        );
    }
```

- [ ] **Step 2: Run** `cargo test -p grid-core standalone` — expect compile failure (functions missing).

- [ ] **Step 3: Implement** in `spawn.rs`. Change the path import at `:8` to `use std::path::{Path, PathBuf};` and add after `prepare_emulator_launch` (`:94`):

```rust
/// Builds the argv and working directory for a ROM-less "open the emulator
/// so I can configure controls" launch — `_launch_emulator_at_index`
/// (emulator_ui_mixin.py:1635-1665). The argv is the resolved executable and
/// nothing else: Python builds `command = [str(emulator_path)]` (:1657) and
/// never templates `entry.args`, so a `%rom%` in the stored arguments cannot
/// leak into a launch that has no ROM.
///
/// The validation chain and its wording follow the reference, minus the
/// ROM checks it has no use for:
///
/// 1. no `entry` → "Emulator '<name>' was not found." (Python's index guard
///    silently returns instead, :1637-1639; a click on a row that vanished
///    is a race worth reporting rather than swallowing)
/// 2. blank `entry.path` → "Emulator '<name>' has no executable path
///    configured." (:1645)
/// 3. the executable is not an existing file → "Emulator executable not
///    found:\n<path>" (:1650)
///
/// Python also calls `_ensure_emulator_sync_settings` before spawning
/// (:1653); the rewrite does not — the autoconfig sync runs at add/install
/// time (D1 call site B) and `launch_game` does not re-run it either.
pub fn prepare_standalone_emulator_launch(
    emulator_name: &str,
    entry: Option<&EmulatorEntry>,
) -> Result<(Vec<String>, PathBuf), String> {
    let name = emulator_name.trim();

    let Some(entry) = entry else {
        return Err(format!("Emulator '{name}' was not found."));
    };

    let configured_path = entry.path.trim();
    if configured_path.is_empty() {
        return Err(format!(
            "Emulator '{name}' has no executable path configured."
        ));
    }

    let executable = expand_home(configured_path);
    if !executable.is_file() {
        return Err(format!(
            "Emulator executable not found:\n{}",
            executable.display()
        ));
    }

    let working_dir = match executable.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };

    Ok((vec![executable.to_string_lossy().into_owned()], working_dir))
}

/// Spawns a standalone emulator and returns as soon as it has started —
/// Python's bare `subprocess.Popen` (emulator_ui_mixin.py:1655-1661). The
/// child gets [`clean_env`] and, on Windows, its own process group, exactly
/// like `spawn_child` in `launch/mod.rs` and Python's
/// `CREATE_NEW_PROCESS_GROUP` (:1660).
///
/// A detached thread owns the [`std::process::Child`] and blocks in `wait()`
/// purely so the process is reaped when the emulator exits — the same
/// arrangement (and the same reason) as
/// [`crate::firmware::rpcs3::spawn_rpcs3_installfw`]. There is no session
/// row for a ROM-less launch, so nothing else is watching it.
///
/// Python then warns 500ms later if the process already died
/// (`_warn_if_process_exited_early`, :1662). That is not ported: the rewrite
/// has no modal warning surface, and a deliberate deviation is better than a
/// half-modelled one.
pub fn spawn_standalone_emulator(argv: &[String], working_dir: &Path) -> Result<(), String> {
    let Some(program) = argv.first() else {
        return Err("Failed to launch emulator:\nno executable to run".to_string());
    };

    let mut command = std::process::Command::new(program);
    command
        .args(&argv[1..])
        .current_dir(working_dir)
        .env_clear()
        .envs(clean_env());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(e) => Err(format!("Failed to launch emulator:\n{e}")),
    }
}
```

- [ ] **Step 4: Run** `cargo test -p grid-core spawn::` — green.

- [ ] **Step 5: Add the command.** In `commands.rs`, extend the launch import at `:18`:

```rust
use grid_core::launch::spawn::{prepare_standalone_emulator_launch, spawn_standalone_emulator};
```

and add after `delete_emulator` (`:736`):

```rust
/// Opens a configured emulator with no ROM, so the user can set its controls
/// up (`_launch_emulator_at_index`, emulator_ui_mixin.py:1635-1665). Returns
/// as soon as the process has started; every failure is a plain, path-only
/// message the Emulators view shows as a toast.
#[tauri::command]
pub async fn launch_emulator(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        let entry = emulator_entry_by_name(&config.emulators, &name);
        let (argv, working_dir) = prepare_standalone_emulator_launch(&name, entry)?;
        spawn_standalone_emulator(&argv, &working_dir)
    })
    .await
    .map_err(|e| format!("launch_emulator did not finish: {e}"))?
}
```

Register in `lib.rs` after `commands::delete_emulator,`:

```rust
            commands::launch_emulator,
```

- [ ] **Step 6: Add the button.** In `api.ts`, after `deleteEmulator`:

```ts
  /** Opens the emulator with no ROM, so its own settings UI can be reached. */
  launchEmulator: (name: string) => invoke<void>('launch_emulator', { name }),
```

In `Emulators.svelte`, add to the imports:

```ts
  import { pushToast } from './stores/toasts.svelte';
```

add a handler beside `handleDeleteClick` (after `:445`):

```ts
  // Set for the row whose launch is in flight, so its button disables and
  // says "Launching…" — the click spawns a process and gives no other
  // feedback until it fails.
  let launchPending = $state<string | null>(null);

  async function handleLaunchClick(name: string) {
    launchPending = name;
    try {
      await api.launchEmulator(name);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      launchPending = null;
    }
  }
```

and add the button as the FIRST child of `.row-actions` (before the Edit button, `:517`):

```svelte
                          <button
                            data-testid={`emulator-launch-${sanitizeName(e.name)}`}
                            disabled={launchPending === e.name}
                            onclick={() => handleLaunchClick(e.name)}
                          >
                            {launchPending === e.name ? 'Launching…' : 'Launch'}
                          </button>
```

- [ ] **Step 7: Update `docs/porting/04-emulator-launch.md`** — add a short "Standalone emulator launch" subsection under the launch-command section: the reference builds `[executable]` with the executable's parent as cwd and a cleaned environment (emulator_ui_mixin.py:1655-1661); the rewrite's `prepare_standalone_emulator_launch`/`spawn_standalone_emulator` (`crates/grid-core/src/launch/spawn.rs`) do the same, with two recorded deviations — no `_ensure_emulator_sync_settings` pre-pass (the sync runs at add/install time) and no 500 ms early-exit warning (no modal surface).

- [ ] **Step 8: Run** `cargo test --workspace`, both clippy commands, `cargo fmt`; from `app/`: `npx vitest run` and `npm run check`.

- [ ] **Step 9: Commit**

```bash
git add crates/grid-core/src/launch/spawn.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/Emulators.svelte ../docs/porting/04-emulator-launch.md
git commit -m "rewrite: launch a configured emulator with no ROM from the Installed list"
```

---

### Task 7: RetroAchievements username + password login

**Files:**
- Create: `crates/grid-core/src/retroachievements.rs`
- Create: `crates/grid-core/tests/ra_login.rs`
- Modify: `crates/grid-core/src/lib.rs` (`pub mod` list `:4-15`)
- Modify: `scripts/check_secret_hygiene.sh` (`allowed_files` `:8`)
- Modify: `app/src-tauri/src/commands.rs` (`store_ra_credentials` extracted from `set_retroachievements_credentials` `:1073-1105`; new `retroachievements_login`)
- Modify: `app/src-tauri/src/lib.rs` (`invoke_handler`, after `commands::set_retroachievements_credentials` `:308`)
- Modify: `app/src/lib/api.ts` (RA block `:385-388`, types near `:155-156`)
- Modify: `app/src/lib/emulators/retroachievements.ts`
- Modify: `app/src/lib/emulators/retroachievements.test.ts`
- Modify: `app/src/lib/settings/RetroAchievementsPage.svelte`
- Modify: `docs/porting/01-romm-api.md` (the RetroAchievements login row, `:78`)

**Interfaces:**
- Produces (Rust): `pub const RA_DOREQUEST_URL: &str = "https://retroachievements.org/dorequest.php"`
- Produces (Rust): `pub struct RaLogin { pub username: String, pub token: SecretString }`
- Produces (Rust): `pub fn build_http_client() -> reqwest::Client`
- Produces (Rust): `pub async fn ra_login(http: &reqwest::Client, username: &str, password: &SecretString) -> Result<RaLogin, String>`
- Produces (Rust): `pub async fn ra_login_with_base(http: &reqwest::Client, base_url: &str, username: &str, password: &SecretString) -> Result<RaLogin, String>`
- Produces (Rust): `pub struct RaLoginResult { pub username: String, pub fan_out: Vec<RaFanOutRow> }` (Serialize; carries no token)
- Produces (Rust): `#[tauri::command] pub async fn retroachievements_login(state: State<'_, AppState>, username: String, password: String) -> Result<RaLoginResult, String>`
- Produces (Rust): `fn store_ra_credentials(ra_store: Arc<dyn RaTokenStore>, username: String, token: SecretString) -> Result<Vec<RaFanOutRow>, String>` — the blocking body shared by the login and paste-a-token commands.
- Produces (TS): `export type RaLoginResult = { username: string; fan_out: RaFanOutRow[] }`; `retroachievementsLogin: (username: string, password: string) => Promise<RaLoginResult>`
- Produces (TS): `canLogin`, `loginToast`, `loginFailedToast`, `LOGIN_MISSING_FIELDS_TOAST`, `CREDENTIALS_CLEARED_TOAST`
- Consumes: `RaTokenStore` (`crates/grid-core/src/secrets.rs:36-41`), `AppState.ra_store` (`commands.rs:36-37`), `autoconfig::fan_out_ra_credentials` (`crates/grid-core/src/autoconfig/mod.rs:343`), `RaCredentials::new` (`autoconfig/mod.rs:47`).

- [ ] **Step 1: Write the failing integration test** `crates/grid-core/tests/ra_login.rs`, modelled on `crates/grid-core/tests/romm_detail.rs:11-20`'s MockServer setup:

```rust
use grid_core::retroachievements::{build_http_client, ra_login_with_base, RaLogin};
use secrecy::{ExposeSecret, SecretString};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAKE_PASSWORD: &str = "pw-FAKE";

async fn login(server: &MockServer) -> Result<RaLogin, String> {
    ra_login_with_base(
        &build_http_client(),
        &format!("{}/dorequest.php", server.uri()),
        "sixdd6",
        &SecretString::from(FAKE_PASSWORD),
    )
    .await
}

#[tokio::test]
async fn login_returns_the_server_reported_user_and_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dorequest.php"))
        .and(query_param("r", "login"))
        .and(query_param("u", "sixdd6"))
        .and(query_param("p", FAKE_PASSWORD))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": true,
            "User": "Sixdd6",
            "Token": "FAKE-RA-TOKEN-not-real"
        })))
        .mount(&server)
        .await;

    let login = login(&server).await.unwrap();
    // The SERVER's spelling wins over what was typed.
    assert_eq!(login.username, "Sixdd6");
    assert_eq!(login.token.expose_secret(), "FAKE-RA-TOKEN-not-real");
}

#[tokio::test]
async fn login_reports_the_servers_error_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": false,
            "Error": "Invalid User/Password combination. Please try again."
        })))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "Invalid User/Password combination. Please try again."
    );
}

#[tokio::test]
async fn login_falls_back_to_invalid_credentials_when_the_server_says_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": false
        })))
        .mount(&server)
        .await;

    assert_eq!(login(&server).await.unwrap_err(), "Invalid credentials");
}

#[tokio::test]
async fn login_rejects_a_success_payload_with_no_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": true,
            "User": "Sixdd6"
        })))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "RetroAchievements login response missing Token"
    );
}

#[tokio::test]
async fn login_maps_an_http_error_to_the_reference_wording() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "RetroAchievements HTTP 503: upstream down"
    );
}

/// Token secrecy: the password is in the query string, so no error may ever
/// carry the URL. The transport failure below is produced by pointing at a
/// port nothing is listening on.
#[tokio::test]
async fn a_transport_failure_never_echoes_the_url_or_the_password() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server); // the port is now closed

    let err = ra_login_with_base(
        &build_http_client(),
        &format!("{uri}/dorequest.php"),
        "sixdd6",
        &SecretString::from(FAKE_PASSWORD),
    )
    .await
    .unwrap_err();

    assert!(
        err.starts_with("RetroAchievements request failed: "),
        "unexpected error: {err}"
    );
    assert!(!err.contains(FAKE_PASSWORD), "password leaked: {err}");
    assert!(!err.contains(&uri), "url leaked: {err}");
}

#[tokio::test]
async fn a_blank_username_or_password_never_reaches_the_network() {
    let server = MockServer::start().await;
    // No Mock is mounted: any request would 404 with a different message.
    let base = format!("{}/dorequest.php", server.uri());
    let http = build_http_client();

    assert_eq!(
        ra_login_with_base(&http, &base, "  ", &SecretString::from(FAKE_PASSWORD))
            .await
            .unwrap_err(),
        "username must be a non-empty string"
    );
    assert_eq!(
        ra_login_with_base(&http, &base, "sixdd6", &SecretString::from("  "))
            .await
            .unwrap_err(),
        "password must be a non-empty string"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run** `cargo test -p grid-core --test ra_login` — expect compile failure (module missing).

- [ ] **Step 3: Implement** `crates/grid-core/src/retroachievements.rs`:

```rust
//! RetroAchievements login client.
//!
//! Ports `ra_login` and the `_fetch_json` error mapping it depends on
//! (`grid_launcher/server/retroachievements.py:22-45, 68-89`), reached from
//! `RALoginWorker` (`grid_launcher/background/workers.py:799-816`) and
//! `_ra_login_clicked` (grid-launcher.py:2705-2756). Only the login endpoint
//! is ported: the achievement-list calls next to it in the Python module
//! belong to the achievements panel, which is a documented exclusion.
//!
//! **Token secrecy.** The endpoint takes the password as a QUERY PARAMETER
//! (`?r=login&u=<user>&p=<password>`), so the request URL is itself a
//! secret. Nothing here ever puts the URL, the password, or the returned
//! token in an error, a log line or a `Debug` rendering:
//!
//! * every `reqwest::Error` goes through `.without_url()` before it is
//!   formatted, the same rule `romm/mod.rs:73, 92` follows;
//! * the returned token is a [`SecretString`], which redacts under `Debug`;
//! * [`RaLogin`] derives no `Debug` that could print the token by accident —
//!   `SecretString` handles that, and the struct carries nothing else
//!   sensitive;
//! * this module is on `scripts/check_secret_hygiene.sh`'s `expose_secret`
//!   allowlist for exactly one call: putting the password into the query.

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

/// `_RA_DOREQUEST_URL` (retroachievements.py:9).
pub const RA_DOREQUEST_URL: &str = "https://retroachievements.org/dorequest.php";

/// `Request(url, headers={"User-Agent": ...})` (retroachievements.py:25).
const USER_AGENT: &str = "grid-launcher/1.0 (retroachievements-client)";

/// A successful login: the account name the SERVER reports (never the typed
/// one) and its connect token.
#[derive(Debug)]
pub struct RaLogin {
    pub username: String,
    pub token: SecretString,
}

/// A plain client with a `User-Agent` and a 10s timeout (matching
/// `urlopen(..., timeout=10)`, retroachievements.py:26) and no other default
/// header. It must never share a client with `RommClient`: different host,
/// and the RomM token must never reach retroachievements.org.
pub fn build_http_client() -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(USER_AGENT),
    );
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .default_headers(headers)
        .build()
        .expect("ra http client: static header/timeout config always builds")
}

/// `ra_login` (retroachievements.py:68-89) against the real endpoint.
pub async fn ra_login(
    http: &reqwest::Client,
    username: &str,
    password: &SecretString,
) -> Result<RaLogin, String> {
    ra_login_with_base(http, RA_DOREQUEST_URL, username, password).await
}

/// The `base_url`-parameterised form, so tests can point at a mock server —
/// the same split `pcgw.rs`'s `fetch_windows_save_paths_with_base` uses.
pub async fn ra_login_with_base(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &SecretString,
) -> Result<RaLogin, String> {
    // retroachievements.py:69-72 — validated before anything is sent.
    if username.trim().is_empty() {
        return Err("username must be a non-empty string".to_string());
    }
    if password.expose_secret().trim().is_empty() {
        return Err("password must be a non-empty string".to_string());
    }

    let payload = fetch_json(
        http,
        base_url,
        &[
            ("r", "login"),
            ("u", username),
            // The one `expose_secret` in this crate outside secrets.rs,
            // romm/mod.rs and autoconfig/mod.rs: the endpoint has no other
            // way to take the password.
            ("p", password.expose_secret()),
        ],
    )
    .await?;

    // retroachievements.py:76-88.
    if payload.get("Success").and_then(Value::as_bool) == Some(true) {
        let user = payload.get("User").and_then(Value::as_str).unwrap_or("");
        if user.is_empty() {
            return Err("RetroAchievements login response missing User".to_string());
        }
        let token = payload.get("Token").and_then(Value::as_str).unwrap_or("");
        if token.is_empty() {
            return Err("RetroAchievements login response missing Token".to_string());
        }
        return Ok(RaLogin {
            username: user.to_string(),
            token: SecretString::from(token),
        });
    }

    Err(error_text(&payload).unwrap_or_else(|| "Invalid credentials".to_string()))
}

/// `_fetch_json` (retroachievements.py:22-45). The query is built with
/// reqwest's own encoder rather than by string concatenation so a password
/// with `&`/`=` in it cannot corrupt the request.
async fn fetch_json(
    http: &reqwest::Client,
    base_url: &str,
    query: &[(&str, &str)],
) -> Result<Value, String> {
    let response = http
        .get(base_url)
        .query(query)
        .send()
        .await
        // `.without_url()`: the URL carries the password.
        .map_err(|e| format!("RetroAchievements request failed: {}", e.without_url()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("RetroAchievements request failed: {}", e.without_url()))?;

    if !status.is_success() {
        // retroachievements.py:32-38: a <=300-char body excerpt, or the
        // status line when the body is empty. RetroAchievements answers with
        // its own JSON error object here, never an echo of the request.
        let detail = if body.trim().is_empty() {
            status.to_string()
        } else {
            body.chars().take(300).collect::<String>()
        };
        return Err(format!(
            "RetroAchievements HTTP {}: {detail}",
            status.as_u16()
        ));
    }

    let payload: Value = serde_json::from_str(&body)
        .map_err(|e| format!("RetroAchievements request failed: {e}"))?;

    if !payload.is_object() {
        return Err("RetroAchievements response must be a JSON object".to_string());
    }

    // retroachievements.py:40-42: an explicit `Success: false` or a
    // non-empty `Error` is an error even on a 200.
    if payload.get("Success").and_then(Value::as_bool) == Some(false)
        || error_text(&payload).is_some()
    {
        return Err(error_text(&payload)
            .unwrap_or_else(|| "RetroAchievements returned an error".to_string()));
    }

    Ok(payload)
}

/// `payload.get("Error") or payload.get("Message")`, blank treated as absent.
fn error_text(payload: &Value) -> Option<String> {
    for key in ["Error", "Message"] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}
```

Note: `fetch_json`'s `Success: false` branch runs before `ra_login_with_base`'s own check, so a `Success: false` payload with no `Error` reaches the caller as `"RetroAchievements returned an error"` — which the "falls back to invalid credentials" test contradicts. Resolve it by giving `fetch_json` no `Success` handling at all and leaving the whole `Success`/`Error` decision to `ra_login_with_base`: delete the `Success`-false condition from `fetch_json`'s guard, keep only `if let Some(text) = error_text(&payload) { return Err(text); }`, and keep `ra_login_with_base`'s trailing `Err(error_text(...).unwrap_or("Invalid credentials"))`. Write it that way.

- [ ] **Step 4: Export it.** In `crates/grid-core/src/lib.rs`, add `pub mod retroachievements;` between `pub mod pcgw;` and `pub mod romm;` (alphabetical, matching the existing order).

- [ ] **Step 5: Allowlist the module.** In `scripts/check_secret_hygiene.sh:8`:

```bash
# `retroachievements.rs` is on this list for one call: the RA login endpoint
# takes the password as a query parameter and has no header form.
allowed_files=("crates/grid-core/src/secrets.rs" "crates/grid-core/src/romm/mod.rs" "crates/grid-core/src/autoconfig/mod.rs" "crates/grid-core/src/retroachievements.rs")
```

- [ ] **Step 6: Run** `cargo test -p grid-core --test ra_login` — green. Run `scripts/check_secret_hygiene.sh` — green.

- [ ] **Step 7: Add the command.** In `commands.rs`, extract the blocking body of `set_retroachievements_credentials` (`:1084-1102`) into a shared function placed just above it, and rewrite the command to call it:

```rust
/// The blocking half both credential paths share: store-or-clear the token,
/// write the plain username to config, then fan it out to the RA-capable
/// emulators (D2). Never returns, logs or formats the token.
fn store_ra_credentials(
    ra_store: Arc<dyn RaTokenStore>,
    username: String,
    token: SecretString,
) -> Result<Vec<RaFanOutRow>, String> {
    if token.expose_secret_len_is_blank() {
        ra_store.clear().map_err(err)?;
    } else {
        ra_store.save(&token).map_err(err)?;
    }

    // The fan-out writes emulator config files, never config.json, so it
    // runs outside the write lock on the saved snapshot.
    let config = modify_config(&Config::default_path(), |config| {
        config.retroachievements_username = username.clone();
        Ok(config.clone())
    })?;

    let ra = RaCredentials::new(username, token);
    Ok(ra_fan_out_rows(autoconfig::fan_out_ra_credentials(
        &config,
        load_profiles(),
        &ra,
    )))
}
```

`expose_secret_len_is_blank` does not exist and must not be invented: pass the blankness in as a parameter instead, computed by the caller from the plain `String` before it is wrapped, exactly as `set_retroachievements_credentials` does today (`commands.rs:1078`). Final signature:

```rust
fn store_ra_credentials(
    ra_store: Arc<dyn RaTokenStore>,
    username: String,
    token: SecretString,
    token_is_blank: bool,
) -> Result<Vec<RaFanOutRow>, String>
```

with `if token_is_blank { ra_store.clear() … } else { ra_store.save(&token) … }`. `set_retroachievements_credentials` then becomes:

```rust
    tokio::task::spawn_blocking(move || {
        store_ra_credentials(ra_store, trimmed_username, token, token_is_blank)
    })
    .await
    .map_err(|e| format!("set_retroachievements_credentials did not finish: {e}"))?
```

Add the new command directly after it:

```rust
/// What [`retroachievements_login`] returns: the account name the RA server
/// reported and the fan-out rows. NEVER the token — the only place it lands
/// is `AppState.ra_store`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaLoginResult {
    pub username: String,
    pub fan_out: Vec<RaFanOutRow>,
}

/// Username + password login (`_ra_login_clicked` / `_on_ra_login_finished`,
/// grid-launcher.py:2705-2756). The password is wrapped in `SecretString`
/// immediately, used once, and dropped at the end of this scope; it is never
/// written to config, never logged, and never present in an error — the
/// login client strips the URL from every transport failure.
///
/// On success the SERVER's spelling of the account name is what gets stored,
/// matching the reference (`bundle["username"]` is `result["username"]`,
/// which is the payload's `User`).
#[tauri::command]
pub async fn retroachievements_login(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<RaLoginResult, String> {
    let password = SecretString::from(password);
    let typed_username = username.trim().to_string();
    let ra_store = state.ra_store.clone();

    let http = grid_core::retroachievements::build_http_client();
    let login = grid_core::retroachievements::ra_login(&http, &typed_username, &password).await?;
    let account = login.username.clone();

    let fan_out = tokio::task::spawn_blocking(move || {
        store_ra_credentials(ra_store, login.username, login.token, false)
    })
    .await
    .map_err(|e| format!("retroachievements_login did not finish: {e}"))??;

    Ok(RaLoginResult {
        username: account,
        fan_out,
    })
}
```

Register in `lib.rs` after `commands::set_retroachievements_credentials,`:

```rust
            commands::retroachievements_login,
```

- [ ] **Step 8: Write the failing TS test.** Append to `app/src/lib/emulators/retroachievements.test.ts`:

```ts
describe('canLogin', () => {
  it('needs both fields', () => {
    expect(canLogin('six', 'pw')).toBe(true);
    expect(canLogin('', 'pw')).toBe(false);
    expect(canLogin('six', '')).toBe(false);
    expect(canLogin('   ', '   ')).toBe(false);
  });

  it('does not trim the password, which may legitimately have spaces', () => {
    expect(canLogin('six', '  ')).toBe(true);
  });
});

describe('the RetroAchievements toast texts', () => {
  it('are the reference strings verbatim', () => {
    expect(LOGIN_MISSING_FIELDS_TOAST).toBe('Enter both username and password.');
    expect(loginToast('Sixdd6')).toBe('Logged in as Sixdd6');
    expect(loginFailedToast('Invalid credentials')).toBe('RA login failed: Invalid credentials');
    expect(CREDENTIALS_CLEARED_TOAST).toBe('RetroAchievements credentials cleared.');
  });
});
```

(Merge `canLogin`, `loginToast`, `loginFailedToast`, `LOGIN_MISSING_FIELDS_TOAST`, `CREDENTIALS_CLEARED_TOAST` into the file's existing `./retroachievements` import.)

- [ ] **Step 9: Run** from `app/`: `npx vitest run src/lib/emulators/retroachievements.test.ts` — expect failure.

- [ ] **Step 10: Implement the frontend.** Append to `app/src/lib/emulators/retroachievements.ts`:

```ts
/**
 * `_ra_login_clicked`'s gate (grid-launcher.py:2708-2712): a non-blank
 * username and a non-empty password. The password is checked for emptiness,
 * not blankness — Python reads `text()` without stripping it, and a password
 * made of spaces is a legal password.
 */
export function canLogin(username: string, password: string): boolean {
  return username.trim() !== '' && password !== '';
}

/** grid-launcher.py:2711, verbatim. */
export const LOGIN_MISSING_FIELDS_TOAST = 'Enter both username and password.';

/** grid-launcher.py:2767, verbatim. */
export const CREDENTIALS_CLEARED_TOAST = 'RetroAchievements credentials cleared.';

/** grid-launcher.py:2750, verbatim. Takes the server-reported account name. */
export function loginToast(username: string): string {
  return `Logged in as ${username}`;
}

/** grid-launcher.py:2736, verbatim. */
export function loginFailedToast(error: string): string {
  return `RA login failed: ${error}`;
}
```

In `api.ts`, add the type beside `RaFanOutRow` (`:156`):

```ts
/** `retroachievements_login`'s answer. Carries no token, by construction. */
export type RaLoginResult = { username: string; fan_out: RaFanOutRow[] };
```

and the wrapper after `setRetroachievementsCredentials` (`:386`):

```ts
  /**
   * Username + password login. The password crosses IPC once and is stored
   * nowhere; the token it yields goes straight to the OS keyring and is
   * never part of this answer.
   */
  retroachievementsLogin: (username: string, password: string) =>
    invoke<RaLoginResult>('retroachievements_login', { username, password }),
```

In `RetroAchievementsPage.svelte`: extend the imports

```ts
  import { api, type RaFanOutRow, type RaStatus } from '../api';
  import {
    canLogin,
    canSubmit,
    CREDENTIALS_CLEARED_TOAST,
    fanOutSummary,
    LOGIN_MISSING_FIELDS_TOAST,
    loginFailedToast,
    loginToast,
    statusLabel,
  } from '../emulators/retroachievements';
  import { pushToast } from '../stores/toasts.svelte';
```

add state and a handler beside `handleRaSave`:

```ts
  // Write-only, like the token field: never seeded, never read back, blanked
  // the moment the login resolves either way.
  let raPassword = $state('');
  let raLoginPending = $state(false);

  async function handleRaLogin() {
    if (!canLogin(raUsername, raPassword)) {
      pushToast(LOGIN_MISSING_FIELDS_TOAST, 'error');
      return;
    }
    raError = null;
    raResultLine = null;
    raLoginPending = true;
    try {
      const result = await api.retroachievementsLogin(raUsername, raPassword);
      raPassword = '';
      raToken = '';
      raResultLine = fanOutSummary(result.fan_out);
      pushToast(loginToast(result.username));
      await refreshRaStatus();
    } catch (err) {
      raPassword = '';
      const message = errorMessage(err);
      raError = message;
      pushToast(loginFailedToast(message), 'error');
    } finally {
      raLoginPending = false;
    }
  }
```

and add `pushToast(CREDENTIALS_CLEARED_TOAST);` in `handleRaClear`'s `try` block, directly after `raToken = '';` — plus `raPassword = '';` on the line before it.

Markup: add the password row between the Username and Token labels (`:83`), and the Log In button as the first child of `.form-actions` (`:93`):

```svelte
  <label>
    Password
    <input
      data-testid="ra-password"
      type="password"
      bind:value={raPassword}
      autocomplete="current-password"
    />
  </label>
```

```svelte
    <button
      data-testid="ra-login"
      type="button"
      onclick={handleRaLogin}
      disabled={raLoginPending || !canLogin(raUsername, raPassword)}
    >
      {raLoginPending ? 'Logging in…' : 'Log In'}
    </button>
```

The existing Token field and Save button stay: pasting a token is the rewrite's own path and `updates.spec.ts:380-384` visits this page.

- [ ] **Step 11: Update `docs/porting/01-romm-api.md:78`** — extend the RetroAchievements login row's last column with the rewrite's home for it: `crates/grid-core/src/retroachievements.rs` (`ra_login`), reached by the `retroachievements_login` command; the token goes to the keyring only, the password is never persisted.

- [ ] **Step 12: Run** `cargo test --workspace`, both clippy commands, `cargo fmt`, `scripts/check_secret_hygiene.sh`; from `app/`: `npx vitest run` and `npm run check`.

- [ ] **Step 13: Commit**

```bash
git add crates/grid-core/src/retroachievements.rs crates/grid-core/src/lib.rs crates/grid-core/tests/ra_login.rs scripts/check_secret_hygiene.sh app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/emulators/retroachievements.ts app/src/lib/emulators/retroachievements.test.ts app/src/lib/settings/RetroAchievementsPage.svelte ../docs/porting/01-romm-api.md
git commit -m "rewrite: log in to RetroAchievements with a username and password"
```

---

### Task 8: Persist the window geometry

**Files:**
- Modify: `app/src-tauri/Cargo.toml` (`[dependencies]`, after `tauri-plugin-opener` `:26`)
- Modify: `app/src-tauri/src/lib.rs` (the builder chain `:103-104`)
- Modify: `docs/porting/02-config-and-secrets.md` (the `window_geometry` row `:140` and the key table's surrounding note)

**Interfaces:**
- Produces: no new command or type. The plugin registers a window hook and persists to its own `.window-state.json` in Tauri's AppConfig directory.
- Consumes: `tauri = "2.11.3"` (`app/src-tauri/Cargo.toml:24`). `tauri-plugin-window-state = "2"` is the same major-version pin the file already uses for `tauri-plugin-log` and `tauri-plugin-opener`; the 2.x plugin line targets Tauri 2 and resolves against 2.11.3. **This crate is not in the local cargo registry — the build needs network access to fetch it.**

- [ ] **Step 1: Add the dependency.** In `app/src-tauri/Cargo.toml`, directly after `tauri-plugin-opener = "2"`:

```toml
tauri-plugin-window-state = "2"
```

- [ ] **Step 2: Register the plugin.** In `lib.rs`, change the builder chain at `:103-104` to:

```rust
    // Window geometry across restarts — `_persist_window_geometry` /
    // `_restore_window_geometry` (grid-launcher.py:2362, 2372). The plugin
    // saves size, position and maximized state on exit and restores them
    // when the window is created, replacing Python's base64 blob in
    // config.toml with its own `.window-state.json`.
    //
    // NOT under the `e2e` feature: the plugin writes to Tauri's AppConfig
    // directory, which `GRID_LAUNCHER_DATA_DIR` does not redirect and
    // `scripts/e2e.sh` does not sandbox (it redirects XDG_DATA_HOME,
    // XDG_RUNTIME_DIR and XDG_CACHE_HOME only). Registering it under the
    // harness would write into the developer's real home and make the Xvfb
    // window size carry between stage groups.
    #[cfg(not(feature = "e2e"))]
    let builder = builder.plugin(tauri_plugin_window_state::Builder::new().build());
    builder
        .plugin(tauri_plugin_opener::init())
```

(The existing `#[cfg(feature = "e2e")] let builder = …` block immediately above stays; the two `let builder` rebindings are mutually exclusive by feature and both shadow the original binding.)

- [ ] **Step 3: Build both feature sets.** `cargo build -p app` (fetches the crate) and `cargo build -p app --features e2e`. Both must succeed; the `e2e` build must not link the plugin (confirm with `cargo tree -p app -e normal --features e2e | grep window-state` returning the crate as a dependency but the `#[cfg]` keeping it unused — a `dead_code`/unused-crate warning is not emitted for an unused dependency, so this is only a build check).

- [ ] **Step 4: Update `docs/porting/02-config-and-secrets.md`.** On the `window_geometry` row (`:140`) and the `window_state` row beside it, add: the rewrite does not port these two config keys — `tauri-plugin-window-state` owns the geometry and stores it in its own `.window-state.json` under Tauri's AppConfig directory, because the toolkit blob Python stored is Qt-specific and meaningless to a webview window. Note that the plugin is registered only in non-`e2e` builds and why.

- [ ] **Step 5: Run** `cargo fmt`, both clippy commands, `cargo test --workspace`.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/Cargo.toml Cargo.lock app/src-tauri/src/lib.rs ../docs/porting/02-config-and-secrets.md
git commit -m "rewrite: persist the window geometry across restarts"
```

---

### Task 9: Edit the connection from Settings

**Files:**
- Modify: `app/src/lib/settings/connection.ts`
- Modify: `app/src/lib/settings/connection.test.ts`
- Modify: `app/src/lib/settings/ConnectionPage.svelte` (script `:1-8`, markup after `.rows` `:29`, styles)

**Interfaces:**
- Produces: `export function canConnect(serverUrl: string, secret: string): boolean`
- Consumes: `connect(serverUrl, username, secret, useToken)` and the `session` store (`app/src/lib/stores/session.svelte.ts:19-26`) — the same function `Connect.svelte` calls, which re-probes the server, rewrites `server_url`/`username` in config.toml and re-saves the credential (`crates/grid-core/src/session.rs:92-132`).

- [ ] **Step 1: Write the failing test.** Append to `app/src/lib/settings/connection.test.ts`:

```ts
describe('canConnect', () => {
  it('needs a server URL and a secret', () => {
    expect(canConnect('https://romm.example', 'tok')).toBe(true);
    expect(canConnect('', 'tok')).toBe(false);
    expect(canConnect('   ', 'tok')).toBe(false);
    expect(canConnect('https://romm.example', '')).toBe(false);
  });

  it('does not trim the secret', () => {
    expect(canConnect('https://romm.example', '  ')).toBe(true);
  });
});
```

(Merge `canConnect` into the existing `./connection` import.)

- [ ] **Step 2: Run** from `app/`: `npx vitest run src/lib/settings/connection.test.ts` — expect failure.

- [ ] **Step 3: Implement the helper.** Append to `settings/connection.ts`:

```ts
/**
 * The Edit-connection form's submit gate. Mirrors the `required` attributes
 * on `Connect.svelte`'s two fields: a URL that is more than whitespace and a
 * non-empty secret. The secret is not trimmed — a token or password may
 * legitimately end in one.
 */
export function canConnect(serverUrl: string, secret: string): boolean {
  return serverUrl.trim() !== '' && secret !== '';
}
```

- [ ] **Step 4: Implement the disclosure.** In `ConnectionPage.svelte`, extend the script:

```ts
  import { api } from '../api';
  import { connect, disconnect, retry, session } from '../stores/session.svelte';
  import {
    canConnect,
    credentialStatusLabel,
    OPEN_CONFIG_FOLDER_LABEL,
    reconnectEnabled,
    serverLine,
  } from './connection';
```

```ts
  // The reference's Settings › Server Connection panel (grid-launcher.py:
  // 1601-1623): Server URL + API Token, then Connect. Collapsed by default —
  // the page's job is status; editing is the exception.
  let editing = $state(false);
  let editServerUrl = $state('');
  let editUsername = $state('');
  let editSecret = $state('');
  let editUseToken = $state(true);

  function openEdit() {
    // Seeded from the store's URL only. The secret is never seeded: the
    // store has never held one and the backend never returns one.
    editServerUrl = session.serverUrl;
    editUsername = session.username;
    editSecret = '';
    editUseToken = true;
    editing = true;
  }

  function closeEdit() {
    editing = false;
    editSecret = '';
  }

  async function submitEdit() {
    await connect(editServerUrl, editUseToken ? '' : editUsername, editSecret, editUseToken);
    editSecret = ''; // never keep the plain secret in frontend state
    // `connect` reports through `session.error`; a run that set none
    // succeeded, so the form can close.
    if (session.error === null) editing = false;
  }
```

Markup, inserted directly after the `</dl>` of `.rows` (`:29`) and before the `{#if !session.connected && session.lastError}` block:

```svelte
{#if editing}
  <form
    data-testid="settings-connection-edit-form"
    class="edit"
    onsubmit={(e) => {
      e.preventDefault();
      submitEdit();
    }}
  >
    <label>
      Server URL
      <input data-testid="settings-connection-server-url" bind:value={editServerUrl} required />
    </label>
    {#if !editUseToken}
      <label>
        Username
        <input data-testid="settings-connection-username" bind:value={editUsername} autocomplete="username" />
      </label>
    {/if}
    <label>
      {editUseToken ? 'API Token' : 'Password'}
      <input
        data-testid="settings-connection-secret"
        type="password"
        bind:value={editSecret}
        autocomplete="new-password"
        required
      />
    </label>
    <label class="checkbox">
      <input data-testid="settings-connection-use-token" type="checkbox" bind:checked={editUseToken} />
      Use API token
    </label>
    <div class="actions">
      <button
        data-testid="settings-connection-save"
        type="submit"
        disabled={session.busy || !canConnect(editServerUrl, editSecret)}
      >
        {session.busy ? 'Connecting…' : 'Connect'}
      </button>
      <button data-testid="settings-connection-cancel" type="button" class="secondary" onclick={closeEdit}>
        Cancel
      </button>
    </div>
    {#if session.error}
      <p data-testid="settings-connection-edit-error" class="error" role="alert">{session.error}</p>
    {/if}
  </form>
{/if}
```

and the toggle button as the FIRST child of the existing `.actions` block (before Reconnect, `:39`):

```svelte
  <button
    data-testid="settings-connection-edit"
    class="secondary"
    onclick={() => (editing ? closeEdit() : openEdit())}
  >
    {editing ? 'Close editor' : 'Edit connection'}
  </button>
```

Styles, appended to the block:

```css
  .edit {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 420px;
    margin: 16px 0;
  }

  .edit label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  .edit label.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .edit input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .edit input[type='checkbox'] {
    width: auto;
    padding: 0;
    accent-color: var(--primary);
  }

  .edit input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
```

- [ ] **Step 5: Run** from `app/`: `npx vitest run` and `npm run check` — green, no new warnings. Then confirm the ids are new: `grep -rn "settings-connection-edit\|settings-connection-server-url\|settings-connection-secret\|settings-connection-save\|settings-connection-cancel\|settings-connection-use-token\|settings-connection-username" e2e/specs app/src` — only `ConnectionPage.svelte`.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/settings/connection.ts app/src/lib/settings/connection.test.ts app/src/lib/settings/ConnectionPage.svelte
git commit -m "rewrite: allow editing the server URL and token from Settings > Connection"
```

---

### Task 10: E2E cases and the full gate

**Files:**
- Modify: `e2e/specs/emulators.spec.ts` (`before` hook `:27-66`, new cases)
- Modify: `e2e/specs/connect.spec.ts` (new case at the end)

**Interfaces:** none new — the specs drive the ids Tasks 1-9 added.

- [ ] **Step 1: Make the emulators stub observable.** In `emulators.spec.ts`'s `before` hook, change the stub body so a standalone launch leaves evidence (nothing else in this group ever executes it — the comment at `:19-24` says so explicitly):

```ts
    stubPath = path.join(stubsDir, 'retroarch');
    launchMarker = path.join(stubsDir, 'retroarch.launched');
    // The `emulator-launch-*` case below is the only thing in this group
    // that runs the stub; the marker is how it proves the spawn happened.
    writeFileSync(stubPath, `#!/bin/sh\ntouch '${launchMarker}'\nexit 0\n`);
    chmodSync(stubPath, 0o755);
```

with `let launchMarker: string;` declared beside `let stubPath: string;` and `existsSync` added to the `node:fs` import.

- [ ] **Step 2: Assert the toast and the new form fields.** Extend the existing case `'auto-fills name and args from a profile-matching path, then saves the row'` — after `await $(testId('emu-form-save')).click();` and BEFORE the row wait, insert:

```ts
    // The global toast surface (parity gap 5) with the reference's text
    // (emulator_ui_mixin.py:1591). Asserted before the row wait so it is
    // read well inside TOAST_DURATION_MS.
    await $(testId('toast')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after adding an emulator',
    });
    await expect($(testId('toast'))).toHaveText("Added emulator 'RetroArch (Multi-System)'.");
```

and, before the save click, assert the new label and set the five fields:

```ts
    await expect($(testId('emu-form-args-label'))).toHaveText(
      'Arguments (%rom%, %core%, %ps3_launch_target%)',
    );
    await selectValue('emu-form-save-strategy', 'folder');
    await $(testId('emu-form-ignore-files')).setValue('skip.bin;other.bin');
    await $(testId('emu-form-ignore-extensions')).setValue('.tmp;.log');
    await $(testId('emu-form-save-paths')).setValue('saves');
    await $(testId('emu-form-state-paths')).setValue('states');
```

(`selectValue` is already defined in this spec at the `optionValues` helper's sibling — move its definition above this case if the current position is later in the file.)

- [ ] **Step 3: Add the round-trip case** directly after that one:

```ts
  it('writes the five per-emulator cloud fields to config.toml and reloads them into the edit sheet', async () => {
    await waitForConfigLine('save_strategy = "folder"');
    await waitForConfigLine('ignore_files = "skip.bin;other.bin"');
    await waitForConfigLine('ignore_extensions = ".tmp;.log"');
    await waitForConfigLine('save_paths = "saves"');
    await waitForConfigLine('state_paths = "states"');

    await showPage('installed');
    await $(testId(`emulator-edit-${sanitize('RetroArch (Multi-System)')}`)).click();
    await $(testId('emu-edit-sheet')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('emu-form-save-strategy'))).toHaveValue('folder');
    await expect($(testId('emu-form-ignore-files'))).toHaveValue('skip.bin;other.bin');
    await expect($(testId('emu-form-ignore-extensions'))).toHaveValue('.tmp;.log');
    await expect($(testId('emu-form-save-paths'))).toHaveValue('saves');
    await expect($(testId('emu-form-state-paths'))).toHaveValue('states');
    await $(testId('emu-form-cancel')).click();
  });

  it('launches an installed emulator with no ROM', async () => {
    await showPage('installed');
    const launch = $(testId(`emulator-launch-${sanitize('RetroArch (Multi-System)')}`));
    await expect(launch).toBeDisplayed();
    await launch.click();
    await browser.waitUntil(() => existsSync(launchMarker), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the standalone launch never ran the emulator stub',
    });
  });
```

`waitForConfigLine` is defined later in the file (just above the defaults cases); move its definition up to sit with the other helpers so both call sites can use it. This case must run BEFORE the existing `'adds a second emulator and keeps row order when editing the first'` case, which renames the entry.

- [ ] **Step 4: Assert the controller notes.** RetroArch has no note, so add a small dedicated case after the delete case (which leaves only the renamed RetroArch row):

```ts
  it('shows the DuckStation controller note and none for RetroArch', async () => {
    await $(testId('emulator-add')).click();
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-form-name')).setValue('DuckStation');
    await $(testId('emu-form-path')).setValue('/nonexistent/duckstation');
    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('DuckStation')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the DuckStation row never appeared',
    });

    await expect($(testId('emulator-note-duckstation-duckstation'))).toHaveText(
      'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
    );
    await expect($(testId('emulator-note-azahar-duckstation'))).not.toExist();

    // Clean up so the defaults cases below still see the single RetroArch row.
    const deleteBtn = $(testId(`emulator-delete-${sanitize('DuckStation')}`));
    await deleteBtn.click();
    await deleteBtn.click();
    await $(testId(`emulator-row-${sanitize('DuckStation')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the DuckStation row was not removed',
    });
  });
```

- [ ] **Step 5: Add the Connection-edit case** at the end of `e2e/specs/connect.spec.ts`, after the config.toml test:

```ts
  it('re-connects from Settings › Connection with the same credentials', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-connection')).click();
    await $(testId('settings-connection-url')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the connection settings page never rendered',
    });

    await $(testId('settings-connection-edit')).click();
    await $(testId('settings-connection-server-url')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the edit-connection form never opened',
    });
    await expect($(testId('settings-connection-server-url'))).toHaveValue(mockUrl());
    await $(testId('settings-connection-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('settings-connection-save')).click();

    await $(testId('settings-connection-edit-form')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the edit form stayed open after a successful reconnect',
    });
    await expect($(testId('settings-connection-status'))).toHaveTextContaining('Connected');
    // Still no secret anywhere on disk.
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`server_url = "${mockUrl()}"`);
    expect(text).not.toContain(FIXTURE_TOKEN);
  });

  it('offers Open Config Folder without navigating away', async () => {
    // The opener has nothing to open into under Xvfb, so this asserts the
    // control exists and is reachable — the command's own path rule is unit
    // tested (commands.rs `config_dir_tests`).
    await expect($(testId('settings-open-config-folder'))).toBeDisplayed();
    await expect($(testId('settings-open-config-folder'))).toHaveText('Open Config Folder');
  });
```

Add `TRANSITION_TIMEOUT` to this spec's `../helpers/env.js` import if it is not there already (it is, `connect.spec.ts:9`).

- [ ] **Step 6: Check the library-path field does not break the connect flow.** `connect.spec.ts` never touches `connect-library-path`, and a blank value means `set_library_path` is not called. Confirm with `grep -n "connect-library-path" e2e/specs` (no hits) and re-read `Connect.svelte`'s `submit()` to confirm the blank guard.

- [ ] **Step 7: Run the gate.** From `rewrite/`, detached with a log:

```bash
nohup scripts/e2e.sh connect connect-restore emulators launch emulator-catalog firmware ps3-install cloud-saves updates > /tmp/claude-1000/-home-six-Documents-Programming-grid-launcher/d527a4be-8a2d-487c-bc02-e067fbdcf4ce/scratchpad/e2e-parity1.log 2>&1 &
```

then poll the log until the summary line appears. Why each group: `connect`/`connect-restore` exercise the new Connect field and the Settings edit path; `emulators` covers the form fields, the notes, the toast and the standalone launch; `launch` and `emulator-catalog` re-drive the Emulators view's rows and forms; `firmware` and `ps3-install` prove the untouched `emulator-ps3-firmware-*` ids still work under the new row markup; `cloud-saves` and `updates` cover the two Settings pages that changed.

- [ ] **Step 8:** All nine groups green. If one fails, read the failure, fix the cause within this plan's scope, re-run that group, and commit the fix with a `rewrite: ` subject.

- [ ] **Step 9: Commit the specs**

```bash
git add e2e/specs/emulators.spec.ts e2e/specs/connect.spec.ts
git commit -m "rewrite: cover the emulator form fields, notes, toast, standalone launch and connection edit in E2E"
```

- [ ] **Step 10:** Report the per-group result lines verbatim.

---

## Self-review notes

- **Spec coverage.** Gap 1 → Task 2. Gap 2 → Task 7. Gap 5 → Task 1 (surface) plus Tasks 2, 6, 7 (the messages routed through it). Gap 6 → Task 3. Gap 7 → Task 9. Gap 10 → Task 5. Gap 11 → Task 6. Gap 12 → Task 4 (text input only; the `Browse…` button is gap 4's plan). Gap 14 → Task 8. Gap 15 → Task 2 (`ARGS_LABEL`) and Task 4 (auto-sync hint); the `NativeSettings.svelte` strings are explicitly out of scope. Gaps 3, 4, 9, 13 and section B are untouched, and the four deferred controller-ruling items appear nowhere.
- **Type consistency.** Every type referenced is either defined in a task (`Toast`, `ToastLevel`, `EmulatorNote`, `EmulatorFormValues`, `SaveStrategy`, `RaLogin`, `RaLoginResult`, `RaLoginResult` (TS)) or cited in an existing file: `EmulatorEntry` (`app/src/lib/api.ts:137-151`, `crates/grid-core/src/config.rs:8-49`), `RaFanOutRow`/`RaStatus` (`commands.rs:1035-1049`, `api.ts:155-156`), `ProfileSummary` (`api.ts:153`), `SecretString` (secrecy), `RaTokenStore` (`crates/grid-core/src/secrets.rs:36-41`), `RaCredentials` (`crates/grid-core/src/autoconfig/mod.rs:47`), `SessionState`/`connect` (`crates/grid-core/src/session.rs:92`, `app/src/lib/stores/session.svelte.ts:19`).
- **Fixed during review.** (a) Task 7's first draft had `fetch_json` reject a `Success: false` payload before `ra_login_with_base` could apply the reference's `"Invalid credentials"` fallback — Step 3 now states the correction explicitly. (b) The same task's `store_ra_credentials` draft invented `expose_secret_len_is_blank`; Step 7 replaces it with an explicit `token_is_blank` parameter computed by the caller, matching `commands.rs:1078`. (c) Task 1's first draft re-routed `emulator-ps3-firmware-toast` and `details-update-toast` through the new store, which would have broken `firmware.spec.ts:175-181` and `updates.spec.ts:238` — ruling 4 now forbids it. (d) Task 3's notes originally used class `.hint`; they use `.note` so `emulators.spec.ts`'s `[data-testid^="emulator-row-"] .name` selector is unaffected, and the note text never lands in a `.name` element. (e) Task 2 originally adopted Python's `%rom%` default for a blank Arguments field, which contradicts `emulators.spec.ts`'s `toHaveValue('')` assertion and both auto-fill guards — ruling 5 now forbids it.
- **Placeholder scan.** No "similar to", "add appropriate", "TODO" or elided code block remains. Every string that must be verbatim is quoted in full at least once, with its reference line. The only value the plan does not pin exactly is `tauri-plugin-window-state`'s patch version, deliberately: Task 8 pins the same `"2"` major-version form the file already uses for its two other plugins, and records that the crate must be fetched from the network.
- **Open question.** `tauri-plugin-window-state` is not in the local cargo registry, so Task 8's Step 3 is the first step in this plan that requires network access. If the environment is offline, Task 8 must be skipped and re-run later; nothing else in the plan depends on it.
