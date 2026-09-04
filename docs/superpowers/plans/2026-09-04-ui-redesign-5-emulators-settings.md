# Desktop UI redesign 5 — Emulators and Settings views Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the one-column Emulators page into the redesign's category-rail view — Installed (with an inline edit sheet), Add from catalog, Platform defaults, Compat tools — and fill the four Settings pages plan 1 left as placeholders (Connection, Cloud saves, RetroAchievements, Updates) while extending Appearance, moving the RetroAchievements and Cloud saves forms out of Emulators in the same step.

**Architecture:** Every rule the two views need lives in pure modules that vitest covers: `emulators/pages.ts` (the four pages, rail entries with counts, where the manual form renders, which page a save lands on, the Ctrl+F target) and `settings/{pages,connection,updates,appearance}.ts` (the five pages, the credential line, the version / last-check / check-only copy, the background on/off rule). `Emulators.svelte` becomes a rail plus four always-mounted panes switched with `hidden` (the same rule the shell uses for views, so the catalog's terminal-signature refresh and the defaults' compatibility fetch keep running whichever pane is visible); the manual form is extracted once into `EmulatorForm.svelte` and rendered either as the Installed pane's edit sheet or as the catalog pane's Manual mode. `Settings.svelte` becomes a rail plus five page components. No new command or event: every command the pages need already exists (`app_version`, `app_update_notice`, `retry_connect`, `disconnect`, `cloud_settings`, `set_cloud_settings`, the `*_retroachievements_*` trio, `get_ui_settings` / `set_ui_settings`). The one backend change is a `checked_at` timestamp on the payload `app_update_notice` already returns, so Settings › Updates can tell "not checked" from "up to date".

**Tech Stack:** Rust (Tauri 2 `app` crate, `app_update.rs` only), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — binding. This plan implements **delivery item 5 only** (§12.5): §9 Emulators view, §10 Settings view, D-UI-5, the Emulators/Settings half of D-UI-7, the §3 Ctrl+F rule for the Emulators catalog search, and the §11 new ids `emu-nav-<page>` (plus the `settings-nav-<page>` and `theme-select` E2E cases). TV mode, core downloads, collections/search endpoints, the achievements tab, notes and controller navigation are out of scope (§13).

**What plan 1 already delivered, and this plan therefore does not repeat.** The §11 renames are done: `Shell.svelte` renders the pill `nav-emulators` (no `emulators-open` exists anywhere in `app/src` or `e2e/`), `Emulators.svelte`'s root is `data-testid="emulators-view"` (the id that replaced `emulators-panel`), and `emulators-close` is gone — every spec already navigates with `nav-server` / `nav-emulators`. `Settings.svelte` already has the five-entry rail with `settings-nav-<page>` ids, the `theme-select` control and the `background-fade` slider on Appearance, and the Updates page already hosts `app-update-notice` / `app-update-open` / `app-update-dismiss`; `Shell.svelte` already renders `app-update-badge` in the session cluster and routes it to `settings.show('updates')`; the banner strip is gone and `updates.spec.ts` already asserts the badge route and `theme-select`. `app.css` already defines `.view-content` (max-width 1100px, centred). What is left for this plan: the Emulators rail and panes, the edit sheet, the Emulators Ctrl+F, the four placeholder Settings pages (which still read `Coming in a later step`), the Appearance additions (card-size defaults, background art on/off), moving RetroAchievements and Cloud saves out of `Emulators.svelte`, the `emu-nav-*` / `settings-nav-*` E2E cases, and the docs.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Deliberate deviations, and why

Each of these is a decision the plan makes against, or beyond, the spec text. They are listed here so a reviewer can reject the decision rather than discover it buried in a task.

- **The four Emulators panes stay mounted and switch with `hidden`, not `{#if}`.** §9 says "one scrolling pane per category". The catalog pane re-reads itself whenever an emulator download reaches a terminal status, and the defaults pane re-fetches compatibility whenever the emulator list changes; both effects must keep running while the user is on another pane, or `emulator-catalog.spec.ts`'s "the PlayStation 2 default select never offered the freshly installed PCSX2" wait (which reads `default-select-1` without leaving the catalog) breaks. Mounting all four is also the rule §3 sets for the views themselves. Every spec still clicks the rail before it *interacts* with a pane — a hidden element cannot be clicked and `getText` on it is empty.
- **"Manual" is a tab, and its ids are the existing `emu-add-tab-install` / `emu-add-tab-manual`.** §9 says the Add from catalog pane has "a 'Manual' button [that] opens the manual form". `emulator-catalog.spec.ts` asserts `emu-add-tab-install` carries `aria-selected="true"` and that `emu-add-tab-manual` exists, and §11 keeps every `emu-*` id. So the pane has a two-tab toggle — **Catalog** (`emu-add-tab-install`) and **Manual** (`emu-add-tab-manual`) — where the Manual tab *is* the button §9 describes. Nothing is lost.
- **A successful manual add lands on the Installed pane.** §9 does not say where the user goes after saving from the catalog pane. Four specs (`emulators`, `firmware`, `launch`, `emulator-catalog`) save and then wait for `emulator-row-<name>`; switching to Installed on success is what a user expects ("here is what you added") and is what keeps those waits honest instead of passing against a hidden row. An edit-sheet save stays on Installed and closes the sheet.
- **`emulator-add` survives as a button on the Installed pane that opens the catalog pane.** §11 keeps `emulator-*`; three specs click it. It now selects the Add from catalog page with the Catalog tab active — one click, same as before.
- **Ctrl+F on the Emulators view switches to the catalog pane and focuses its search.** §3 says Ctrl+F "focuses the current view's search"; the Emulators view has exactly one search box and it lives on one pane. A silent no-op on the other three panes would make the accelerator look broken, so the chord goes to the search wherever the user is.
- **Settings › Connection shows credential *presence*, never a value or a mode.** §10 says "token status". The backend's `SessionState` carries no auth-mode flag and, by the token-secrecy rule, nothing that could carry the secret; once the shell is up a credential is by definition in the keyring (`restore_session` returned `connected` or `unreachable`, both of which require one). The line therefore reads `Stored in the OS keyring · session verified` / `… · not verified (server unreachable)`.
- **Settings › Updates reads a backend `checked_at`, and this is the plan's one Rust change (controller ruling, 2026-09-04).** The backend kept no check timestamp, and `fetch_notice_from` folded "checked, nothing newer" and "request failed" into one `None`, so the frontend could not tell "not checked" from "up to date" — and a frontend pull time is not a check time. Task 3 adds `checked_at: Option<String>` (RFC 3339 UTC) to the payload `app_update_notice` already returns, set only when a check completes (with or without a notice) and left `None` when the check was skipped (dev build) or failed. The page then says `Not checked yet`, `Up to date · checked <relative time>`, or the notice — never "Up to date" for a check that never ran. This goes against the "prefer no new IPC" preference by one optional field on an existing payload; no command or event is added.
- **Background art "on/off" is a checkbox over the existing fade value.** §10 lists "background art on/off, background fade slider". No separate config key exists and adding one would be a Rust change for a boolean the slider already expresses (fade 0 = off). Off writes `ui.background_fade = 0`; on restores the last non-zero value the store saw this session, or the 25% default.
- **Library path stays on the Server view.** `SPEC.md` (Python era) lists "library path" under Settings; §10 of the redesign spec does not, and `library-path-banner` on the Server view is E2E-covered by `install-a`. Moving it is out of this plan.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. The RetroAchievements token field stays write-only (empty on mount, never bound to a value read back; `RaStatus` carries only `token_present`), the Connection page renders presence only, and nothing this plan adds may log or render a credential.
- **No new command or event.** Every page uses commands that exist in `app/src/lib/api.ts` today. The one Rust change (Task 3) reshapes `app_update_notice`'s payload from `Option<AppUpdateNotice>` to `AppUpdateStatus { notice, checked_at }` — one optional field added, nothing new registered — and `cargo test --workspace` runs for that task.
- **View roots:** `emulators-view` (plan 1's id, replacing `emulators-panel`) and `settings-view` stay the roots. Each pane's content column takes `.view-content` (**max-width 1100px, centred** — D-UI-7); the 220px rail sits outside that column.
- **Emulators rail, in this order (D-UI-5, §9):** Installed, Add from catalog, Platform defaults, Compat tools; ids `emu-nav-installed` / `emu-nav-catalog` / `emu-nav-defaults` / `emu-nav-compat`, counts `emu-nav-count-<page>`, pane roots `emu-page-<page>`. **Compat tools is hidden on a Windows host** (`isWindowsHost(navigator.platform)`).
- **Settings rail, in this order (§10):** Connection, Cloud saves, RetroAchievements, Updates, Appearance; ids `settings-nav-connection` / `-cloud-saves` / `-retroachievements` / `-updates` / `-appearance` (unchanged from plan 1), pane roots `settings-page-<page>`.
- **Every existing id survives with its existing behaviour:** `emulator-add`, `emulator-row-<name>`, `emulator-edit-<name>`, `emulator-delete-<name>`, `emulator-ps3-firmware-note-<name>`, `emulator-ps3-firmware-<name>`, `emulator-ps3-firmware-toast`, `emu-add-tab-install`, `emu-add-tab-manual`, `emu-catalog-search`, `emu-catalog-install-<key>`, `emu-catalog-installed-<key>`, `emu-form-name` / `-path` / `-args` / `-error` / `-save` / `-cancel`, `emu-autofill-hint`, `default-select-<id>`, `default-core-<id>`, `compat-*`, `ra-status` / `-username` / `-token` / `-error` / `-save` / `-clear`, `cloud-settings-*`, `app-update-badge` / `-notice` / `-open` / `-dismiss`, `theme-select`, `background-fade`. **Every string a spec asserts stays verbatim:** `An emulator named '<name>' already exists.` (backend), `Confirm delete`, `PS3 firmware installation started — follow the RPCS3 dialog to complete.`, `No compatible emulator`, `Not set`, `Set for <username>`, `GRID Launcher <tag> is available`.
- **Only `app.css` tokens for colours** (`--primary`, `--danger`, `--surface`, `--surface-2`, `--border`, `--text-*`); motion only via the `--m-*` tokens; radii via `--r-*`. The literal `#e5484d` in `Emulators.svelte` and `CompatTools.svelte` becomes `var(--danger)`.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` and `npx vitest run` green. Then a commit whose subject starts `rewrite: `. **The final code task runs every E2E group (`scripts/e2e.sh` with no argument) and must be green.**
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** in this repo (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted, unit-tested pure module plus `npm run check` and E2E — never by a fabricated component test.
- **Specs are updated in the same task as the markup they click.** A spec that clicks inside a pane clicks that pane's `emu-nav-*` / `settings-nav-*` entry first.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/RailPane.svelte` | count badge becomes optional (Settings has no counts) |
| `app/src/lib/emulators/pages.ts` (+ test) | `EMULATOR_PAGES`, labels, `visibleEmulatorPages`, `emulatorRailEntries`, `formPlacement`, `pageAfterSave`, `SEARCH_PAGE`, `safeEmulatorPage` |
| `app/src/lib/emulators/EmulatorForm.svelte` | the manual add / edit form, extracted once; used as the edit sheet and the catalog pane's Manual tab |
| `app/src/lib/Emulators.svelte` | rail + four mounted panes; RetroAchievements and Cloud saves blocks removed |
| `app/src/lib/emulators/CompatTools.svelte` | `#e5484d` → `var(--danger)`; no behaviour change |
| `app/src/lib/settings/pages.ts` (+ test) | moved from `lib/settings.ts`; `SETTINGS_PAGES`, labels, `settingsRailEntries`; `LATER_STEP_TEXT` deleted |
| `app/src/lib/settings/connection.ts` (+ test) | `credentialStatusLabel`, `reconnectEnabled`, `serverLine` |
| `app/src-tauri/src/app_update.rs` | `CheckOutcome`, `AppUpdateStatus` with `checked_at`, `AppUpdateState::record` / `status`, the three-state tests |
| `app/src-tauri/src/commands/updates.rs` | `app_update_notice` returns `AppUpdateStatus` |
| `app/src/lib/api.ts` | `AppUpdateStatus` mirror |
| `app/src/lib/settings/updates.ts` (+ test) | `versionLine`, `relativeCheckTime`, `updateStatusLine`, `CHECK_ONLY_NOTE` |
| `app/src/lib/settings/appearance.ts` (+ test) | `backgroundEnabled`, `rememberFade`, `fadeForToggle`, `CARD_SIZE_VIEWS` |
| `app/src/lib/stores/appUpdate.svelte.ts` | `checkedAt` (from the payload), `stored` |
| `app/src/lib/stores/uiSettings.svelte.ts` (+ test) | `setBackgroundEnabled`, the remembered fade |
| `app/src/lib/settings/ConnectionPage.svelte` | server URL, user, credential line, Reconnect, Disconnect |
| `app/src/lib/settings/CloudSavesPage.svelte` | the cloud settings form (moved verbatim from `Emulators.svelte`) |
| `app/src/lib/settings/RetroAchievementsPage.svelte` | the RA form (moved verbatim from `Emulators.svelte`) |
| `app/src/lib/settings/UpdatesPage.svelte` | version, last check, notice + release link, check-only note |
| `app/src/lib/settings/AppearancePage.svelte` | theme, background on/off + fade, card-size defaults |
| `app/src/lib/Settings.svelte` | rail + five mounted pages |
| `app/src/lib/Shell.svelte` | `active` for Settings, `bind:this` for Emulators, the Server chip routes to Platform defaults, `view-content` moves into the pages |
| `e2e/specs/emulators.spec.ts`, `emulator-catalog.spec.ts`, `launch.spec.ts`, `firmware.spec.ts` | rail clicks before pane interaction; rail and sheet cases |
| `e2e/specs/cloud-saves.spec.ts` | the upload-delay edit moves to Settings › Cloud saves |
| `e2e/specs/updates.spec.ts` | Settings rail walk: Connection, Updates, Cloud saves, RetroAchievements |
| `SPEC.md`, `rewrite/README.md`, `docs/porting/04-emulator-launch.md`, `docs/porting/06-cloud-saves.md`, `docs/porting/10-identity-updates.md` | documentation |

---

### Task 1: The Emulators page rules, and an optional rail count

**Files:**
- Create: `app/src/lib/emulators/pages.ts`, `app/src/lib/emulators/pages.test.ts`
- Modify: `app/src/lib/RailPane.svelte:1-17` (the `RailPaneEntry` type), `:41-58` (the markup)

**Interfaces:**
- Consumes: nothing from the app (pure).
- Produces, used by Tasks 6 and 7:
  - `export const EMULATOR_PAGES = ['installed', 'catalog', 'defaults', 'compat'] as const`; `export type EmulatorPage = (typeof EMULATOR_PAGES)[number]`.
  - `emulatorPageLabel(page: EmulatorPage): string` — `Installed`, `Add from catalog`, `Platform defaults`, `Compat tools`.
  - `visibleEmulatorPages(windowsHost: boolean): EmulatorPage[]` — drops `compat` on Windows.
  - `safeEmulatorPage(page: EmulatorPage, windowsHost: boolean): EmulatorPage` — `compat` on Windows falls back to `installed`.
  - `export type EmulatorPageCounts = Record<EmulatorPage, number>`.
  - `export type EmulatorRailEntry = { key: EmulatorPage; testId: string; countTestId: string; label: string; count: number; selected: boolean; heading?: string }`.
  - `emulatorRailEntries(counts: EmulatorPageCounts, selected: EmulatorPage, windowsHost: boolean): EmulatorRailEntry[]` — the first entry carries `heading: 'EMULATORS'`.
  - `export type AddTab = 'install' | 'manual'`; `export type FormPlacement = 'sheet' | 'manual' | null`.
  - `formPlacement(page: EmulatorPage, editing: boolean, addTab: AddTab): FormPlacement`.
  - `pageAfterSave(mode: 'add' | 'edit'): EmulatorPage` — always `installed`.
  - `export const SEARCH_PAGE: EmulatorPage = 'catalog'`.
  - `RailPaneEntry.count` and `RailPaneEntry.countTestId` become optional; the badge renders only when `count` is a number.

- [ ] **Step 1: Write the failing tests**

Create `app/src/lib/emulators/pages.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  EMULATOR_PAGES,
  emulatorPageLabel,
  emulatorRailEntries,
  formPlacement,
  pageAfterSave,
  safeEmulatorPage,
  SEARCH_PAGE,
  visibleEmulatorPages,
  type EmulatorPageCounts,
} from './pages';

const counts: EmulatorPageCounts = { installed: 2, catalog: 7, defaults: 3, compat: 1 };

describe('emulator pages', () => {
  it('lists the four categories of design §9, in order', () => {
    expect([...EMULATOR_PAGES]).toEqual(['installed', 'catalog', 'defaults', 'compat']);
  });

  it('labels every page', () => {
    expect(emulatorPageLabel('installed')).toBe('Installed');
    expect(emulatorPageLabel('catalog')).toBe('Add from catalog');
    expect(emulatorPageLabel('defaults')).toBe('Platform defaults');
    expect(emulatorPageLabel('compat')).toBe('Compat tools');
  });

  it('hides Compat tools on a Windows host (design §9)', () => {
    expect(visibleEmulatorPages(false)).toEqual(['installed', 'catalog', 'defaults', 'compat']);
    expect(visibleEmulatorPages(true)).toEqual(['installed', 'catalog', 'defaults']);
  });

  it('falls back to Installed when a hidden page is asked for', () => {
    expect(safeEmulatorPage('compat', true)).toBe('installed');
    expect(safeEmulatorPage('compat', false)).toBe('compat');
    expect(safeEmulatorPage('defaults', true)).toBe('defaults');
  });
});

describe('emulatorRailEntries', () => {
  it('builds one entry per visible page with the §11 ids and the counts', () => {
    const entries = emulatorRailEntries(counts, 'defaults', false);
    expect(entries.map((e) => e.key)).toEqual(['installed', 'catalog', 'defaults', 'compat']);
    expect(entries.map((e) => e.testId)).toEqual([
      'emu-nav-installed',
      'emu-nav-catalog',
      'emu-nav-defaults',
      'emu-nav-compat',
    ]);
    expect(entries.map((e) => e.countTestId)).toEqual([
      'emu-nav-count-installed',
      'emu-nav-count-catalog',
      'emu-nav-count-defaults',
      'emu-nav-count-compat',
    ]);
    expect(entries.map((e) => e.count)).toEqual([2, 7, 3, 1]);
    expect(entries.map((e) => e.selected)).toEqual([false, false, true, false]);
  });

  it('puts the section heading on the first entry only', () => {
    const entries = emulatorRailEntries(counts, 'installed', false);
    expect(entries[0].heading).toBe('EMULATORS');
    expect(entries.slice(1).every((e) => e.heading === undefined)).toBe(true);
  });

  it('omits the compat entry on Windows', () => {
    expect(emulatorRailEntries(counts, 'installed', true).map((e) => e.key)).toEqual([
      'installed',
      'catalog',
      'defaults',
    ]);
  });
});

describe('formPlacement', () => {
  it('renders the edit sheet only on Installed while an entry is being edited', () => {
    expect(formPlacement('installed', true, 'install')).toBe('sheet');
    expect(formPlacement('installed', false, 'install')).toBeNull();
    expect(formPlacement('defaults', true, 'install')).toBeNull();
  });

  it('renders the manual add form only on the catalog page under the Manual tab', () => {
    expect(formPlacement('catalog', false, 'manual')).toBe('manual');
    expect(formPlacement('catalog', false, 'install')).toBeNull();
    expect(formPlacement('installed', false, 'manual')).toBeNull();
  });

  it('an open edit never leaks onto the catalog page', () => {
    expect(formPlacement('catalog', true, 'install')).toBeNull();
    expect(formPlacement('catalog', true, 'manual')).toBe('manual');
  });
});

describe('pageAfterSave / SEARCH_PAGE', () => {
  it('lands every successful save on Installed', () => {
    expect(pageAfterSave('add')).toBe('installed');
    expect(pageAfterSave('edit')).toBe('installed');
  });

  it('Ctrl+F targets the catalog page, the only one with a search box', () => {
    expect(SEARCH_PAGE).toBe('catalog');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run, from `rewrite/app`: `npx vitest run src/lib/emulators/pages.test.ts`
Expected: FAIL — `Failed to resolve import "./pages"`.

- [ ] **Step 3: Write `pages.ts`**

Create `app/src/lib/emulators/pages.ts`:

```ts
// The Emulators view's category rail (design §9, D-UI-5): four pages, the
// rail entries with their §11 ids and counts, where the manual form renders,
// and where a save lands. Pure — Emulators.svelte owns the state and the
// markup, this module owns the rules.

export const EMULATOR_PAGES = ['installed', 'catalog', 'defaults', 'compat'] as const;
export type EmulatorPage = (typeof EMULATOR_PAGES)[number];

const LABELS: Record<EmulatorPage, string> = {
  installed: 'Installed',
  catalog: 'Add from catalog',
  defaults: 'Platform defaults',
  compat: 'Compat tools',
};

export function emulatorPageLabel(page: EmulatorPage): string {
  return LABELS[page];
}

/** Design §9: "Compat tools (hidden on Windows)". */
export function visibleEmulatorPages(windowsHost: boolean): EmulatorPage[] {
  return EMULATOR_PAGES.filter((p) => p !== 'compat' || !windowsHost);
}

/** A page that is not on this host's rail falls back to the first one. */
export function safeEmulatorPage(page: EmulatorPage, windowsHost: boolean): EmulatorPage {
  return visibleEmulatorPages(windowsHost).includes(page) ? page : 'installed';
}

export type EmulatorPageCounts = Record<EmulatorPage, number>;

/** One rail row. The shape `RailPane.svelte` renders, minus its generics. */
export type EmulatorRailEntry = {
  key: EmulatorPage;
  testId: string;
  countTestId: string;
  label: string;
  count: number;
  selected: boolean;
  heading?: string;
};

export function emulatorRailEntries(
  counts: EmulatorPageCounts,
  selected: EmulatorPage,
  windowsHost: boolean,
): EmulatorRailEntry[] {
  return visibleEmulatorPages(windowsHost).map((page, i) => ({
    key: page,
    testId: `emu-nav-${page}`,
    countTestId: `emu-nav-count-${page}`,
    label: LABELS[page],
    count: counts[page],
    selected: page === selected,
    ...(i === 0 ? { heading: 'EMULATORS' } : {}),
  }));
}

/** The catalog pane's two tabs: the catalog rows, or the manual form. */
export type AddTab = 'install' | 'manual';

/**
 * Where the one `EmulatorForm` renders: as the Installed pane's edit sheet
 * (design §9: "Edit opens the manual form inline as a sheet on the right of
 * the pane"), as the catalog pane's Manual tab, or nowhere.
 */
export type FormPlacement = 'sheet' | 'manual' | null;

export function formPlacement(page: EmulatorPage, editing: boolean, addTab: AddTab): FormPlacement {
  if (page === 'installed' && editing) return 'sheet';
  if (page === 'catalog' && addTab === 'manual') return 'manual';
  return null;
}

/** A successful save shows the row it produced: both modes land on Installed. */
export function pageAfterSave(mode: 'add' | 'edit'): EmulatorPage {
  void mode;
  return 'installed';
}

/** Design §3: Ctrl+F focuses the view's search; only this page has one. */
export const SEARCH_PAGE: EmulatorPage = 'catalog';
```

- [ ] **Step 4: Run the tests to verify they pass**

Run, from `rewrite/app`: `npx vitest run src/lib/emulators/pages.test.ts`
Expected: PASS (12 tests).

- [ ] **Step 5: Make the rail count optional**

In `app/src/lib/RailPane.svelte`, replace the `RailPaneEntry` type (lines 4–17) with:

```ts
  export type RailPaneEntry<K extends string = string> = {
    key: K;
    /** `data-testid` for the row's button. */
    testId: string;
    /** `data-testid` for the count badge. Only read when `count` is set. */
    countTestId?: string;
    label: string;
    /** The count badge. Omit it (the Settings rail does) and no badge renders. */
    count?: number;
    selected: boolean;
    /** A section heading rendered above this row when set (e.g. "PLATFORMS"). */
    heading?: string;
    /** A `data-rail` attribute value for the row, when the view wants one. */
    dataRail?: string;
  };
```

and replace the count span inside the button (line 56) with:

```svelte
      {#if entry.count !== undefined}
        <span data-testid={entry.countTestId} class="rail-count">{entry.count}</span>
      {/if}
```

Update the header comment of the `<script lang="ts" generics>` block so it reads "the Library, Server, Emulators and Settings views share one rail" instead of naming only Library and Server. Library and Server pass both fields today, so their markup is unchanged.

- [ ] **Step 6: Check and commit**

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: both green.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/emulators/pages.ts rewrite/app/src/lib/emulators/pages.test.ts rewrite/app/src/lib/RailPane.svelte
git commit -m "rewrite: add the Emulators page rules and an optional rail count"
```

---
### Task 2: Extract the manual form into `EmulatorForm.svelte`

The Installed pane's edit sheet and the catalog pane's Manual tab render the same form. Extracting it now, against the *current* `Emulators.svelte`, keeps the change small and lets the existing E2E groups prove the extraction before Task 6 rearranges everything around it.

**Files:**
- Create: `app/src/lib/emulators/EmulatorForm.svelte`
- Modify: `app/src/lib/Emulators.svelte:14-18` (imports), `:62-68` (form state), `:353-373` (`openAdd` / `openEdit` / `closeForm`), `:385-449` (`autoFillFromPath`, `autoFillFromName`, `onPathKeydown`, `saveForm`), `:708-738` (the `<form>` block), the `.form-section form / label / input / .hint / .form-actions` styles

**Interfaces:**
- Consumes: `api.saveEmulator(originalName: string, entry: EmulatorEntry)`, `api.matchProfile(executablePath: string)`, `matchProfileByName`, `shouldAutoFillFromName` from `emulators/catalog.ts`.
- Produces, used by Task 6:
  - `EmulatorForm` props: `{ mode: 'add' | 'edit'; entry?: EmulatorEntry | null; profiles: ProfileSummary[]; onSaved: () => void; onCancel: () => void }`. The form initialises its fields from `entry` **once, on mount** — a parent that switches the edited entry wraps the component in `{#key entry.name}`.
  - Ids rendered, unchanged: `emu-form-name`, `emu-form-path`, `emu-form-args`, `emu-form-error`, `emu-form-save`, `emu-form-cancel`, `emu-autofill-hint`; the root `<form>` carries `data-testid="emu-form"`.

- [ ] **Step 1: Write `EmulatorForm.svelte`**

Create `app/src/lib/emulators/EmulatorForm.svelte`:

```svelte
<script lang="ts">
  // The manual add / edit form (design §9). One component, two hosts: the
  // Installed pane renders it as the edit sheet, the catalog pane as its
  // Manual tab. The fields seed from `entry` on mount only — a parent that
  // changes which entry is edited wraps this in `{#key entry.name}`.
  import { api, type EmulatorEntry, type ProfileSummary } from '../api';
  import { matchProfileByName, shouldAutoFillFromName } from './catalog';

  let {
    mode,
    entry = null,
    profiles,
    onSaved,
    onCancel,
  }: {
    mode: 'add' | 'edit';
    entry?: EmulatorEntry | null;
    profiles: ProfileSummary[];
    onSaved: () => void;
    onCancel: () => void;
  } = $props();

  let formName = $state(entry?.name ?? '');
  let formPath = $state(entry?.path ?? '');
  let formArgs = $state(entry?.args ?? '');
  let formError = $state<string | null>(null);
  let formPending = $state(false);
  let autofillMatch = $state<ProfileSummary | null>(null);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function autoFillFromPath() {
    if (formName.trim() !== '' || formArgs.trim() !== '') return;
    const path = formPath.trim();
    if (!path) return;
    try {
      const profile = await api.matchProfile(path);
      if (profile) {
        formName = profile.name;
        formArgs = profile.args;
      }
    } catch {
      // Best-effort autofill only — leave the form as typed on failure.
    }
  }

  // Manual-add auto-fill from the typed NAME (task-7-brief.md): add mode
  // only, and only when path and args are both still empty, so it never
  // clobbers a manually typed or path-derived value and never touches an
  // entry being edited. Fires on blur/input of the name field.
  function autoFillFromName() {
    if (!shouldAutoFillFromName(mode, formPath, formArgs)) {
      autofillMatch = null;
      return;
    }
    const match = matchProfileByName(formName, profiles);
    autofillMatch = match;
    if (match) {
      formArgs = match.args;
    }
  }

  function onPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      autoFillFromPath();
    }
  }

  async function save() {
    // `originalName` is what `save_emulator` uses to find-and-replace a
    // renamed entry; blank means "insert". The fields the form does not
    // show (install provenance, autoconfig paths) are spread back from
    // `entry` untouched instead of being dropped on save.
    const originalName = mode === 'add' ? '' : (entry?.name ?? '');
    const next: EmulatorEntry = {
      ...(mode === 'edit' && entry ? entry : {}),
      // Backend stores the name as-given; trim client-side so a name typed
      // with stray whitespace doesn't get persisted verbatim.
      name: formName.trim(),
      path: formPath,
      args: formArgs,
    };
    formError = null;
    formPending = true;
    try {
      await api.saveEmulator(originalName, next);
      onSaved();
    } catch (err) {
      formError = errorMessage(err);
    } finally {
      formPending = false;
    }
  }
</script>

<form
  data-testid="emu-form"
  onsubmit={(e) => {
    e.preventDefault();
    save();
  }}
>
  <label>
    Name
    <input
      data-testid="emu-form-name"
      bind:value={formName}
      onblur={autoFillFromName}
      oninput={autoFillFromName}
      required
    />
  </label>
  {#if autofillMatch}
    <p data-testid="emu-autofill-hint" class="hint">Matched profile: {autofillMatch.name}</p>
  {/if}
  <label>
    Executable path
    <input data-testid="emu-form-path" bind:value={formPath} onblur={autoFillFromPath} onkeydown={onPathKeydown} />
  </label>
  <label>Arguments <input data-testid="emu-form-args" bind:value={formArgs} /></label>
  {#if formError}<p data-testid="emu-form-error" class="error" role="alert">{formError}</p>{/if}
  <div class="form-actions">
    <button data-testid="emu-form-save" type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
    <button data-testid="emu-form-cancel" type="button" onclick={onCancel} disabled={formPending}>Cancel</button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .form-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .form-actions button[type='button'] {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
```

- [ ] **Step 2: Use it from the current `Emulators.svelte`**

In `app/src/lib/Emulators.svelte`:

1. Replace the `./emulators/catalog` import (lines 14–18) with:

```ts
  import { filterCatalogEntries } from './emulators/catalog';
  import EmulatorForm from './emulators/EmulatorForm.svelte';
```

2. Delete the six form-state declarations (`formName`, `formPath`, `formArgs`, `formError`, `formPending`, `autofillMatch`; lines 62–68). Keep `addTab`.

3. Replace `openAdd`, `openEdit` and `closeForm` (lines 353–373) with:

```ts
  function openAdd() {
    editing = { mode: 'add' };
    addTab = 'install';
    catalogError = null;
    catalogSearch = '';
    confirmingDelete = null;
  }

  function openEdit(entry: EmulatorEntry) {
    editing = { mode: 'edit', name: entry.name, entry };
    confirmingDelete = null;
  }

  function closeForm() {
    editing = null;
  }

  /** The form saved: close it, then re-read the list and the defaults it may have changed. */
  async function afterSave() {
    closeForm();
    await refreshEmulators();
    await refreshDefaults();
  }
```

4. Delete `autoFillFromPath`, `autoFillFromName`, `onPathKeydown` and `saveForm` (lines 385–449) — they live in the component now.

5. Replace the whole `<form … </form>` block (lines 709–737, the `{:else}` branch of the catalog/manual `{#if}`) with:

```svelte
          {#key editing.mode === 'edit' ? editing.name : 'add'}
            <EmulatorForm
              mode={editing.mode}
              entry={editing.mode === 'edit' ? editing.entry : null}
              {profiles}
              onSaved={afterSave}
              onCancel={closeForm}
            />
          {/key}
```

6. In the `<style>` block delete the rules `.form-section form`, `.form-section label`, `.form-section input`, `.form-section input:focus-visible`, `.hint`, `.form-actions`, `.form-actions button`, `.form-actions button[type='button']`, `.form-actions button:disabled`. Keep `.form-section` in the shared section rule. The PS3 firmware note still uses the `hint` class, so re-add a minimal rule for it in place of the deleted one:

```css
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
```

- [ ] **Step 3: Check, run the three groups that drive the form, and commit**

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green — `svelte-check` reports no unused imports or missing symbols.

Run, from `rewrite/`: `scripts/e2e.sh emulators launch firmware`
Expected: all three groups PASS (the autofill-on-blur case, the rename case, the duplicate-name error, the broken-path edit and the hand-added RPCS3 all go through the extracted form).

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/emulators/EmulatorForm.svelte rewrite/app/src/lib/Emulators.svelte
git commit -m "rewrite: extract the manual emulator form into EmulatorForm.svelte"
```

---

### Task 3: `checked_at` on the app-update payload — the plan's one Rust change

**Why this goes against "prefer no new IPC".** `app_update_notice` returns `Option<AppUpdateNotice>`, and `fetch_notice_from` answers `None` both for "checked, nothing newer" and "the request failed"; `spawn_check` also returns early on a dev build. The frontend therefore cannot tell "not checked" from "up to date", and a Settings page that read "Up to date" after a skipped check would be a false claim (controller ruling, 2026-09-04). The fix is one optional field on the payload the existing command already returns — `AppUpdateStatus { notice, checked_at }` — with `#[serde(default)]` so an older payload still decodes. No command, event, or state is added or registered; the `app-update-available` event is unchanged.

**Files:**
- Modify: `app/src-tauri/src/app_update.rs:27-52` (`AppUpdateNotice`, `AppUpdateState`), `:93-105` (`spawn_check`), `:107-176` (`fetch_notice`, `fetch_notice_from`), the `tests` module (`the_notice_store_starts_empty_and_round_trips`, the four `fetch_notice_from` tests)
- Modify: `app/src-tauri/src/commands/updates.rs:12` (import), `:78-85` (`app_update_notice`)
- Modify: `app/src/lib/api.ts:330` (`AppUpdateNotice`), `:432` (`appUpdateNotice`)
- Modify: `app/src/lib/stores/appUpdate.svelte.ts:35-38` (the pull reads the new shape; Task 4 rewrites the file)

**Interfaces:**
- Consumes: `grid_core::launch::forge::ForgeClient`; `chrono` (already a dependency of the `app` crate).
- Produces, used by Tasks 4 and 5:
  - Rust: `pub enum CheckOutcome { UpToDate, Newer(AppUpdateNotice), Failed }`; `pub struct AppUpdateStatus { pub notice: Option<AppUpdateNotice>, #[serde(default)] pub checked_at: Option<String> }` (`Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq`); `AppUpdateState::record(&self, outcome: &CheckOutcome, now: chrono::DateTime<chrono::Utc>)`; `AppUpdateState::status(&self) -> AppUpdateStatus`; `AppUpdateState::get(&self) -> Option<AppUpdateNotice>` kept; `fetch_notice_from` becomes `fetch_outcome_from(client, url, current) -> CheckOutcome`. `checked_at` is RFC 3339 UTC to the second (`2023-11-14T22:13:20Z`), set for `UpToDate` and `Newer`, untouched for `Failed`, never set when `should_check` is false.
  - Command: `app_update_notice(state) -> AppUpdateStatus` (same name, same registration).
  - TS: `export type AppUpdateStatus = { notice: AppUpdateNotice | null; checked_at: string | null }`; `api.appUpdateNotice: () => Promise<AppUpdateStatus>`.

- [ ] **Step 1: Write the failing Rust tests**

In `app/src-tauri/src/app_update.rs`, inside `mod tests`, replace `the_notice_store_starts_empty_and_round_trips` with these five tests, and add the helper:

```rust
    fn at(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(secs, 0).unwrap()
    }

    fn notice() -> AppUpdateNotice {
        AppUpdateNotice {
            tag: "v9.9.9".to_string(),
            url: "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9".to_string(),
        }
    }

    /// State 1: skipped (a dev build never calls `record`) or failed — no
    /// stamp, so the frontend cannot claim "up to date".
    #[test]
    fn a_skipped_or_failed_check_leaves_no_timestamp() {
        let store = AppUpdateState::new();
        assert_eq!(store.status(), AppUpdateStatus::default());
        store.record(&CheckOutcome::Failed, at(1_700_000_000));
        assert_eq!(
            store.status(),
            AppUpdateStatus { notice: None, checked_at: None }
        );
        assert_eq!(store.get(), None);
    }

    /// State 2: the check completed and found nothing newer.
    #[test]
    fn an_up_to_date_check_stamps_the_time_without_a_notice() {
        let store = AppUpdateState::new();
        store.record(&CheckOutcome::UpToDate, at(1_700_000_000));
        assert_eq!(
            store.status(),
            AppUpdateStatus {
                notice: None,
                checked_at: Some("2023-11-14T22:13:20Z".to_string()),
            }
        );
    }

    /// State 3: the check completed and found a newer release.
    #[test]
    fn a_newer_release_stamps_the_time_and_stores_the_notice() {
        let store = AppUpdateState::new();
        store.record(&CheckOutcome::Newer(notice()), at(1_700_000_000));
        assert_eq!(
            store.status(),
            AppUpdateStatus {
                notice: Some(notice()),
                checked_at: Some("2023-11-14T22:13:20Z".to_string()),
            }
        );
        assert_eq!(store.get(), Some(notice()));
    }

    /// A failure after a completed check keeps the completed result: the
    /// last true statement stands, and nothing is un-checked.
    #[test]
    fn a_failure_never_erases_a_completed_check() {
        let store = AppUpdateState::new();
        store.record(&CheckOutcome::Newer(notice()), at(1_700_000_000));
        store.record(&CheckOutcome::Failed, at(1_700_000_060));
        assert_eq!(store.status().checked_at, Some("2023-11-14T22:13:20Z".to_string()));
        assert_eq!(store.get(), Some(notice()));
    }

    /// `#[serde(default)]`: a payload written before `checked_at` existed
    /// still decodes, as "not checked".
    #[test]
    fn a_payload_without_checked_at_still_decodes() {
        let status: AppUpdateStatus = serde_json::from_str(r#"{"notice":null}"#).unwrap();
        assert_eq!(status, AppUpdateStatus { notice: None, checked_at: None });
        let status: AppUpdateStatus =
            serde_json::from_str(r#"{"notice":{"tag":"v9.9.9","url":"https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9"}}"#)
                .unwrap();
        assert_eq!(status.notice, Some(notice()));
        assert_eq!(status.checked_at, None);
    }
```

Then change the four `fetch_notice_from` tests to the three-way outcome — rename each call to `fetch_outcome_from` and replace its assertion:

```rust
        // newer_release_becomes_a_notice
        assert_eq!(
            fetch_outcome_from(&client, &url, "0.9.0").await,
            CheckOutcome::Newer(AppUpdateNotice {
                tag: "v9.9.9".to_string(),
                url: "https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9".to_string(),
            })
        );
        // a_failed_request_is_no_notice
        assert_eq!(fetch_outcome_from(&client, &url, "0.9.0").await, CheckOutcome::Failed);
        // an_oversized_body_is_no_notice
        assert_eq!(fetch_outcome_from(&client, &url, "0.9.0").await, CheckOutcome::Failed);
        // an_older_release_is_no_notice
        assert_eq!(fetch_outcome_from(&client, &url, "0.9.0").await, CheckOutcome::UpToDate);
```

- [ ] **Step 2: Run them to verify they fail**

Run, from `rewrite/`: `cargo test -p app app_update`
Expected: compile error — `AppUpdateStatus`, `CheckOutcome`, `record`, `status`, `fetch_outcome_from` do not exist.

- [ ] **Step 3: Write the Rust change**

In `app/src-tauri/src/app_update.rs` replace the block from `#[derive(Debug, Clone, Serialize, PartialEq, Eq)]\npub struct AppUpdateNotice` through the end of `impl AppUpdateState` with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppUpdateNotice {
    pub tag: String,
    pub url: String,
}

/// What one run of the check concluded. `Failed` covers every silent
/// `None` of old — transport, status, cap, decode, missing fields — and is
/// the one outcome that must NOT count as a completed check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate,
    Newer(AppUpdateNotice),
    Failed,
}

/// The payload `commands::updates::app_update_notice` returns. `checked_at`
/// is RFC 3339 UTC, set when a check COMPLETED (with or without a notice)
/// and `None` when it was skipped (dev build, `should_check`) or failed —
/// so Settings › Updates can say "Not checked yet" instead of a false
/// "Up to date". `#[serde(default)]` keeps a payload written before this
/// field existed decodable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppUpdateStatus {
    pub notice: Option<AppUpdateNotice>,
    #[serde(default)]
    pub checked_at: Option<String>,
}

/// The startup check's result, held so a webview that mounts after the
/// emit can still pull it (`commands::updates::app_update_notice`). Tauri
/// buffers nothing for a window with no listener, and the check never
/// repeats, so without this the badge is simply lost when the forge answers
/// faster than the frontend boots.
#[derive(Default)]
pub struct AppUpdateState(Mutex<AppUpdateStatus>);

fn stamp(now: chrono::DateTime<chrono::Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl AppUpdateState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Records one finished run. A `Failed` run changes nothing: the last
    /// completed result (or the initial "not checked") stands.
    pub fn record(&self, outcome: &CheckOutcome, now: chrono::DateTime<chrono::Utc>) {
        let mut status = self.0.lock().expect("app update status mutex");
        match outcome {
            CheckOutcome::Failed => {}
            CheckOutcome::UpToDate => {
                status.notice = None;
                status.checked_at = Some(stamp(now));
            }
            CheckOutcome::Newer(notice) => {
                status.notice = Some(notice.clone());
                status.checked_at = Some(stamp(now));
            }
        }
    }

    pub fn status(&self) -> AppUpdateStatus {
        self.0.lock().expect("app update status mutex").clone()
    }

    pub fn get(&self) -> Option<AppUpdateNotice> {
        self.status().notice
    }
}
```

Replace `spawn_check`, `fetch_notice` and `fetch_notice_from` with:

```rust
/// Runs the check once, on Tauri's async runtime. Call from `setup`.
/// Records the outcome in `store` BEFORE emitting, so a frontend that
/// misses the event can pull the same value (and its `checked_at`)
/// afterwards. A dev build returns before spawning: nothing is recorded,
/// and the status stays "not checked".
pub fn spawn_check(app: AppHandle, store: Arc<AppUpdateState>) {
    let current = app.package_info().version.to_string();
    if !should_check(&current, e2e_forced()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let outcome = fetch_outcome(&current).await;
        store.record(&outcome, chrono::Utc::now());
        if let CheckOutcome::Newer(notice) = outcome {
            let _ = app.emit(APP_UPDATE_EVENT, notice);
        }
    });
}

/// The production check: builds the forge client and asks GitHub.
async fn fetch_outcome(current: &str) -> CheckOutcome {
    match ForgeClient::new() {
        Ok(client) => fetch_outcome_from(&client, LATEST_RELEASE_URL, current).await,
        Err(_) => CheckOutcome::Failed,
    }
}

/// One `releases/latest` request against `url`, decoded and compared against
/// `current`. The endpoint is a parameter so tests can point it at a local
/// mock server; production always passes [`LATEST_RELEASE_URL`].
///
/// `Newer` only for a release that carries both a tag and a page URL and
/// whose tag is newer than `current`; `UpToDate` for a well-formed release
/// that is not newer; every failure — transport, non-2xx status, a body
/// over [`MAX_RELEASE_BODY`], undecodable body, missing fields — is a
/// silent `Failed` logged at debug level, naming [`LATEST_RELEASE_HOST`]
/// and never the request URL.
async fn fetch_outcome_from(client: &ForgeClient, url: &str, current: &str) -> CheckOutcome {
    let mut response = match client.get(url, true).await {
        Ok(response) => response,
        Err(_) => {
            tracing::debug!("self-update check: request to {LATEST_RELEASE_HOST} failed");
            return CheckOutcome::Failed;
        }
    };
    // Chunk by chunk rather than `bytes()`: the cap has to stop the READ, not
    // just the parse, or an endless body is buffered in full before anyone
    // objects. Dropping `response` here closes the connection.
    let mut body: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len() + chunk.len() > MAX_RELEASE_BODY {
                    tracing::debug!(
                        "self-update check: release body from {LATEST_RELEASE_HOST} over the cap"
                    );
                    return CheckOutcome::Failed;
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => {
                tracing::debug!(
                    "self-update check: response from {LATEST_RELEASE_HOST} did not read"
                );
                return CheckOutcome::Failed;
            }
        }
    }
    let release: LatestRelease = match serde_json::from_slice(&body) {
        Ok(release) => release,
        Err(_) => {
            tracing::debug!("self-update check: release JSON did not decode");
            return CheckOutcome::Failed;
        }
    };
    if release.tag_name.is_empty() || release.html_url.is_empty() {
        tracing::debug!("self-update check: release JSON is missing its tag or URL");
        return CheckOutcome::Failed;
    }
    if !is_newer(current, &release.tag_name) {
        return CheckOutcome::UpToDate;
    }
    CheckOutcome::Newer(AppUpdateNotice {
        tag: release.tag_name,
        url: release.html_url,
    })
}
```

In `app/src-tauri/src/commands/updates.rs` change line 12 to `use crate::app_update::AppUpdateStatus;` and replace `app_update_notice` (with its doc comment) with:

```rust
/// The startup check's result: the notice, if any, and when the check
/// completed (`checked_at`, RFC 3339 UTC). `checked_at` is `None` when the
/// check was skipped (dev build) or failed, so Settings › Updates never
/// claims "up to date" for a check that never ran. The frontend's event
/// listener can register after the check has already emitted, so it pulls
/// this once on mount as well as listening.
#[tauri::command]
pub fn app_update_notice(state: State<'_, AppState>) -> AppUpdateStatus {
    state.app_update.status()
}
```

- [ ] **Step 4: Run the Rust tests to verify they pass**

Run, from `rewrite/`: `cargo test -p app app_update`
Expected: PASS — the five state tests, the four outcome tests, and the untouched `is_newer` / `should_check` tests.

- [ ] **Step 5: Mirror the payload in TypeScript**

In `app/src/lib/api.ts` replace line 330 with:

```ts
export type AppUpdateNotice = { tag: string; url: string };
/** `app_update_notice`'s payload: the notice, if any, and when the startup
 *  check completed (RFC 3339 UTC) — `null` when it was skipped or failed. */
export type AppUpdateStatus = { notice: AppUpdateNotice | null; checked_at: string | null };
```

and line 432 with:

```ts
  appUpdateNotice: () => invoke<AppUpdateStatus>('app_update_notice'),
```

In `app/src/lib/stores/appUpdate.svelte.ts` (Task 4 rewrites this file; this keeps `npm run check` green now) replace the two lines inside `initAppUpdate`'s `try`:

```ts
    const status = await api.appUpdateNotice();
    if (status.notice !== null && state.notice === null) state.notice = status.notice;
```

- [ ] **Step 6: Gate and commit**

Run, from `rewrite/`: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy -p app --all-targets --features e2e -- -D warnings && cargo test --workspace`
Expected: clean and green.

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green.

Run, from `rewrite/`: `scripts/e2e.sh updates`
Expected: PASS — the badge still appears from the mock forge's `v9.9.9-e2e` and routes to Settings › Updates.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src-tauri/src/app_update.rs rewrite/app/src-tauri/src/commands/updates.rs rewrite/app/src/lib/api.ts rewrite/app/src/lib/stores/appUpdate.svelte.ts
git commit -m "rewrite: stamp checked_at on the app-update status so Settings can tell not-checked from up-to-date"
```

---

### Task 4: The Settings page rules and the two store extensions

**Files:**
- Move: `app/src/lib/settings.ts` → `app/src/lib/settings/pages.ts`; `app/src/lib/settings.test.ts` → `app/src/lib/settings/pages.test.ts`
- Modify: `app/src/lib/Settings.svelte:10` (the import path only)
- Create: `app/src/lib/settings/connection.ts`, `app/src/lib/settings/connection.test.ts`
- Create: `app/src/lib/settings/updates.ts`, `app/src/lib/settings/updates.test.ts`
- Create: `app/src/lib/settings/appearance.ts`, `app/src/lib/settings/appearance.test.ts`
- Modify: `app/src/lib/stores/appUpdate.svelte.ts` (whole file, 41 lines)
- Modify: `app/src/lib/stores/uiSettings.svelte.ts:5-17` (imports), `:586-596` (`previewBackgroundFade` / `commitBackgroundFade`), append `setBackgroundEnabled`
- Modify: `app/src/lib/stores/uiSettings.test.ts` (append one `describe`)

**Interfaces:**
- Consumes: `AppUpdateNotice`, `AppUpdateStatus` from `api.ts` (Task 3); `FADE_DEFAULT`, `FADE_MAX` from `theme.ts`; `CardSize` from `cards/size.ts`.
- Produces, used by Task 5:
  - `settings/pages.ts`: `SETTINGS_PAGES`, `SettingsPage`, `settingsPageLabel` (unchanged from `lib/settings.ts`), plus `export type SettingsRailEntry = { key: SettingsPage; testId: string; label: string; selected: boolean; heading?: string }` and `settingsRailEntries(selected: SettingsPage): SettingsRailEntry[]` (first entry `heading: 'SETTINGS'`). `LATER_STEP_TEXT` stays until Task 5 deletes its last reader.
  - `settings/connection.ts`: `export const CREDENTIAL_STORED = 'Stored in the OS keyring'`; `credentialStatusLabel(connected: boolean): string`; `reconnectEnabled(connected: boolean, busy: boolean): boolean`; `serverLine(serverUrl: string): string` (`Not set` for blank).
  - `settings/updates.ts`: `versionLine(version: string): string`; `relativeCheckTime(checkedAt: string, nowMs: number): string` (`''` for an unparseable stamp); `updateStatusLine(notice: AppUpdateNotice | null, checkedAt: string | null, nowMs: number): string`; `export const CHECK_ONLY_NOTE: string`.
  - `settings/appearance.ts`: `backgroundEnabled(fade: number): boolean`; `rememberFade(fade: number, remembered: number): number`; `fadeForToggle(enabled: boolean, remembered: number): number`; `export const CARD_SIZE_VIEWS: readonly { view: 'library' | 'server'; label: string; testId: string }[]`.
  - `stores/appUpdate.svelte.ts`: `appUpdate.notice` (badge: null once dismissed — unchanged), **new** `appUpdate.stored: AppUpdateNotice | null` (the notice regardless of dismissal), **new** `appUpdate.checkedAt: string | null` (the backend's RFC 3339 `checked_at`, Task 3).
  - `stores/uiSettings.svelte.ts`: **new** `setBackgroundEnabled(enabled: boolean): Promise<void>`.

- [ ] **Step 1: Move the rail module and its test**

```bash
cd /home/six/Documents/Programming/grid-launcher/rewrite/app
mkdir -p src/lib/settings
git mv src/lib/settings.ts src/lib/settings/pages.ts
git mv src/lib/settings.test.ts src/lib/settings/pages.test.ts
```

In `src/lib/settings/pages.test.ts` change the import to `from './pages'`. In `src/lib/Settings.svelte` line 10 change `from './settings'` to `from './settings/pages'`. Replace the header comment of `pages.ts` (lines 1–2) with:

```ts
// The Settings rail (design §10): five pages, their labels, and the rail
// entries Settings.svelte hands to RailPane. Pure.
```

- [ ] **Step 2: Write the failing tests for the four modules**

Append to `src/lib/settings/pages.test.ts`:

```ts
describe('settingsRailEntries', () => {
  it('builds one entry per page with the §11 ids, the heading on the first', () => {
    const entries = settingsRailEntries('updates');
    expect(entries.map((e) => e.testId)).toEqual([
      'settings-nav-connection',
      'settings-nav-cloud-saves',
      'settings-nav-retroachievements',
      'settings-nav-updates',
      'settings-nav-appearance',
    ]);
    expect(entries.map((e) => e.selected)).toEqual([false, false, false, true, false]);
    expect(entries[0].heading).toBe('SETTINGS');
    expect(entries.slice(1).every((e) => e.heading === undefined)).toBe(true);
  });
});
```

(The file already imports `describe`, `expect`, `it`; add `settingsRailEntries` to the existing import from `./pages` rather than a second import statement.)

Create `src/lib/settings/connection.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { CREDENTIAL_STORED, credentialStatusLabel, reconnectEnabled, serverLine } from './connection';

describe('credentialStatusLabel', () => {
  it('reports presence only, never a value (token secrecy)', () => {
    expect(credentialStatusLabel(true)).toBe(`${CREDENTIAL_STORED} · session verified`);
    expect(credentialStatusLabel(false)).toBe(`${CREDENTIAL_STORED} · not verified (server unreachable)`);
  });
});

describe('reconnectEnabled', () => {
  it('offers Reconnect only while offline and idle', () => {
    expect(reconnectEnabled(false, false)).toBe(true);
    expect(reconnectEnabled(false, true)).toBe(false);
    expect(reconnectEnabled(true, false)).toBe(false);
  });
});

describe('serverLine', () => {
  it('shows the stored URL, or Not set', () => {
    expect(serverLine('https://romm.example:8080/base')).toBe('https://romm.example:8080/base');
    expect(serverLine('')).toBe('Not set');
    expect(serverLine('   ')).toBe('Not set');
  });
});
```

Create `src/lib/settings/updates.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { CHECK_ONLY_NOTE, relativeCheckTime, updateStatusLine, versionLine } from './updates';

const MINUTE = 60_000;
// 2023-11-14T22:13:20Z, the stamp the Rust tests use too.
const CHECKED_AT = '2023-11-14T22:13:20Z';
const CHECKED_MS = Date.parse(CHECKED_AT);
const NOTICE = { tag: 'v9.9.9-e2e', url: 'https://github.com/Sixdd6/grid-launcher/releases/tag/v9.9.9-e2e' };

describe('versionLine', () => {
  it('names the running build', () => {
    expect(versionLine('0.9.0')).toBe('GRID Launcher 0.9.0');
    expect(versionLine('0.9.0-dev')).toBe('GRID Launcher 0.9.0-dev');
  });
  it('says so when the version has not loaded', () => {
    expect(versionLine('')).toBe('GRID Launcher (version unknown)');
  });
});

describe('relativeCheckTime', () => {
  it('rounds to minutes, then hours', () => {
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 20_000)).toBe('just now');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + MINUTE)).toBe('1 min ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 5 * MINUTE)).toBe('5 min ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 60 * MINUTE)).toBe('1 h ago');
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS + 150 * MINUTE)).toBe('2 h ago');
  });
  it('never goes negative when the clock moves', () => {
    expect(relativeCheckTime(CHECKED_AT, CHECKED_MS - MINUTE)).toBe('just now');
  });
  it('is empty for a stamp it cannot parse', () => {
    expect(relativeCheckTime('yesterday', CHECKED_MS)).toBe('');
  });
});

describe('updateStatusLine (the three backend states, Task 3)', () => {
  it('never claims up to date when no check completed', () => {
    expect(updateStatusLine(null, null, CHECKED_MS)).toBe('Not checked yet');
  });
  it('reports up to date with the relative check time', () => {
    expect(updateStatusLine(null, CHECKED_AT, CHECKED_MS + 5 * MINUTE)).toBe('Up to date · checked 5 min ago');
  });
  it('drops the time when the stamp is unparseable', () => {
    expect(updateStatusLine(null, 'garbage', CHECKED_MS)).toBe('Up to date');
  });
  it('names the release when a notice is stored, verbatim to the badge title', () => {
    expect(updateStatusLine(NOTICE, CHECKED_AT, CHECKED_MS)).toBe('GRID Launcher v9.9.9-e2e is available');
  });
});

describe('CHECK_ONLY_NOTE', () => {
  it('states the check-only rule (doc 10 D-10-h)', () => {
    expect(CHECK_ONLY_NOTE).toBe(
      'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.',
    );
  });
});
```

Create `src/lib/settings/appearance.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { FADE_DEFAULT } from '../theme';
import { backgroundEnabled, CARD_SIZE_VIEWS, fadeForToggle, rememberFade } from './appearance';

describe('backgroundEnabled', () => {
  it('is off exactly at fade 0', () => {
    expect(backgroundEnabled(0)).toBe(false);
    expect(backgroundEnabled(1)).toBe(true);
    expect(backgroundEnabled(60)).toBe(true);
  });
});

describe('rememberFade', () => {
  it('keeps the last non-zero value and ignores zero', () => {
    expect(rememberFade(40, 25)).toBe(40);
    expect(rememberFade(0, 40)).toBe(40);
  });
});

describe('fadeForToggle', () => {
  it('off writes 0; on restores the remembered value', () => {
    expect(fadeForToggle(false, 40)).toBe(0);
    expect(fadeForToggle(true, 40)).toBe(40);
  });
  it('on with nothing remembered uses the design default', () => {
    expect(fadeForToggle(true, 0)).toBe(FADE_DEFAULT);
  });
});

describe('CARD_SIZE_VIEWS', () => {
  it('lists the two grids with their ids', () => {
    expect(CARD_SIZE_VIEWS.map((v) => v.view)).toEqual(['library', 'server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.testId)).toEqual(['card-size-library', 'card-size-server']);
    expect(CARD_SIZE_VIEWS.map((v) => v.label)).toEqual(['Library cards', 'Server cards']);
  });
});
```

- [ ] **Step 3: Run them to verify they fail**

Run, from `rewrite/app`: `npx vitest run src/lib/settings`
Expected: FAIL — `settingsRailEntries` is not exported; `./connection`, `./updates`, `./appearance` cannot be resolved.

- [ ] **Step 4: Write the four modules**

Append to `src/lib/settings/pages.ts`:

```ts
/** One rail row, the shape `RailPane.svelte` renders (no count: Settings has none). */
export type SettingsRailEntry = {
  key: SettingsPage;
  testId: string;
  label: string;
  selected: boolean;
  heading?: string;
};

export function settingsRailEntries(selected: SettingsPage): SettingsRailEntry[] {
  return SETTINGS_PAGES.map((page, i) => ({
    key: page,
    testId: `settings-nav-${page}`,
    label: LABELS[page],
    selected: page === selected,
    ...(i === 0 ? { heading: 'SETTINGS' } : {}),
  }));
}
```

Create `src/lib/settings/connection.ts`:

```ts
// Settings › Connection (design §10): the server URL, the credential's
// presence, and when Reconnect applies. Pure. Nothing here can hold or
// format a secret: the session store never has one, and the label below
// states presence only.

export const CREDENTIAL_STORED = 'Stored in the OS keyring';

/**
 * Once the shell is up a credential is in the keyring by construction —
 * `restore_session` answers `connected` or `unreachable` only when one is
 * stored. The two states differ in whether the server has accepted it.
 */
export function credentialStatusLabel(connected: boolean): string {
  return connected
    ? `${CREDENTIAL_STORED} · session verified`
    : `${CREDENTIAL_STORED} · not verified (server unreachable)`;
}

/** Mirrors the server menu: Reconnect exists only while offline, and not mid-retry. */
export function reconnectEnabled(connected: boolean, busy: boolean): boolean {
  return !connected && !busy;
}

export function serverLine(serverUrl: string): string {
  const trimmed = serverUrl.trim();
  return trimmed === '' ? 'Not set' : trimmed;
}
```

Create `src/lib/settings/updates.ts`:

```ts
// Settings › Updates (design §10): "app version, last check, release link,
// 'check-only' note". Pure; UpdatesPage.svelte reads the stores. The three
// states come from the backend's `checked_at` (Task 3): absent means the
// check was skipped or failed, and the page must not claim "up to date".
import type { AppUpdateNotice } from '../api';

export function versionLine(version: string): string {
  const v = version.trim();
  return v === '' ? 'GRID Launcher (version unknown)' : `GRID Launcher ${v}`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/**
 * How long ago the backend stamped `checked_at` (RFC 3339 UTC). Coarse on
 * purpose: the page is not a log. Empty for a stamp `Date.parse` rejects,
 * so the caller can drop the clause instead of printing "NaN min ago".
 */
export function relativeCheckTime(checkedAt: string, nowMs: number): string {
  const at = Date.parse(checkedAt);
  if (Number.isNaN(at)) return '';
  const elapsed = Math.max(0, nowMs - at);
  if (elapsed < MINUTE) return 'just now';
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)} min ago`;
  return `${Math.floor(elapsed / HOUR)} h ago`;
}

/** The notice sentence is the badge's tooltip verbatim, so the two never disagree. */
export function updateStatusLine(
  notice: AppUpdateNotice | null,
  checkedAt: string | null,
  nowMs: number,
): string {
  if (notice !== null) return `GRID Launcher ${notice.tag} is available`;
  if (checkedAt === null) return 'Not checked yet';
  const relative = relativeCheckTime(checkedAt, nowMs);
  return relative === '' ? 'Up to date' : `Up to date · checked ${relative}`;
}

/** Doc 10 D-10-h: the launcher only ever checks. */
export const CHECK_ONLY_NOTE =
  'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.';
```

Create `src/lib/settings/appearance.ts`:

```ts
// Settings › Appearance additions (design §10): "background art on/off" as
// a rule over the existing fade value (no separate config key: fade 0 IS
// off), and the two card-size defaults. Pure; the store persists.
import { FADE_DEFAULT } from '../theme';

export function backgroundEnabled(fade: number): boolean {
  return fade > 0;
}

/** The value "on" goes back to: the last non-zero fade seen this session. */
export function rememberFade(fade: number, remembered: number): number {
  return fade > 0 ? fade : remembered;
}

export function fadeForToggle(enabled: boolean, remembered: number): number {
  if (!enabled) return 0;
  return remembered > 0 ? remembered : FADE_DEFAULT;
}

/** D-UI-9: "Size control Small / Medium / Large per view, remembered". */
export const CARD_SIZE_VIEWS = [
  { view: 'library', label: 'Library cards', testId: 'card-size-library' },
  { view: 'server', label: 'Server cards', testId: 'card-size-server' },
] as const satisfies readonly { view: 'library' | 'server'; label: string; testId: string }[];
```

- [ ] **Step 5: Run the module tests to verify they pass**

Run, from `rewrite/app`: `npx vitest run src/lib/settings`
Expected: PASS.

- [ ] **Step 6: Write the failing store test**

Append to `src/lib/stores/uiSettings.test.ts` (inside the file, after the existing `describe` blocks; it reuses the file's `fakeStorage` / `fakeMedia` helpers and the `beforeEach` / `afterEach` hooks):

```ts
describe('setBackgroundEnabled', () => {
  async function loadStore(fade: number) {
    vi.stubGlobal('localStorage', fakeStorage());
    vi.stubGlobal('document', { documentElement: { dataset: {} } });
    vi.stubGlobal('window', { matchMedia: () => fakeMedia(false) });
    const setUiSettings = vi.fn().mockResolvedValue(undefined);
    vi.doMock('../api', () => ({
      api: {
        getUiSettings: () =>
          Promise.resolve({
            theme: 'system',
            background_fade: fade,
            card_size_library: 'medium',
            card_size_server: 'medium',
          }),
        setUiSettings,
      },
    }));
    const store = await import('./uiSettings.svelte');
    await store.initUiSettings();
    return { store, setUiSettings };
  }

  it('off writes fade 0, and on restores the value the slider last held', async () => {
    const { store, setUiSettings } = await loadStore(40);
    await store.setBackgroundEnabled(false);
    expect(store.uiSettings.backgroundFade).toBe(0);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 0 }));

    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(40);
    expect(setUiSettings).toHaveBeenLastCalledWith(expect.objectContaining({ background_fade: 40 }));
  });

  it('on with a config that was already 0 uses the design default', async () => {
    const { store } = await loadStore(0);
    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(25);
  });

  it('a slider drag updates what "on" restores', async () => {
    const { store } = await loadStore(25);
    store.previewBackgroundFade(55);
    await store.setBackgroundEnabled(false);
    await store.setBackgroundEnabled(true);
    expect(store.uiSettings.backgroundFade).toBe(55);
  });
});
```

Run, from `rewrite/app`: `npx vitest run src/lib/stores/uiSettings.test.ts`
Expected: FAIL — `store.setBackgroundEnabled is not a function`.

- [ ] **Step 7: Extend the two stores**

In `src/lib/stores/uiSettings.svelte.ts` add to the imports (after the `../cards/size` line):

```ts
import { fadeForToggle, rememberFade } from '../settings/appearance';
```

After the `state` declaration add:

```ts
// What "background art on" restores (design §10): the last non-zero fade
// this session saw — loaded from config or dragged on the slider. Module
// scoped like `state`, not persisted: fade 0 in config means off, and the
// default is what a fresh "on" goes back to.
let rememberedFade = FADE_DEFAULT;
```

In `initUiSettings`, directly after `state.backgroundFade = clampFade(stored.background_fade);` add:

```ts
    rememberedFade = rememberFade(state.backgroundFade, rememberedFade);
```

Replace `previewBackgroundFade` with:

```ts
/** Slider drag: updates the live preview without touching the config. */
export function previewBackgroundFade(value: number): void {
  state.backgroundFade = clampFade(value);
  rememberedFade = rememberFade(state.backgroundFade, rememberedFade);
}
```

Append at the end of the file:

```ts
/**
 * The Appearance page's on/off checkbox (design §10). Off persists fade 0;
 * on persists the remembered value, so toggling off and on returns the art
 * exactly as it was.
 */
export async function setBackgroundEnabled(enabled: boolean): Promise<void> {
  await commitBackgroundFade(fadeForToggle(enabled, rememberedFade));
}
```

Replace the whole of `src/lib/stores/appUpdate.svelte.ts` with:

```ts
// The self-update notice's state (design §3: a badge on the user menu plus
// an entry under Settings › Updates). Module-scoped so a dismissal survives
// Shell remounts for the rest of the process.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, APP_UPDATE_EVENT, type AppUpdateNotice, type AppUpdateStatus } from '../api';

const state = $state<{ notice: AppUpdateNotice | null; dismissed: boolean; checkedAt: string | null }>({
  notice: null,
  dismissed: false,
  checkedAt: null,
});

export const appUpdate = {
  /** What the badge shows: nothing once dismissed. */
  get notice() {
    return state.dismissed ? null : state.notice;
  },
  /** What Settings › Updates shows: the stored notice, dismissed or not. */
  get stored() {
    return state.notice;
  },
  /** The backend's `checked_at` (RFC 3339 UTC): null while no check has completed. */
  get checkedAt() {
    return state.checkedAt;
  },
};

export function dismiss(): void {
  state.dismissed = true;
}

function applyStatus(status: AppUpdateStatus): void {
  // An event that arrived first is newer than the pull and keeps its notice;
  // `checked_at` comes only from the backend, never from a local clock.
  if (status.notice !== null && state.notice === null) state.notice = status.notice;
  state.checkedAt = status.checked_at;
}

export async function initAppUpdate(): Promise<UnlistenFn> {
  // Listener FIRST, then pull: the startup check runs from Tauri's `setup`
  // and can emit before the webview mounts, and Tauri buffers nothing for a
  // window with no listener. `app_update_notice` holds whatever the check
  // already found, so the badge survives that race. An event that arrived
  // in between is newer and wins.
  const unlisten = await listen<AppUpdateNotice>(APP_UPDATE_EVENT, (e) => {
    state.notice = e.payload;
    // The backend stamps `checked_at` before it emits, so one more pull
    // picks the stamp up for the Updates page.
    api.appUpdateNotice().then(applyStatus).catch(() => {});
  });
  try {
    applyStatus(await api.appUpdateNotice());
  } catch {
    // No notice is the normal outcome; a failed pull is never surfaced and
    // leaves `checkedAt` null, so the Updates page says "Not checked yet".
  }
  return unlisten;
}
```

- [ ] **Step 8: Run everything and commit**

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green. `Settings.svelte` still compiles against `./settings/pages` and still reads `LATER_STEP_TEXT`.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/settings rewrite/app/src/lib/Settings.svelte rewrite/app/src/lib/stores/appUpdate.svelte.ts rewrite/app/src/lib/stores/uiSettings.svelte.ts rewrite/app/src/lib/stores/uiSettings.test.ts
git commit -m "rewrite: add the Settings page rules and the update/background store extensions"
```

---
### Task 5: The Settings view — five pages, with RetroAchievements and Cloud saves moved out of Emulators

**Files:**
- Create: `app/src/lib/settings/ConnectionPage.svelte`, `app/src/lib/settings/CloudSavesPage.svelte`, `app/src/lib/settings/RetroAchievementsPage.svelte`, `app/src/lib/settings/UpdatesPage.svelte`, `app/src/lib/settings/AppearancePage.svelte`
- Modify: `app/src/lib/Settings.svelte` (whole file), `app/src/lib/settings/pages.ts` (delete `LATER_STEP_TEXT`), `app/src/lib/settings/pages.test.ts` (delete its `LATER_STEP_TEXT` case and import)
- Modify: `app/src/lib/Shell.svelte:192-194` (the Settings mount)
- Modify: `app/src/lib/Emulators.svelte` — remove the RetroAchievements and Cloud saves blocks (imports, state, the two effect calls, five functions, two `<section>`s, their styles)
- Modify: `e2e/specs/cloud-saves.spec.ts:134-150`, `e2e/specs/updates.spec.ts` (append one case)

**Interfaces:**
- Consumes: `settingsRailEntries`, `SETTINGS_PAGES`, `settingsPageLabel`, `SettingsPage` (Task 4); `credentialStatusLabel`, `reconnectEnabled`, `serverLine` (Task 4); `versionLine`, `updateStatusLine`, `CHECK_ONLY_NOTE` (Task 4); `backgroundEnabled`, `CARD_SIZE_VIEWS` (Task 4); `appUpdate.stored`, `appUpdate.checkedAt`, `appUpdate.notice`, `dismiss` (Task 4); `setBackgroundEnabled`, `setCardSize`, `setTheme`, `previewBackgroundFade`, `commitBackgroundFade`, `uiSettings` (store); `session`, `retry`, `disconnect` (`stores/session.svelte.ts`); `RailPaneEntry` with optional count (Task 1); `api.appVersion`, `api.openReleasePage`, `api.cloudSettings`, `api.setCloudSettings`, `api.getRetroachievementsStatus`, `api.setRetroachievementsCredentials`, `api.clearRetroachievementsCredentials`; `canSubmit`, `fanOutSummary`, `statusLabel` from `emulators/retroachievements.ts`.
- Produces, used by Tasks 7 and 8:
  - `Settings` props `{ active?: boolean }`; `show(next: SettingsPage)` unchanged.
  - Ids: `settings-rail`, `settings-page-<page>`, `settings-connection-url` / `-user` / `-credential` / `-status` / `-error` / `-reconnect` / `-disconnect`, `settings-updates-version` / `-status` / `-note`, `background-art-toggle`, `card-size-library`, `card-size-server`; plus every surviving id listed in Global Constraints.
  - The Settings view's default page is **Connection** (§10 rail order); the badge still routes to `updates`.

- [ ] **Step 1: Write the five page components**

Create `app/src/lib/settings/ConnectionPage.svelte`:

```svelte
<script lang="ts">
  // Settings › Connection (design §10): "server URL, token status, reconnect,
  // disconnect". Reads the session store; the two actions are the same
  // functions the server menu calls. Nothing here can render a secret —
  // the store never holds one.
  import { disconnect, retry, session } from '../stores/session.svelte';
  import { credentialStatusLabel, reconnectEnabled, serverLine } from './connection';
</script>

<dl class="rows">
  <div class="row">
    <dt>Server</dt>
    <dd data-testid="settings-connection-url">{serverLine(session.serverUrl)}</dd>
  </div>
  <div class="row">
    <dt>User</dt>
    <dd data-testid="settings-connection-user">{session.username.trim() === '' ? 'Not set' : session.username}</dd>
  </div>
  <div class="row">
    <dt>Credential</dt>
    <dd data-testid="settings-connection-credential">{credentialStatusLabel(session.connected)}</dd>
  </div>
  <div class="row">
    <dt>Status</dt>
    <dd data-testid="settings-connection-status">
      <span class="dot" class:online={session.connected} aria-hidden="true"></span>
      {session.connected ? 'Connected' : 'Not connected'}
    </dd>
  </div>
</dl>

{#if !session.connected && session.lastError}
  <p data-testid="settings-connection-error" class="error" role="alert">{session.lastError}</p>
{/if}

<div class="actions">
  <button
    data-testid="settings-connection-reconnect"
    disabled={!reconnectEnabled(session.connected, session.busy)}
    onclick={() => {
      retry();
    }}
  >
    {session.busy ? 'Reconnecting…' : 'Reconnect'}
  </button>
  <button
    data-testid="settings-connection-disconnect"
    class="secondary"
    onclick={() => {
      disconnect();
    }}
  >
    Disconnect
  </button>
</div>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
  }

  .row {
    display: flex;
    align-items: baseline;
    gap: 12px;
    font-size: 13px;
  }

  dt {
    flex: 0 0 180px;
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    min-width: 0;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--danger);
    vertical-align: middle;
  }

  .dot.online {
    background: var(--success);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .actions button.secondary {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
```

Create `app/src/lib/settings/CloudSavesPage.svelte` — the form is the one `Emulators.svelte` renders today, moved verbatim (ids, labels, min/max, the `Saved.` line):

```svelte
<script lang="ts">
  // Settings › Cloud saves (design §10: "current cloud settings form"), moved
  // out of Emulators.svelte (task-19-brief.md). The refresh is gated on the
  // Settings view being visible, as the Emulators view gated it before.
  import { api, type CloudSettings } from '../api';

  let { active = true }: { active?: boolean } = $props();

  let cloudSettings = $state<CloudSettings | null>(null);
  let cloudSettingsError = $state<string | null>(null);
  let cloudSettingsSavedLine = $state<string | null>(null);
  let cloudSettingsPending = $state(false);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshCloudSettings() {
    try {
      cloudSettings = await api.cloudSettings();
      cloudSettingsError = null;
    } catch (err) {
      cloudSettingsError = errorMessage(err);
    }
  }

  async function handleCloudSettingsSave() {
    if (!cloudSettings) return;
    cloudSettingsError = null;
    cloudSettingsSavedLine = null;
    cloudSettingsPending = true;
    try {
      await api.setCloudSettings(cloudSettings);
      cloudSettingsSavedLine = 'Saved.';
      await refreshCloudSettings();
    } catch (err) {
      cloudSettingsError = errorMessage(err);
    } finally {
      cloudSettingsPending = false;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshCloudSettings();
  });
</script>

{#if cloudSettings}
  <form
    onsubmit={(e) => {
      e.preventDefault();
      handleCloudSettingsSave();
    }}
  >
    <label class="checkbox">
      <input
        data-testid="cloud-settings-download-on-launch"
        type="checkbox"
        bind:checked={cloudSettings.download_on_launch}
      />
      Restore cloud saves before launch
    </label>
    <label class="checkbox">
      <input
        data-testid="cloud-settings-upload-on-exit"
        type="checkbox"
        bind:checked={cloudSettings.upload_on_exit}
      />
      Upload cloud saves after exit
    </label>
    <label class="checkbox">
      <input
        data-testid="cloud-settings-skip-if-local-newer"
        type="checkbox"
        bind:checked={cloudSettings.skip_if_local_newer}
      />
      Skip download when the local save is newer
    </label>
    <label>
      Upload delay (seconds)
      <input
        data-testid="cloud-settings-upload-delay"
        type="number"
        min="0"
        max="60"
        bind:value={cloudSettings.upload_delay_seconds}
      />
    </label>
    <label>
      Save retention limit
      <input
        data-testid="cloud-settings-retention-limit"
        type="number"
        min="1"
        bind:value={cloudSettings.retention_limit}
      />
    </label>
    {#if cloudSettingsError}<p data-testid="cloud-settings-error" class="error" role="alert">{cloudSettingsError}</p>{/if}
    {#if cloudSettingsSavedLine}<p class="hint">{cloudSettingsSavedLine}</p>{/if}
    <div class="form-actions">
      <button data-testid="cloud-settings-save" type="submit" disabled={cloudSettingsPending}>
        {cloudSettingsPending ? 'Saving…' : 'Save'}
      </button>
    </div>
  </form>
{:else if cloudSettingsError}
  <p data-testid="cloud-settings-error" class="error" role="alert">{cloudSettingsError}</p>
{:else}
  <p class="muted">Loading…</p>
{/if}

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  label.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input[type='checkbox'] {
    width: auto;
    padding: 0;
    accent-color: var(--primary);
  }

  input[type='number'] {
    width: 100px;
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .form-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
```

Create `app/src/lib/settings/RetroAchievementsPage.svelte` — moved verbatim from `Emulators.svelte`; the token field stays write-only:

```svelte
<script lang="ts">
  // Settings › RetroAchievements (design §10: "current form"), moved out of
  // Emulators.svelte (task-12-brief.md).
  import { api, type RaFanOutRow, type RaStatus } from '../api';
  import { canSubmit, fanOutSummary, statusLabel } from '../emulators/retroachievements';

  let { active = true }: { active?: boolean } = $props();

  let raStatus = $state<RaStatus | null>(null);
  let raUsername = $state('');
  // The token field is write-only: it starts empty on every mount and is
  // never bound to a value read back from the backend, which never returns
  // the token in the first place (RaStatus carries only `token_present`).
  let raToken = $state('');
  let raError = $state<string | null>(null);
  let raResultLine = $state<string | null>(null);
  let raSavePending = $state(false);
  let raClearPending = $state(false);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshRaStatus() {
    try {
      raStatus = await api.getRetroachievementsStatus();
      raUsername = raStatus.username;
    } catch (err) {
      raError = errorMessage(err);
    }
  }

  async function handleRaSave() {
    if (!canSubmit(raUsername, raToken)) return;
    raError = null;
    raResultLine = null;
    raSavePending = true;
    try {
      const rows: RaFanOutRow[] = await api.setRetroachievementsCredentials(raUsername, raToken);
      raToken = '';
      raResultLine = fanOutSummary(rows);
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raSavePending = false;
    }
  }

  async function handleRaClear() {
    raError = null;
    raResultLine = null;
    raClearPending = true;
    try {
      await api.clearRetroachievementsCredentials();
      raToken = '';
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raClearPending = false;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshRaStatus();
  });
</script>

<p class="muted" data-testid="ra-status">{statusLabel(raStatus)}</p>
<form
  onsubmit={(e) => {
    e.preventDefault();
    handleRaSave();
  }}
>
  <label>
    Username
    <input data-testid="ra-username" bind:value={raUsername} autocomplete="username" />
  </label>
  <label>
    Token
    <input
      data-testid="ra-token"
      type="password"
      bind:value={raToken}
      autocomplete="new-password"
    />
  </label>
  {#if raError}<p data-testid="ra-error" class="error" role="alert">{raError}</p>{/if}
  {#if raResultLine}<p class="hint">{raResultLine}</p>{/if}
  <div class="form-actions">
    <button
      data-testid="ra-save"
      type="submit"
      disabled={raSavePending || !canSubmit(raUsername, raToken)}
    >
      {raSavePending ? 'Saving…' : 'Save'}
    </button>
    <button
      data-testid="ra-clear"
      type="button"
      onclick={handleRaClear}
      disabled={raClearPending}
    >
      {raClearPending ? 'Clearing…' : 'Clear'}
    </button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .form-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .form-actions button[type='button'] {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
```

Create `app/src/lib/settings/UpdatesPage.svelte`:

```svelte
<script lang="ts">
  // Settings › Updates (design §10): "app version, last check, release link,
  // 'check-only' note". The status line has three states, driven by the
  // backend's `checked_at`: not checked, up to date (with the relative check
  // time), or the notice. The notice line and its two buttons keep the ids
  // plan 1 gave them; Dismiss hides the badge, not this entry.
  import { api } from '../api';
  import { appUpdate, dismiss } from '../stores/appUpdate.svelte';
  import { CHECK_ONLY_NOTE, updateStatusLine, versionLine } from './updates';

  let { active = true }: { active?: boolean } = $props();

  let version = $state('');
  // Re-read whenever the view comes forward so "5 min ago" is honest at the
  // moment the user looks, without a ticking timer.
  let now = $state(Date.now());

  $effect(() => {
    if (!active) return;
    now = Date.now();
    api
      .appVersion()
      .then((v) => {
        version = v;
      })
      .catch(() => {
        // `versionLine('')` already says the version is unknown.
      });
  });

  function openRelease() {
    const stored = appUpdate.stored;
    if (stored === null) return;
    api.openReleasePage(stored.url).catch(() => {
      // The opener refuses anything outside the repo's releases prefix.
    });
  }
</script>

<p data-testid="settings-updates-version" class="line">{versionLine(version)}</p>

{#if appUpdate.stored}
  <p data-testid="app-update-notice" class="update-line">
    {updateStatusLine(appUpdate.stored, appUpdate.checkedAt, now)}
    <button data-testid="app-update-open" onclick={openRelease}>Open release</button>
    {#if appUpdate.notice}
      <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
    {/if}
  </p>
{:else}
  <p data-testid="settings-updates-status" class="line">{updateStatusLine(null, appUpdate.checkedAt, now)}</p>
{/if}

<p data-testid="settings-updates-note" class="muted">{CHECK_ONLY_NOTE}</p>

<style>
  .line {
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .muted {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    max-width: 60ch;
  }

  .update-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
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

Create `app/src/lib/settings/AppearancePage.svelte` — plan 1's theme select and fade slider, plus the on/off checkbox and the two card-size defaults:

```svelte
<script lang="ts">
  // Settings › Appearance (design §10): "theme, card size defaults,
  // background art on/off, background fade slider 0–60% with a live preview
  // behind the settings pane". The art reads the same store, so `oninput`
  // previews and `onchange` persists.
  import {
    commitBackgroundFade,
    previewBackgroundFade,
    setBackgroundEnabled,
    setCardSize,
    setTheme,
    uiSettings,
  } from '../stores/uiSettings.svelte';
  import { FADE_MAX, type ThemeChoice } from '../theme';
  import { CARD_SIZES, cardSizeLabel, normalizeCardSize } from '../cards/size';
  import { backgroundEnabled, CARD_SIZE_VIEWS } from './appearance';

  function onThemeChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as ThemeChoice;
    setTheme(value).catch(() => {
      // The attribute is already applied; a failed save is not worth a
      // blocking error in a settings pane.
    });
  }

  function onToggle(e: Event) {
    setBackgroundEnabled((e.currentTarget as HTMLInputElement).checked).catch(() => {});
  }

  function onCardSize(view: 'library' | 'server', e: Event) {
    setCardSize(view, normalizeCardSize((e.currentTarget as HTMLSelectElement).value)).catch(() => {});
  }

  function sizeFor(view: 'library' | 'server') {
    return view === 'library' ? uiSettings.cardSizeLibrary : uiSettings.cardSizeServer;
  }
</script>

<div class="field">
  <label for="theme-select">Theme</label>
  <select data-testid="theme-select" id="theme-select" value={uiSettings.theme} onchange={onThemeChange}>
    <option value="system">Follow system</option>
    <option value="dark">Dark</option>
    <option value="light">Light</option>
  </select>
</div>

<div class="field">
  <label for="background-art-toggle">Background art</label>
  <input
    data-testid="background-art-toggle"
    id="background-art-toggle"
    type="checkbox"
    checked={backgroundEnabled(uiSettings.backgroundFade)}
    onchange={onToggle}
  />
</div>

<div class="field">
  <label for="background-fade">Background art fade</label>
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

{#each CARD_SIZE_VIEWS as v (v.view)}
  <div class="field">
    <label for={v.testId}>{v.label}</label>
    <select data-testid={v.testId} id={v.testId} value={sizeFor(v.view)} onchange={(e) => onCardSize(v.view, e)}>
      {#each CARD_SIZES as size (size)}
        <option value={size}>{cardSizeLabel(size)}</option>
      {/each}
    </select>
  </div>
{/each}

<style>
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

  .field select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .field input[type='range'] {
    flex: 1 1 auto;
    max-width: 320px;
    accent-color: var(--primary);
  }

  .field input[type='checkbox'] {
    accent-color: var(--primary);
  }

  .value {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
```

- [ ] **Step 2: Rewrite `Settings.svelte` around the rail and the pages**

Replace the whole of `app/src/lib/Settings.svelte` with:

```svelte
<script lang="ts">
  // The Settings view (design §10, D-UI-5): a 220px category rail and one
  // pane per page. All five panes stay mounted and switch with `hidden`,
  // the same rule the shell applies to views, so an in-flight save or a
  // typed-but-unsaved field survives a rail click. Each pane's column caps
  // at 1100px (D-UI-7).
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import {
    SETTINGS_PAGES,
    settingsPageLabel,
    settingsRailEntries,
    type SettingsPage,
  } from './settings/pages';
  import ConnectionPage from './settings/ConnectionPage.svelte';
  import CloudSavesPage from './settings/CloudSavesPage.svelte';
  import RetroAchievementsPage from './settings/RetroAchievementsPage.svelte';
  import UpdatesPage from './settings/UpdatesPage.svelte';
  import AppearancePage from './settings/AppearancePage.svelte';

  let { active = true }: { active?: boolean } = $props();

  let page = $state<SettingsPage>('connection');

  /**
   * Programmatic page selection, for callers that route straight to a page
   * — the top bar's update badge opens Settings on `updates` (design §3).
   */
  export function show(next: SettingsPage) {
    page = next;
  }

  let railRows = $derived(
    settingsRailEntries(page).map(
      (e): RailPaneEntry<SettingsPage> => ({
        key: e.key,
        testId: e.testId,
        label: e.label,
        selected: e.selected,
        heading: e.heading,
      }),
    ),
  );
</script>

<section class="settings" aria-label="Settings">
  <RailPane entries={railRows} testId="settings-rail" ariaLabel="Settings pages" onSelect={(k) => (page = k)} />

  <div class="panes">
    {#each SETTINGS_PAGES as p (p)}
      <section data-testid={`settings-page-${p}`} class="pane" hidden={page !== p} aria-label={settingsPageLabel(p)}>
        <div class="view-content pane-inner">
          <h2>{settingsPageLabel(p)}</h2>
          {#if p === 'connection'}
            <ConnectionPage />
          {:else if p === 'cloud-saves'}
            <CloudSavesPage {active} />
          {:else if p === 'retroachievements'}
            <RetroAchievementsPage {active} />
          {:else if p === 'updates'}
            <UpdatesPage {active} />
          {:else}
            <AppearancePage />
          {/if}
        </div>
      </section>
    {/each}
  </div>
</section>

<style>
  .settings {
    display: flex;
    align-items: stretch;
    height: 100%;
    min-height: 0;
  }

  .panes {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  /* No `display` on `.pane` itself: the `hidden` attribute's UA rule must
     win, and an author `display: flex` here would override it. */
  .pane {
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .pane[hidden] {
    display: none;
  }

  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-h);
  }
</style>
```

In `app/src/lib/settings/pages.ts` delete the `LATER_STEP_TEXT` export and its comment; in `pages.test.ts` delete the `holds the placeholder copy verbatim` case and drop `LATER_STEP_TEXT` from the import.

- [ ] **Step 3: Mount it from the shell**

In `app/src/lib/Shell.svelte` replace lines 192–194 with:

```svelte
<div data-testid="settings-view" class="view" hidden={view !== 'settings'}>
  <Settings active={view === 'settings'} bind:this={settings} />
</div>
```

(`view-content` moves inside each pane; the wrapper no longer caps the rail.)

- [ ] **Step 4: Remove the two blocks from `Emulators.svelte`**

In `app/src/lib/Emulators.svelte` (as left by Task 2):

1. In the `./api` import drop the types `CloudSettings`, `RaFanOutRow`, `RaStatus`. Delete the line `import { canSubmit, fanOutSummary, statusLabel } from './emulators/retroachievements';`.
2. Delete the state block that starts with the comment `// RetroAchievements block state (task-12-brief.md).` through `let raClearPending = $state(false);`, and the block that starts `// Cloud saves settings block (task-19-brief.md).` through `let cloudSettingsPending = $state(false);`.
3. In the `$effect` gated on `active`, delete the two calls `refreshRaStatus();` and `refreshCloudSettings();`.
4. Delete the functions `refreshRaStatus`, `handleRaSave`, `handleRaClear`, `refreshCloudSettings`, `handleCloudSettingsSave`.
5. Delete the markup from `<section class="ra-section">` through its `</section>` and from `<section class="cloud-settings-section">` through its `</section>` (the last child before the closing `</section>` of the view).
6. In `<style>`, remove `.ra-section` and `.cloud-settings-section` from the shared section rule's selector list, and delete every rule whose selector mentions `.ra-section` or `.cloud-settings-section`.

- [ ] **Step 5: Update the cloud-saves spec and add the Settings rail case**

In `e2e/specs/cloud-saves.spec.ts` replace lines 134–150 (from `await $(testId('nav-emulators')).click();` through the `reverse: true` wait on `emulators-view`) with:

```ts
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the settings view never rendered',
    });
    await $(testId('settings-nav-cloud-saves')).click();
    await $(testId('settings-page-cloud-saves')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Settings › Cloud saves never rendered',
    });
    const delayInput = $(testId('cloud-settings-upload-delay'));
    await delayInput.waitForExist({ timeout: TRANSITION_TIMEOUT });
    await delayInput.clearValue();
    await delayInput.setValue('0');
    await $(testId('cloud-settings-save')).click();
    await waitForConfigLine('auto_cloud_save_upload_delay_seconds = 0');
    await $(testId('nav-server')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the settings view never went away',
    });
```

The comment above that block (`Auto-download-on-launch … down to 0 so scenario 3 doesn't need a real wait.`) stays as it is; nothing in it names the Emulators view.

In `e2e/specs/updates.spec.ts` append, after the `applies and persists the Appearance theme choice` case (still inside the `describe`):

```ts
  // Design §10: the four pages plan 1 left as placeholders. One pass over
  // the rail, reading one line each page owns; the pure rules live in
  // `settings/*.test.ts`.
  it('walks the Settings rail: Connection, Updates, Cloud saves, RetroAchievements', async () => {
    await $(testId('settings-nav-connection')).click();
    await $(testId('settings-page-connection')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('settings-connection-url'))).toHaveText(mockUrl());
    await expect($(testId('settings-connection-credential'))).toHaveText(
      'Stored in the OS keyring · session verified',
    );
    // Connected: Reconnect is not on offer, Disconnect is.
    await expect($(testId('settings-connection-reconnect'))).toBeDisabled();
    await expect($(testId('settings-connection-disconnect'))).toBeEnabled();

    await $(testId('settings-nav-updates')).click();
    await $(testId('settings-page-updates')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('settings-updates-version')).getText()).toMatch(/^GRID Launcher \d/);
    // The notice dismissed earlier is still listed here — Dismiss hid the
    // badge, not the entry (design §3).
    expect(await $(testId('app-update-notice')).getText()).toContain(SELF_UPDATE_TAG);
    expect(await $(testId('app-update-dismiss')).isExisting()).toBe(false);
    await expect($(testId('settings-updates-note'))).toHaveText(
      'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.',
    );

    await $(testId('settings-nav-cloud-saves')).click();
    await $(testId('settings-page-cloud-saves')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('cloud-settings-save')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });

    await $(testId('settings-nav-retroachievements')).click();
    await $(testId('settings-page-retroachievements')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('ra-status'))).toHaveText('Not set');
    await expect($(testId('ra-save'))).toBeDisabled();

    await $(testId('settings-nav-appearance')).click();
    await $(testId('settings-page-appearance')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('background-art-toggle'))).toBeSelected();
    await expect($(testId('card-size-library'))).toHaveValue('medium');
  });
```

- [ ] **Step 6: Check, run the affected groups, and commit**

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green (no reference to `LATER_STEP_TEXT`, `raStatus` or `cloudSettings` remains in `Emulators.svelte`; `svelte-check` reports no unused import).

Run, from `rewrite/`: `scripts/e2e.sh cloud-saves updates emulators`
Expected: PASS ×3. `cloud-saves` proves the moved form writes `auto_cloud_save_upload_delay_seconds = 0`; `updates` proves the badge route, the theme select and the rail walk; `emulators` proves the Emulators view still works without the two removed blocks.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/settings rewrite/app/src/lib/Settings.svelte rewrite/app/src/lib/Shell.svelte rewrite/app/src/lib/Emulators.svelte rewrite/e2e/specs/cloud-saves.spec.ts rewrite/e2e/specs/updates.spec.ts
git commit -m "rewrite: build the Settings pages and move RetroAchievements and Cloud saves out of Emulators"
```

---
### Task 6: The Emulators view — rail, four panes, the edit sheet, the catalog tabs

**Files:**
- Modify: `app/src/lib/Emulators.svelte` (whole file — the version below replaces everything Tasks 2 and 5 left)
- Modify: `app/src/lib/emulators/CompatTools.svelte` (the `.error` rule: `#e5484d` → `var(--danger)`)
- Modify: `e2e/specs/emulators.spec.ts`, `e2e/specs/emulator-catalog.spec.ts`, `e2e/specs/launch.spec.ts`, `e2e/specs/firmware.spec.ts`

**Interfaces:**
- Consumes: `EMULATOR_PAGES`, `EmulatorPage`, `emulatorPageLabel`, `emulatorRailEntries`, `safeEmulatorPage`, `formPlacement`, `pageAfterSave`, `AddTab` (Task 1); `EmulatorForm` (Task 2); `RailPaneEntry` with counts (Task 1); `compatTools` store (for the Compat tools count); everything the current file already imports from `api.ts`, `emulators/catalog.ts`, `emulators/defaults.ts`, `emulators/compatTools.ts`, `stores/downloads.svelte.ts`.
- Produces, used by Task 7:
  - `Emulators` props `{ active?: boolean }`; **new** `show(next: EmulatorPage): void`.
  - Ids: root `emulators-view` (kept), `emulators-rail`, `emu-nav-<page>`, `emu-nav-count-<page>`, `emu-page-<page>`, `emu-edit-sheet`; every surviving id from Global Constraints.
  - Behaviour: `emulator-add` → catalog page, Catalog tab; `emulator-edit-<name>` → Installed page with the sheet open on that entry; a save from the Manual tab → Installed page; a save from the sheet → sheet closes; Cancel on the sheet closes it; Cancel on the Manual tab returns to the Catalog tab.

- [ ] **Step 1: Write the new `Emulators.svelte`**

Replace the whole of `app/src/lib/Emulators.svelte` with:

```svelte
<script lang="ts">
  // The Emulators view (design §9, D-UI-5): a 220px category rail and one
  // pane per category — Installed, Add from catalog, Platform defaults,
  // Compat tools (Linux only). All four panes stay mounted and switch with
  // `hidden`, the rule the shell applies to views: the catalog's refresh on
  // a finished install and the defaults' compatibility fetch keep running
  // whichever pane is in front. Each pane's column caps at 1100px (D-UI-7).
  import {
    api,
    type CatalogEntry,
    type EmulatorEntry,
    type LaunchDefaults,
    type Platform,
    type PlatformRef,
    type ProfileSummary,
  } from './api';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import { downloads } from './stores/downloads.svelte';
  import { compatTools } from './stores/compatTools.svelte';
  import { filterCatalogEntries } from './emulators/catalog';
  import {
    NO_CORE_VALUE,
    NO_DEFAULT_VALUE,
    platformCoreSelect,
    platformDefaultSelect,
  } from './emulators/defaults';
  import { isWindowsHost } from './emulators/compatTools';
  import {
    emulatorPageLabel,
    emulatorRailEntries,
    formPlacement,
    pageAfterSave,
    safeEmulatorPage,
    type AddTab,
    type EmulatorPage,
    type EmulatorPageCounts,
  } from './emulators/pages';
  import CompatTools from './emulators/CompatTools.svelte';
  import EmulatorForm from './emulators/EmulatorForm.svelte';

  // Mounted for the whole session now that Emulators is a view, so the
  // refresh below is gated on being the visible view: navigating away and
  // back re-runs `list_platforms`, which is what makes a cleared default
  // survive (the emulators spec's "(none)" case).
  let { active = true }: { active?: boolean } = $props();

  // The app's own OS, not the server's platform field (isNativePlatform) —
  // gates whether the Compat tools pane (wine/proton, which Windows-only
  // content has nothing to do with) exists at all.
  const windowsHost = isWindowsHost(navigator.platform);

  let page = $state<EmulatorPage>('installed');

  /** Programmatic page selection: the Server header's default-emulator chip
   *  routes to Platform defaults (design §6). */
  export function show(next: EmulatorPage) {
    page = safeEmulatorPage(next, windowsHost);
  }

  let emulators = $state<EmulatorEntry[]>([]);
  let listLoading = $state(true);
  let listError = $state<string | null>(null);
  let deleteError = $state<string | null>(null);

  let platforms = $state<Platform[]>([]);
  let defaults = $state<LaunchDefaults | null>(null);
  let defaultsError = $state<string | null>(null);

  // The backend's `compatible_emulators` answer, keyed by platform NAME (the
  // same string `default_emulators` is keyed by). The per-platform default
  // select offers only these, so an emulator is never offered for a platform
  // its profile does not support.
  let compatible = $state<Record<string, string[]>>({});
  // Its own error slot, never `defaultsError`: this fetch re-runs on every
  // platform/emulator change and would otherwise clear a real defaults error.
  let compatibleError = $state<string | null>(null);

  // The backend's `retroarch_core_options` answer, keyed by platform NAME.
  // Fetched on the same trigger set as `compatible`, because both depend on
  // the emulator list and on which core files are on disk.
  let coreOptions = $state<Record<string, string[]>>({});

  let profiles = $state<ProfileSummary[]>([]);

  // The Installed pane's edit sheet (design §9: "Edit opens the manual form
  // inline as a sheet on the right of the pane"). `name` is the entry's
  // current name, used as saveEmulator's originalName so a rename can find
  // & replace itself; `entry` is the row being edited, kept whole so the
  // fields the form does not show are written back untouched.
  let editing = $state<{ name: string; entry: EmulatorEntry } | null>(null);
  // The catalog pane's two tabs: the catalog rows, or the manual add form.
  let addTab = $state<AddTab>('install');
  let placement = $derived(formPlacement(page, editing !== null, addTab));

  let confirmingDelete = $state<string | null>(null);
  let deletePending = $state<string | null>(null);

  // Catalog pane state.
  let catalog = $state<CatalogEntry[]>([]);
  let catalogLoading = $state(true);
  let catalogError = $state<string | null>(null);
  let catalogSearch = $state('');
  let searchEl = $state<HTMLInputElement | null>(null);
  let installingSourceIds = $state<Set<string>>(new Set());
  let filteredCatalog = $derived(filterCatalogEntries(catalogSearch, catalog));

  // Signature of every emulator-job download that has reached a terminal
  // status — read inside the effects below so a fresh terminal entry (an
  // install completing, failing, or getting cancelled) triggers a catalog
  // re-fetch. Approximate on purpose (task-7-brief.md): any terminal
  // emulator entry is enough of a signal, not just the one just installed.
  let emulatorTerminalSignature = $derived(
    downloads.entries
      .filter((e) => e.job === 'emulator' && ['completed', 'failed', 'cancelled'].includes(e.status))
      .map((e) => `${e.id}:${e.status}`)
      .join(',')
  );

  // RPCS3 PS3 firmware note/button (task-17-brief.md). Keyed by emulator
  // entry name; `null` means the status was queried and no PS3UPDAT.PUP is
  // present yet, `undefined` means it hasn't been queried yet — either way
  // the note/button stay hidden until a query resolves with a non-empty path.
  let rpcs3Status = $state<Map<string, string | null>>(new Map());
  let ps3InstallPending = $state<Set<string>>(new Set());
  let ps3Toast = $state<{ entryName: string; ok: boolean; text: string } | null>(null);

  // Re-queried whenever a `firmware`-kind drawer entry reaches 'completed'
  // (task-17-brief.md): the background firmware installer finishing means a
  // freshly-downloaded PS3UPDAT.PUP may now be sitting next to RPCS3.
  let firmwareCompletedSignature = $derived(
    downloads.entries
      .filter((e) => e.kind === 'firmware' && e.status === 'completed')
      .map((e) => `${e.id}:${e.status}`)
      .join(',')
  );

  let counts = $derived<EmulatorPageCounts>({
    installed: emulators.length,
    catalog: catalog.length,
    defaults: platforms.length,
    compat: compatTools.tools.length,
  });

  let railRows = $derived(
    emulatorRailEntries(counts, page, windowsHost).map(
      (e): RailPaneEntry<EmulatorPage> => ({
        key: e.key,
        testId: e.testId,
        countTestId: e.countTestId,
        label: e.label,
        count: e.count,
        selected: e.selected,
        heading: e.heading,
      }),
    ),
  );

  function isRpcs3(name: string): boolean {
    return name.toLowerCase().includes('rpcs3');
  }

  async function refreshRpcs3StatusFor(name: string) {
    try {
      const status = await api.rpcs3FirmwareStatus(name);
      const next = new Map(rpcs3Status);
      next.set(name, status.pup_path);
      rpcs3Status = next;
    } catch {
      // Best-effort only — leave the prior status (or none) on failure.
    }
  }

  async function refreshAllRpcs3Status() {
    await Promise.all(emulators.filter((e) => isRpcs3(e.name)).map((e) => refreshRpcs3StatusFor(e.name)));
  }

  $effect(() => {
    const signature = firmwareCompletedSignature;
    void signature;
    refreshAllRpcs3Status();
  });

  async function handleInstallPs3Firmware(name: string) {
    ps3Toast = null;
    ps3InstallPending = new Set(ps3InstallPending).add(name);
    try {
      const ok = await api.installPs3Firmware(name);
      ps3Toast = ok
        ? {
            entryName: name,
            ok: true,
            text: 'PS3 firmware installation started — follow the RPCS3 dialog to complete.',
          }
        : {
            entryName: name,
            ok: false,
            text: 'Could not launch RPCS3 to install firmware. Check the emulator path.',
          };
    } catch {
      ps3Toast = {
        entryName: name,
        ok: false,
        text: 'Could not launch RPCS3 to install firmware. Check the emulator path.',
      };
    } finally {
      const next = new Set(ps3InstallPending);
      next.delete(name);
      ps3InstallPending = next;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshEmulators();
    refreshPlatformsAndDefaults();
    refreshProfiles();
  });

  // Both inputs of the compatibility and core answers: the platforms they
  // are asked about, and the emulator list the backend draws them from.
  // Reading both here is what makes a freshly added (or installed) emulator
  // show up in the per-platform selects without a reload.
  let compatibilityInputs = $derived({
    platformRefs: platforms.map((p) => ({ name: p.name, slug: p.slug })),
    emulatorNames: emulators.map((e) => e.name).join(','),
  });

  $effect(() => {
    const { platformRefs, emulatorNames } = compatibilityInputs;
    void emulatorNames;
    refreshCompatible(platformRefs);
    refreshCoreOptions(platformRefs);
  });

  // An emulator install reaching a terminal status can have ADDED an entry,
  // so the entry list and the stored defaults are both stale (the
  // compatibility effect above then re-runs off the new emulator list).
  $effect(() => {
    const signature = emulatorTerminalSignature;
    void signature;
    // Also fires once at mount, duplicating the mount effect's two fetches —
    // cheap, and it keeps the refresh rule free of a first-run special case.
    refreshEmulators();
    refreshDefaults();
  });

  // The catalog loads when the view comes forward (its count sits on the
  // rail from the first look) and reloads whenever an emulator download
  // reaches a terminal status, so Install/Installed never goes stale.
  $effect(() => {
    const signature = emulatorTerminalSignature;
    void signature;
    if (!active) return;
    refreshCatalog();
  });

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function sanitizeName(name: string): string {
    return name.toLowerCase().replace(/\s+/g, '-');
  }

  async function refreshEmulators() {
    listLoading = true;
    try {
      emulators = await api.listEmulators();
      listError = null;
      void refreshAllRpcs3Status();
    } catch (err) {
      listError = errorMessage(err);
    } finally {
      listLoading = false;
    }
  }

  async function refreshPlatformsAndDefaults() {
    try {
      const [p, d] = await Promise.all([api.listPlatforms(), api.getLaunchDefaults()]);
      platforms = p;
      defaults = d;
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshDefaults() {
    try {
      defaults = await api.getLaunchDefaults();
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshCompatible(refs: PlatformRef[]) {
    if (refs.length === 0) {
      compatible = {};
      return;
    }
    try {
      compatible = await api.compatibleEmulators(refs);
      compatibleError = null;
    } catch (err) {
      compatibleError = errorMessage(err);
    }
  }

  async function refreshCoreOptions(refs: PlatformRef[]) {
    if (refs.length === 0) {
      coreOptions = {};
      return;
    }
    try {
      coreOptions = await api.retroarchCoreOptions(refs);
      compatibleError = null;
    } catch (err) {
      // Shares the compatibility error slot (design §3.4) so a core-options
      // failure cannot clear a real defaults error.
      compatibleError = errorMessage(err);
    }
  }

  async function refreshProfiles() {
    try {
      profiles = await api.listProfiles();
    } catch {
      // Best-effort only — both auto-fills just won't find a match.
    }
  }

  async function refreshCatalog() {
    catalogLoading = true;
    try {
      catalog = await api.listEmulatorCatalog();
      catalogError = null;
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      catalogLoading = false;
    }
  }

  /** `emulator-add`: the Add from catalog page, on its Catalog tab. */
  function openAdd() {
    page = 'catalog';
    addTab = 'install';
    catalogError = null;
    catalogSearch = '';
    confirmingDelete = null;
  }

  function openEdit(entry: EmulatorEntry) {
    page = 'installed';
    editing = { name: entry.name, entry };
    confirmingDelete = null;
  }

  function closeSheet() {
    editing = null;
  }

  async function afterEditSave() {
    closeSheet();
    await refreshEmulators();
    await refreshDefaults();
    page = pageAfterSave('edit');
  }

  async function afterAddSave() {
    addTab = 'install';
    await refreshEmulators();
    await refreshDefaults();
    page = pageAfterSave('add');
  }

  async function handleInstallClick(sourceId: string) {
    catalogError = null;
    installingSourceIds = new Set(installingSourceIds).add(sourceId);
    try {
      await api.installEmulator(sourceId);
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      const next = new Set(installingSourceIds);
      next.delete(sourceId);
      installingSourceIds = next;
    }
  }

  function testKeyFor(sourceId: string): string {
    return sourceId.replaceAll('/', '-');
  }

  async function handleDeleteClick(name: string) {
    if (confirmingDelete !== name) {
      confirmingDelete = name;
      deleteError = null;
      return;
    }
    deleteError = null;
    deletePending = name;
    try {
      await api.deleteEmulator(name);
      if (editing?.name === name) closeSheet();
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      deleteError = errorMessage(err);
    } finally {
      deletePending = null;
      confirmingDelete = null;
    }
  }

  function selectFor(platformName: string) {
    return platformDefaultSelect(defaults, platformName, compatible[platformName] ?? []);
  }

  function coreSelectFor(platformName: string, selectedEmulator: string) {
    return platformCoreSelect(
      defaults,
      platformName,
      selectedEmulator,
      coreOptions[platformName] ?? []
    );
  }

  async function handleDefaultChange(platformName: string, value: string) {
    try {
      await api.setDefaultEmulator(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function handleCoreChange(platformName: string, value: string) {
    try {
      await api.setRetroarchCore(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }
</script>

<section data-testid="emulators-view" class="emulators" aria-label="Emulators">
  <RailPane entries={railRows} testId="emulators-rail" ariaLabel="Emulator categories" onSelect={(k) => (page = k)} />

  <div class="panes">
    <!-- Installed -->
    <section data-testid="emu-page-installed" class="pane" hidden={page !== 'installed'} aria-label={emulatorPageLabel('installed')}>
      <div class="view-content pane-inner">
        <div class="section-header">
          <h2>{emulatorPageLabel('installed')}</h2>
          <button data-testid="emulator-add" class="add-btn" onclick={openAdd}>+ Add emulator</button>
        </div>

        <div class="installed-body" class:with-sheet={placement === 'sheet'}>
          <div class="list-column">
            {#if listLoading}
              <p class="muted">Loading…</p>
            {:else if listError}
              <p class="error" role="alert">{listError}</p>
            {:else}
              {#if deleteError}
                <p class="error" role="alert">{deleteError}</p>
              {/if}
              {#if emulators.length === 0}
                <p class="muted">No emulators configured.</p>
              {:else}
                <ul class="emulator-list">
                  {#each emulators as e (e.name)}
                    <li data-testid={`emulator-row-${sanitizeName(e.name)}`} class="emulator-row" class:editing={editing?.name === e.name}>
                      <div class="row-main">
                        <div class="row-text">
                          <span class="name">{e.name}</span>
                          <span class="path" title={e.path}>{e.path}</span>
                          {#if e.args}<span class="args">{e.args}</span>{/if}
                        </div>
                        <div class="row-actions">
                          <button data-testid={`emulator-edit-${sanitizeName(e.name)}`} onclick={() => openEdit(e)}>Edit</button>
                          <button
                            data-testid={`emulator-delete-${sanitizeName(e.name)}`}
                            class:confirm={confirmingDelete === e.name}
                            disabled={deletePending === e.name}
                            onclick={() => handleDeleteClick(e.name)}
                          >
                            {confirmingDelete === e.name ? 'Confirm delete' : 'Delete'}
                          </button>
                        </div>
                      </div>
                      {#if isRpcs3(e.name) && rpcs3Status.get(e.name)}
                        <div class="ps3-firmware">
                          <p data-testid={`emulator-ps3-firmware-note-${sanitizeName(e.name)}`} class="hint">
                            PS3 firmware downloaded — click Install to activate it.
                          </p>
                          <button
                            data-testid={`emulator-ps3-firmware-${sanitizeName(e.name)}`}
                            disabled={ps3InstallPending.has(e.name)}
                            onclick={() => handleInstallPs3Firmware(e.name)}
                          >
                            {ps3InstallPending.has(e.name) ? 'Installing…' : 'Install PS3 Firmware'}
                          </button>
                        </div>
                      {/if}
                      {#if ps3Toast && ps3Toast.entryName === e.name}
                        <p data-testid="emulator-ps3-firmware-toast" class={ps3Toast.ok ? 'hint' : 'error'}>
                          {ps3Toast.text}
                        </p>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </div>

          {#if placement === 'sheet' && editing}
            <aside data-testid="emu-edit-sheet" class="sheet" aria-label="Edit emulator">
              <h3>Edit emulator</h3>
              <!-- Keyed on the entry name: the form seeds its fields on
                   mount, so switching rows must remount it. -->
              {#key editing.name}
                <EmulatorForm mode="edit" entry={editing.entry} {profiles} onSaved={afterEditSave} onCancel={closeSheet} />
              {/key}
            </aside>
          {/if}
        </div>
      </div>
    </section>

    <!-- Add from catalog -->
    <section data-testid="emu-page-catalog" class="pane" hidden={page !== 'catalog'} aria-label={emulatorPageLabel('catalog')}>
      <div class="view-content pane-inner">
        <h2>{emulatorPageLabel('catalog')}</h2>

        <div class="tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={addTab === 'install'}
            class:active={addTab === 'install'}
            data-testid="emu-add-tab-install"
            onclick={() => (addTab = 'install')}
          >
            Catalog
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={addTab === 'manual'}
            class:active={addTab === 'manual'}
            data-testid="emu-add-tab-manual"
            onclick={() => (addTab = 'manual')}
          >
            Manual
          </button>
        </div>

        {#if placement === 'manual'}
          <div class="manual-form">
            <EmulatorForm mode="add" entry={null} {profiles} onSaved={afterAddSave} onCancel={() => (addTab = 'install')} />
          </div>
        {:else}
          <div class="catalog-tab">
            <input
              data-testid="emu-catalog-search"
              class="catalog-search"
              type="search"
              placeholder="Search emulators…"
              bind:this={searchEl}
              bind:value={catalogSearch}
              aria-label="Search emulators"
            />
            {#if catalogError}<p class="error" role="alert">{catalogError}</p>{/if}
            {#if catalogLoading}
              <p class="muted">Loading…</p>
            {:else if filteredCatalog.length === 0}
              <p class="muted">No emulators found.</p>
            {:else}
              <ul class="catalog-list">
                {#each filteredCatalog as entry (entry.source_id)}
                  {@const testKey = testKeyFor(entry.source_id)}
                  <li class="catalog-row">
                    <div class="row-text">
                      <span class="name">{entry.name}</span>
                      <span class="meta">{entry.provider} • {entry.tag}</span>
                    </div>
                    {#if entry.installed}
                      <button data-testid={`emu-catalog-installed-${testKey}`} disabled>Installed</button>
                    {:else}
                      <button
                        data-testid={`emu-catalog-install-${testKey}`}
                        disabled={installingSourceIds.has(entry.source_id)}
                        onclick={() => handleInstallClick(entry.source_id)}
                      >
                        {installingSourceIds.has(entry.source_id) ? 'Installing…' : 'Install'}
                      </button>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
      </div>
    </section>

    <!-- Platform defaults -->
    <section data-testid="emu-page-defaults" class="pane" hidden={page !== 'defaults'} aria-label={emulatorPageLabel('defaults')}>
      <div class="view-content pane-inner">
        <h2>{emulatorPageLabel('defaults')}</h2>
        {#if defaultsError}
          <p class="error" role="alert">{defaultsError}</p>
        {/if}
        <!-- Rendered separately from the defaults error so neither can hide the
             other: the compatibility fetch has its own failure mode. -->
        {#if compatibleError}
          <p class="error" role="alert">{compatibleError}</p>
        {/if}
        {#if platforms.length === 0}
          <p class="muted">No platforms available.</p>
        {:else}
          <ul class="defaults-list">
            {#each platforms as p (p.id)}
              {@const selectId = `default-emulator-${p.id}`}
              {@const choice = selectFor(p.name)}
              {@const coreId = `default-core-${p.id}`}
              {@const core = coreSelectFor(p.name, choice.selected)}
              <li class="defaults-card">
                <div class="defaults-card-header">
                  <label class="platform-name" for={selectId}>{p.name}</label>
                </div>
                <div class="defaults-field">
                  <span class="defaults-field-label">Emulator</span>
                  <!-- `default-select-<platformId>` is the per-platform select's
                       test id; its `id` (used by the label) is
                       `default-emulator-<platformId>`. -->
                  <select
                    data-testid={`default-select-${p.id}`}
                    id={selectId}
                    disabled={choice.disabled}
                    value={choice.selected}
                    onchange={(e) => handleDefaultChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
                  >
                    {#if choice.disabled}
                      <option value={NO_DEFAULT_VALUE}>No compatible emulator</option>
                    {:else}
                      <option value={NO_DEFAULT_VALUE}>(none)</option>
                      {#each choice.options as name (name)}
                        <option value={name}>{name}</option>
                      {/each}
                    {/if}
                  </select>
                </div>
                {#if core.visible}
                  <div class="defaults-field">
                    <label class="defaults-field-label" for={coreId}>Core</label>
                    <select
                      data-testid={`default-core-${p.id}`}
                      id={coreId}
                      disabled={core.disabled}
                      value={core.selected}
                      onchange={(e) => handleCoreChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
                    >
                      {#if core.disabled}
                        <option value={NO_CORE_VALUE}>No installed core</option>
                      {:else}
                        {#each core.options as id (id)}
                          <option value={id}>{id}</option>
                        {/each}
                      {/if}
                    </select>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </section>

    <!-- Compat tools (design §9: hidden on Windows) -->
    {#if !windowsHost}
      <section data-testid="emu-page-compat" class="pane" hidden={page !== 'compat'} aria-label={emulatorPageLabel('compat')}>
        <div class="view-content pane-inner">
          <h2>{emulatorPageLabel('compat')}</h2>
          <CompatTools />
        </div>
      </section>
    {/if}
  </div>
</section>

<style>
  .emulators {
    display: flex;
    align-items: stretch;
    height: 100%;
    min-height: 0;
  }

  .panes {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  /* No `display` on `.pane` itself: the `hidden` attribute's UA rule must
     win, and an author `display: flex` here would override it. */
  .pane {
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .pane[hidden] {
    display: none;
  }

  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 18px;
    font-weight: 600;
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 14px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .add-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .add-btn:hover {
    background: var(--surface);
  }

  /* Design §9: the edit sheet sits to the right of the list. */
  .installed-body {
    display: flex;
    align-items: flex-start;
    gap: 24px;
  }

  .list-column {
    flex: 1 1 auto;
    min-width: 0;
  }

  .sheet {
    flex: 0 0 360px;
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface-2);
    animation: sheet-in var(--m-base) ease;
  }

  @keyframes sheet-in {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .emulator-list,
  .defaults-list,
  .catalog-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .emulator-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    border: 1px solid transparent;
    background: var(--surface);
    transition: border-color var(--m-fast) ease;
  }

  .emulator-row.editing {
    border-color: var(--primary);
  }

  .row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .ps3-firmware {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }

  .ps3-firmware .hint {
    flex: 1 1 auto;
    min-width: 0;
  }

  .ps3-firmware button,
  .row-actions button,
  .catalog-row button {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .ps3-firmware button:hover:not(:disabled),
  .row-actions button:hover:not(:disabled),
  .catalog-row button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .ps3-firmware button:disabled,
  .row-actions button:disabled,
  .catalog-row button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .row-actions button.confirm {
    background: var(--danger);
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    color: var(--text-h);
    font-weight: 500;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path {
    color: var(--text-muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .args,
  .meta {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .tabs {
    display: flex;
    gap: 4px;
  }

  .tabs button {
    font: inherit;
    font-size: 13px;
    padding: 6px 12px;
    border-radius: var(--r-chip) var(--r-chip) 0 0;
    border: 1px solid var(--border);
    border-bottom: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--m-fast) ease, color var(--m-fast) ease;
  }

  .tabs button.active {
    background: var(--surface);
    color: var(--text-h);
  }

  .manual-form {
    max-width: 480px;
  }

  .catalog-tab {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .catalog-search {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .catalog-search:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .catalog-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .defaults-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .defaults-card-header {
    display: flex;
  }

  .platform-name {
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    white-space: normal;
  }

  .defaults-field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .defaults-field-label {
    color: var(--text-muted);
    font-size: 13px;
    flex-shrink: 0;
  }

  .defaults-card select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
```

In `app/src/lib/emulators/CompatTools.svelte` change the `.error` rule's `color: #e5484d;` to `color: var(--danger);`. Nothing else in that file changes.

- [ ] **Step 2: Update `emulators.spec.ts`**

Add the pane helper directly after the `sanitize` const (line 71):

```ts
  /**
   * Design §9: the view is a rail of four panes and only the selected one
   * is displayed. Every pane stays mounted, so a `waitForExist` on a hidden
   * element passes — but a click or `getText` needs the pane in front.
   */
  async function showPage(page: 'installed' | 'catalog' | 'defaults' | 'compat') {
    await $(testId(`emu-nav-${page}`)).click();
    await $(testId(`emu-page-${page}`)).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the ${page} pane never came forward`,
    });
  }
```

Insert this case as the **first** `it` (before `auto-fills name and args…`):

```ts
  it('walks the four category panes of the rail', async () => {
    // Nothing is configured yet in this group: the Installed count is 0.
    await expect($(testId('emu-nav-count-installed'))).toHaveText('0');

    await showPage('catalog');
    await expect($(testId('emu-catalog-search'))).toBeDisplayed();
    await expect($(testId('emu-add-tab-install'))).toHaveAttribute('aria-selected', 'true');

    await showPage('defaults');
    await expect($(testId('default-select-1'))).toBeDisplayed();

    // Linux host: Compat tools is on the rail (design §9 hides it on Windows).
    await showPage('compat');
    await expect($(testId('compat-tools-section'))).toBeDisplayed();

    await showPage('installed');
    await expect($(testId('emulator-add'))).toBeDisplayed();
  });
```

In `auto-fills name and args from a profile-matching path, then saves the row`, after `await $(testId('emulator-add')).click();` and its comment add:

```ts
    // `emulator-add` opens the Add from catalog pane on its Catalog tab.
    await $(testId('emu-page-catalog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
```

and after the final `waitForExist` on the saved row add:

```ts
    // A manual save lands on Installed, where the new row is.
    await expect($(testId('emu-page-installed'))).toBeDisplayed();
    await expect($(testId('emu-nav-count-installed'))).toHaveText('1');
```

In `adds a second emulator and keeps row order when editing the first`, after the `emulator-edit-…` click replace the `emu-form-name` `waitForExist` with:

```ts
    // Design §9: Edit opens the form as a sheet beside the list.
    await $(testId('emu-edit-sheet')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the edit sheet never opened',
    });
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
```

and after the renamed row's `waitForExist` add:

```ts
    await $(testId('emu-edit-sheet')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the edit sheet stayed open after a successful save',
    });
```

In `deletes an emulator with a two-click confirm`, insert `await showPage('installed');` as the first line (the duplicate-name case before it ends on the catalog pane).

In `assigns a per-platform default and records a core in config.toml`, insert `await showPage('defaults');` as the first line.

In `the (none) choice survives leaving and re-entering the view`, after the `nav-emulators` click replace the `default-select-1` `waitForExist` with:

```ts
    await showPage('defaults');
    await $(testId('default-select-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the per-platform defaults list never rendered after re-entering',
    });
```

- [ ] **Step 3: Update `emulator-catalog.spec.ts`**

Add the same `showPage` helper (identical code to Step 2) after `closeEmulators`. Replace `openCatalog` with:

```ts
  /** The Add from catalog pane, on its Catalog tab (design §9). */
  async function openCatalog() {
    await showPage('catalog');
    await $(testId('emu-add-tab-install')).click();
    await $(testId('emu-catalog-search')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the catalog pane never rendered its search box',
    });
  }
```

In `installs PCSX2 from the catalog and marks it installed`, before the `browser.waitUntil` on `optionValues(PS2_SELECT)` insert:

```ts
    // The defaults pane stays mounted behind the catalog (its selects are
    // readable while hidden), but the disabled-state assertion below reads
    // rendered text, so bring it forward.
    await showPage('defaults');
```

and replace the comment + `closeEmulators(); openEmulators();` pair before `$(testId(PCSX2_ROW)).waitForExist` with:

```ts
    // The terminal-status effect already re-read the list; the row's text
    // is only readable once its pane is in front.
    await showPage('installed');
```

In `plays the seeded PS2 game with the installed PCSX2 as the platform default`, insert `await showPage('defaults');` before `await selectValue(PS2_SELECT, PCSX2_NAME);`.

In `installs Redream by scraping its download page…`, replace the `closeEmulators(); openEmulators();` pair before `$(testId(REDREAM_ROW)).waitForExist` with `await showPage('installed');`.

- [ ] **Step 4: Update `launch.spec.ts`**

Add the `showPage` helper (identical code to Step 2) after `closeEmulators` (line 84). In `shows "exited immediately" after switching the default to the instant-exit stub` insert `await showPage('defaults');` after `await openEmulators();`. In `shows the verbatim "Emulator executable not found:" error…` replace

```ts
    await openEmulators();
    await $(testId('emulator-edit-instantexit')).click();
    await $(testId('emu-form-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
```

with

```ts
    await openEmulators();
    await showPage('installed');
    await $(testId('emulator-edit-instantexit')).click();
    await $(testId('emu-edit-sheet')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-form-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
```

- [ ] **Step 5: Update `firmware.spec.ts`**

Replace `showEmulators` (lines 61–67) with:

```ts
  async function showEmulators() {
    await $(testId('nav-emulators')).click();
    await $(testId('emulators-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators view never rendered',
    });
    // The RPCS3 row, its note and its button live on the Installed pane.
    await $(testId('emu-nav-installed')).click();
    await $(testId('emu-page-installed')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Installed pane never came forward',
    });
  }
```

In `fetches the PS3 PUP through its own drawer row when RPCS3 is added by hand`, after `await $(testId('emulator-add')).click();` add:

```ts
    await $(testId('emu-page-catalog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
```

(The manual save then lands on Installed, where `emulator-row-rpcs3` is waited for.)

- [ ] **Step 6: Check, run the four groups, and commit**

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green.

Run, from `rewrite/`: `scripts/e2e.sh emulators emulator-catalog launch firmware`
Expected: PASS ×4.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/Emulators.svelte rewrite/app/src/lib/emulators/CompatTools.svelte rewrite/e2e/specs/emulators.spec.ts rewrite/e2e/specs/emulator-catalog.spec.ts rewrite/e2e/specs/launch.spec.ts rewrite/e2e/specs/firmware.spec.ts
git commit -m "rewrite: give the Emulators view its category rail, panes and edit sheet"
```

---
### Task 7: Ctrl+F on the Emulators view, the Server chip route, and the full gate

**Files:**
- Modify: `app/src/lib/Emulators.svelte` (imports; one `focusSearch` export, one `onKey` handler, one `<svelte:window>`)
- Modify: `app/src/lib/Shell.svelte:31-33` (`emulators` ref), `:179-190` (the Server and Emulators mounts)
- Modify: `e2e/specs/library-grid.spec.ts` (one new case after `shows the platform header…`)

**Interfaces:**
- Consumes: `SEARCH_PAGE` (Task 1); `Emulators.show` (Task 6); `chordBlocked`, `chordContext`, `shouldFocusSearch` from `views/searchKeys.ts`; `tick` from `svelte`.
- Produces: `Emulators.focusSearch(): Promise<void>`; the Server header's `server-emulator-chip` opens the Emulators view **on the Platform defaults pane** (design §6).

- [ ] **Step 1: Wire Ctrl+F in `Emulators.svelte`**

Add to the imports:

```ts
  import { tick } from 'svelte';
  import { chordContext, shouldFocusSearch } from './views/searchKeys';
```

and add `SEARCH_PAGE` to the `./emulators/pages` import list. After the `show` function add:

```ts
  /**
   * Design §3: `Ctrl+F` focuses the current view's search box. This view's
   * one search lives on the catalog pane, so the chord brings that pane
   * forward first; the input is hidden until the DOM updates, hence `tick`.
   */
  export async function focusSearch() {
    page = SEARCH_PAGE;
    await tick();
    searchEl?.focus();
    searchEl?.select();
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    // `shouldFocusSearch` already applies `chordBlocked`: a modal dialog or
    // a focused text control keeps the chord for itself.
    if (!shouldFocusSearch(e, chordContext(document))) return;
    e.preventDefault();
    focusSearch();
  }
```

Directly above the root `<section data-testid="emulators-view" …>` add:

```svelte
<svelte:window onkeydown={onKey} />
```

- [ ] **Step 2: Route the Server chip to Platform defaults**

In `app/src/lib/Shell.svelte` add, after `let settings = $state<ReturnType<typeof Settings> | null>(null);`:

```ts
  let emulators = $state<ReturnType<typeof Emulators> | null>(null);
```

Replace the Server and Emulators mounts (the two `<div class="view" …>` blocks for `server-view` and the unnamed emulators wrapper) with:

```svelte
<div data-testid="server-view" class="view" hidden={view !== 'server'}>
  <Server
    active={view === 'server'}
    onOpenEmulators={() => {
      // Design §6: the default-emulator chip links to Emulators › Platform defaults.
      view = 'emulators';
      emulators?.show('defaults');
    }}
    bind:this={server}
  />
</div>
<div data-testid="downloads-view" class="view" hidden={view !== 'downloads'}>
  <Downloads />
</div>
<div class="view" hidden={view !== 'emulators'}>
  <Emulators active={view === 'emulators'} bind:this={emulators} />
</div>
```

(The Downloads block is repeated only so the three wrappers read in pill order; its content is unchanged.)

- [ ] **Step 3: Add the chip case to `library-grid.spec.ts`**

After the case `shows the platform header…` (the one that asserts `server-emulator-chip` is displayed) append:

```ts
  it('routes the default-emulator chip to Emulators › Platform defaults (design §6)', async () => {
    await $(testId('server-emulator-chip')).click();
    await $(testId('emulators-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators view never opened from the chip',
    });
    await expect($(testId('emu-page-defaults'))).toBeDisplayed();
    await expect($(testId('default-select-1'))).toBeDisplayed();

    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back',
    });
  });
```

- [ ] **Step 4: Check, then run every E2E group**

Run, from `rewrite/`: `cargo fmt && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy -p app --all-targets --features e2e -- -D warnings`
Expected: clean. Then, from `rewrite/`: `cargo test --workspace` — green (Task 3 changed Rust; the gate runs for the whole plan here).

Run, from `rewrite/app`: `npm run check && npx vitest run`
Expected: green.

Run, from `rewrite/`: `scripts/e2e.sh`
Expected: every group PASS. If a group fails, fix and re-run the whole script before committing — this is the plan's final code task.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/Emulators.svelte rewrite/app/src/lib/Shell.svelte rewrite/e2e/specs/library-grid.spec.ts
git commit -m "rewrite: Ctrl+F on Emulators and the Server chip route to Platform defaults"
```

---

### Task 8: Documentation

**Files:**
- Modify: `SPEC.md:56-60` (the Emulators, Settings and Appearance bullets)
- Modify: `rewrite/README.md` (the `cloud-saves` and `updates` rows of the stage table; the "RetroArch core picker", "RetroArch platform gating" and "Explicit (none)" checklist rows; two new rows)
- Modify: `docs/porting/04-emulator-launch.md` (after item 5 under the rulings, line 1117), `docs/porting/06-cloud-saves.md:1383-1384`, `docs/porting/10-identity-updates.md:826-827`

- [ ] **Step 1: Rewrite the SPEC.md bullets**

Replace the three bullets at `SPEC.md` lines 56–60 (`- **Emulators** …`, `- **Settings** …`, `- **Appearance** …`) with:

```markdown
- **Emulators** is a rail of four category panes, each capped at 1100px and centred:
  **Installed** lists every configured emulator (name, path, arguments) with Edit and
  Delete — Edit opens the manual form as a sheet beside the list — and, for an RPCS3
  entry with a downloaded PS3 firmware package, the note and the Install PS3 Firmware
  button; **Add from catalog** has a search box, one row per catalog entry with its
  provider and an Install / Installed button, and a Manual tab for the hand-typed form
  (name, executable path, arguments, with the profile auto-fill); **Platform defaults**
  shows one card per server platform with the emulator select (only compatible
  emulators, plus "(none)") and, for a RetroArch default, the core select; **Compat
  tools** (Linux only) is the wine/proton default picker and its install catalog. Each
  emulator entry includes a name, executable path, launch arguments, save strategy,
  ignore rules, and optional custom save/state directories.
- **Settings** is a rail of five panes: **Connection** (server URL, user, whether a
  credential is stored in the OS keyring and whether the session is verified, Reconnect,
  Disconnect), **Cloud saves** (restore before launch, upload after exit, skip when the
  local save is newer, upload delay, retention limit), **RetroAchievements** (username
  and a write-only token; Save fans the credentials out to every emulator that supports
  them, Clear removes them), **Updates** (the running version, when the startup check
  ran, the release notice with its Open release and Dismiss buttons, and the note that
  the launcher only ever checks — it never downloads or installs an update), and
  **Appearance**.
- **Appearance** (under Settings) chooses the theme — follow the OS, dark, or light —
  turns the background art on or off, sets its fade from 0 to 60 percent with a live
  preview behind the pane, and sets the default card size for the Library and Server
  grids. They are stored under `[ui]` in `config.toml` as `theme`, `background_fade`
  (0 is off), `card_size_library` and `card_size_server`.
```

- [ ] **Step 2: Update the README**

In the stage table, in the `cloud-saves` row replace `(delay zeroed via the Emulators settings UI)` with `(delay zeroed via Settings › Cloud saves)`. In the `updates` row replace `the self-update banner appears with the mock forge's tag `v9.9.9-e2e` (`GRID_LAUNCHER_E2E_UPDATE_CHECK=1` lifts the dev-build gate for this group only) and Dismiss hides it` with `the self-update badge appears on the top bar with the mock forge's tag `v9.9.9-e2e` (`GRID_LAUNCHER_E2E_UPDATE_CHECK=1` lifts the dev-build gate for this group only), opens Settings › Updates, and Dismiss hides the badge while the Updates entry stays; the Settings rail's Connection, Updates, Cloud saves, RetroAchievements and Appearance panes each render their own line`.

In the "Residual manual checklist": in **RetroArch core picker** replace `confirm the Emulators panel shows a Core select` with `confirm Emulators › Platform defaults shows a Core select`; in **RetroArch platform gating** replace `after the panel refreshes` with `after the pane refreshes`; in **Explicit "(none)"** replace `set a platform to (none), reopen the panel, it stays (none)` with `set a platform to (none) on Emulators › Platform defaults, switch views and back, it stays (none)`. Append two rows:

```markdown
- **Edit sheet**: on Emulators › Installed press Edit on one row, then Edit on another
  and confirm the sheet's fields switch to the second entry; press Cancel and confirm the
  row highlight clears. Press Ctrl+F from any Emulators pane and confirm the catalog pane
  comes forward with its search box focused.
- **Background art off**: untick Settings › Appearance › Background art and confirm the
  art disappears and `background_fade = 0` reaches `config.toml`; tick it again and
  confirm the previous fade value returns.
```

- [ ] **Step 3: Update the three porting docs**

In `docs/porting/04-emulator-launch.md`, after item 5's last line (`emulator appears in the selector without reopening the panel.`, line 1117) append a new item:

```markdown
6. **The Emulators UI is a view with a category rail, not a panel.** Since the desktop
   redesign (plan 5, `docs/superpowers/plans/2026-09-04-ui-redesign-5-emulators-settings.md`)
   `app/src/lib/Emulators.svelte` renders four always-mounted panes — Installed (with the
   edit form as a sheet, `app/src/lib/emulators/EmulatorForm.svelte`), Add from catalog,
   Platform defaults, Compat tools (Linux only) — selected by `emu-nav-<page>`. The
   per-platform default and core selects above live on the Platform defaults pane; the
   Server header's default-emulator chip opens that pane directly. The RetroAchievements
   and cloud-save settings forms moved to the Settings view.
```

In `docs/porting/06-cloud-saves.md` replace lines 1383–1384

```
   editable from the Emulators panel's
   Cloud Saves settings block (`app/src/lib/Emulators.svelte`,
```

with

```
   editable from Settings › Cloud saves
   (`app/src/lib/settings/CloudSavesPage.svelte`,
```

In `docs/porting/10-identity-updates.md` replace, in D-10-k (lines 826–827), `shows the banner even when it mounts after the event fired.` with `shows the top-bar badge and the Settings › Updates entry even when it mounts after the event fired (the banner strip was removed by the desktop redesign; Dismiss hides the badge only).`

- [ ] **Step 4: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add SPEC.md rewrite/README.md docs/porting/04-emulator-launch.md docs/porting/06-cloud-saves.md docs/porting/10-identity-updates.md
git commit -m "rewrite: document the redesigned Emulators and Settings views"
```

---

## Self-review

**1. Spec coverage.**

| Spec requirement (§9 / §10 / §3 / D-UI-5 / D-UI-7 / §11 / §12.5) | Task |
|---|---|
| D-UI-5 / §9: Emulators rail — Installed, Add from catalog, Platform defaults, Compat tools (hidden on Windows) | 1 (`EMULATOR_PAGES`, `visibleEmulatorPages`), 6 (rail + panes, `{#if !windowsHost}`) |
| §9 Installed: rows with name, path, source, Edit / Remove, the RPCS3 firmware note and button | 6 (rows unchanged; `path` and `args` shown — the entry type carries no separate source string, the path is the source) |
| §9 "Edit opens the manual form inline as a sheet on the right of the pane" | 1 (`formPlacement` → `'sheet'`), 2 (`EmulatorForm`), 6 (`emu-edit-sheet`, `.installed-body` / `.sheet`) |
| §9 Add from catalog: search box, rows with provider and Install / Installed, a "Manual" button | 6 (`emu-catalog-search`, rows, the Catalog / Manual tabs — see deviations) |
| §9 Platform defaults: the card list shipped on 2026-09-04 | 6 (moved verbatim) |
| §9 Compat tools: the current `CompatTools` content | 6 (`<CompatTools />` on `emu-page-compat`) |
| §10 Connection: server URL, token status, reconnect, disconnect | 4 (`connection.ts`), 5 (`ConnectionPage`) |
| §10 Cloud saves: current cloud settings form | 5 (`CloudSavesPage`, moved verbatim) |
| §10 RetroAchievements: current form | 5 (`RetroAchievementsPage`, moved verbatim) |
| §10 Updates: app version, last check, release link, "check-only" note | 3 (`checked_at` on the payload), 4 (`updates.ts`, `appUpdate.checkedAt`), 5 (`UpdatesPage`) |
| Controller ruling: "not checked" distinguishable from "up to date" | 3 (three-state `CheckOutcome`, five Rust tests), 4 (`updateStatusLine` three states, vitest) |
| §10 Appearance: theme, card size defaults, background art on/off, fade slider with live preview | plan 1 (theme, fade) + 4 (`appearance.ts`, `setBackgroundEnabled`) + 5 (`AppearancePage`: toggle, two card-size selects) |
| §3 app-update badge on the user menu + Settings › Updates entry; banner strip removed | plan 1 (done); 5 keeps the entry visible after Dismiss (`appUpdate.stored`) |
| §3 Ctrl+F focuses the current view's search | 7 (`focusSearch`, `onKey`) |
| D-UI-7 content columns cap at 1100px and centre | 5 and 6 (`.view-content` on every `pane-inner`) |
| §11 renames (`emulators-open` → `nav-emulators`, `emulators-panel` → view root, `emulators-close` removed) | plan 1 (done; confirmed by grep in the header) |
| §11 new ids `emu-nav-<page>`, `settings-nav-<page>`, `theme-select` with E2E | 6 (`emu-nav-*` cases in `emulators.spec`), 5 (`settings-nav-*` walk in `updates.spec`; `theme-select` case already in `updates.spec`) |
| §11 survivors keep working: `emulator-*`, `emu-*`, `default-select-<id>`, `default-core-<id>`, `compat-*`, `ra-*`, `cloud-settings-*`, `app-update-*` | 5, 6 (ids copied unchanged; the four Emulators groups + cloud-saves + updates rerun) |
| §12.5 "removal of the old modal and its ids" | plan 1 (done); nothing named `emulators-open` / `-panel` / `-close` remains to remove |
| §12 SPEC.md and README updated | 8 |
| §6 platform header's default emulator chip links to Emulators › Platform defaults | 7 (`emulators.show('defaults')`, `library-grid.spec` case) |

No §9 / §10 requirement is left without a task. The decisions made against the text (mounted panes, Manual as a tab, where a save lands, the credential line, the `checked_at` Rust change, on/off over the fade) are listed under "Deliberate deviations".

**2. Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N": Task 6 repeats the whole component rather than diffing Task 2's and Task 5's edits; Task 5 repeats the RetroAchievements and Cloud saves forms verbatim rather than saying "move them"; Task 3 repeats the full `fetch_outcome_from` body rather than "rename and change the returns"; each spec change quotes the exact lines to insert or replace.

**3. Type consistency.**

- `EmulatorPage` is `'installed' | 'catalog' | 'defaults' | 'compat'` in Task 1 and every id built from it — `emu-nav-<page>`, `emu-nav-count-<page>`, `emu-page-<page>` — matches the strings the specs click in Tasks 6 and 7 (`emu-nav-catalog`, `emu-page-defaults`, …).
- `formPlacement(page, editing: boolean, addTab)` — Task 6 passes `editing !== null`, a boolean, and `addTab` typed `AddTab`; the three return values `'sheet' | 'manual' | null` are the three branches Task 6's markup tests.
- `EmulatorForm` props (`mode`, `entry`, `profiles`, `onSaved`, `onCancel`) are the same in Task 2's component, Task 2's interim call site and Task 6's two call sites; `entry` is `null` for add and `editing.entry` for edit, never `undefined`.
- `RailPaneEntry.count` / `countTestId` optional (Task 1): Task 5's Settings rows omit both; Task 6's Emulators rows pass both; Library and Server are untouched.
- `AppUpdateStatus { notice, checked_at }` has the same two fields in Rust (Task 3), in `api.ts` (Task 3) and in the store's `applyStatus` (Task 4); `checked_at` is a `string | null` everywhere on the TS side — `appUpdate.checkedAt: string | null` (Task 4), `updateStatusLine(notice, checkedAt: string | null, nowMs)` (Task 4, its test, and `UpdatesPage` in Task 5), `relativeCheckTime(checkedAt: string, nowMs)` (Task 4). `CheckOutcome`'s three variants are the three `record` arms and the four `fetch_outcome_from` test assertions.
- `settingsRailEntries(selected: SettingsPage)` (Task 4) → Task 5 maps it to `RailPaneEntry<SettingsPage>`; `SETTINGS_PAGES` order equals the §10 rail order and the ids in the `updates.spec` walk.
- `setBackgroundEnabled(enabled: boolean)` (Task 4 store) is what `AppearancePage.onToggle` calls (Task 5); `fadeForToggle(enabled, remembered)` / `rememberFade(fade, remembered)` argument orders match between `appearance.ts`, its test and the store.
- `Emulators.show(next: EmulatorPage)` (Task 6) is called with `'defaults'` from `Shell.svelte` (Task 7); `Settings.show('updates')` is unchanged from plan 1.
- Test ids in the new spec cases — `emu-edit-sheet`, `emu-nav-count-installed`, `settings-page-*`, `settings-connection-*`, `settings-updates-version` / `-status` / `-note`, `background-art-toggle`, `card-size-library` — are all produced by Tasks 5 and 6 and named in their Interfaces blocks.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-04-ui-redesign-5-emulators-settings.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration. REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute the tasks in one session with checkpoints. REQUIRED SUB-SKILL: `superpowers:executing-plans`.

Which approach?
