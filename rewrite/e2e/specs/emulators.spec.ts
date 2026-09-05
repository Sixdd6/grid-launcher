import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `emulators`: the Emulators panel's CRUD flows (add with autofill,
 * edit-in-place ordering, duplicate rejection, two-click delete) and the
 * per-platform defaults select, asserted against the real config.toml on
 * disk.
 *
 * Only the `emulator-launch-*` case spawns the emulator; every other flow
 * here just needs the stub to exist on disk with its executable bit set,
 * because `match_profile` (grid-core's `profile_for_entry`) matches purely
 * on the path string, not on file contents. The stub therefore touches a
 * marker file, which is how the launch case proves the spawn happened.
 */
describe('emulators', () => {
  let stubPath: string;
  let launchMarker: string;

  before(async () => {
    const stubsDir = path.join(dataDir(), 'stubs');
    mkdirSync(stubsDir, { recursive: true });
    // Basename "retroarch" is a literal `emulator-autoprofiles.json`
    // match_tokens entry (root/emulator-autoprofiles.json), matching the
    // "RetroArch (Multi-System)" profile: name "RetroArch (Multi-System)",
    // args `-L "%core%" "%rom%"`.
    stubPath = path.join(stubsDir, 'retroarch');
    launchMarker = path.join(stubsDir, 'retroarch.launched');
    // The `emulator-launch-*` case below is the only thing in this group
    // that runs the stub; the marker is how it proves the spawn happened.
    writeFileSync(stubPath, `#!/bin/sh\ntouch '${launchMarker}'\nexit 0\n`);
    chmodSync(stubPath, 0o755);

    // Design D-RC-1: RetroArch's platform support is now decided by the
    // core files installed beside its executable, so the stub needs a
    // `cores/` sibling. Two SNES cores in the bundled slug map's curated
    // order (romm-platform-cores.json maps "snes" to
    // ["snes9x", "snes9x2010", "bsnes"]) and no Arcade core at all, so
    // platform 1 offers RetroArch and platform 2 (Arcade) does not.
    const coresDir = path.join(stubsDir, 'cores');
    mkdirSync(coresDir, { recursive: true });
    for (const core of ['snes9x', 'bsnes']) {
      writeFileSync(path.join(coresDir, `${core}_libretro.so`), '');
    }

    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });

    await $(testId('nav-emulators')).click();
    await $(testId('emulators-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators view never rendered',
    });
  });

  /** Row/edit/delete testids sanitize a name the same way Emulators.svelte does. */
  const sanitize = (name: string) => name.toLowerCase().replace(/\s+/g, '-');

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

  async function rowNames(): Promise<string[]> {
    const rows = await $$('[data-testid^="emulator-row-"] .name');
    const names: string[] = [];
    for (const el of rows) {
      names.push(await el.getText());
    }
    return names;
  }

  /**
   * Sets a native `<select>`'s value and dispatches `change` directly. A
   * WebDriver-protocol click on the underlying `<option>` (what
   * `selectByAttribute` does) does not reliably fire a `change` event on
   * this embedded WebKitGTK driver — confirmed by config.toml never picking
   * up the new default when this test used `selectByAttribute` instead.
   */
  async function selectValue(testIdName: string, value: string) {
    await browser.execute(
      (selector, val) => {
        const el = document.querySelector(selector) as HTMLSelectElement | null;
        if (!el) throw new Error(`no element matched ${selector}`);
        el.value = val;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      testId(testIdName),
      value,
    );
  }

  /** Waits until config.toml contains `line`, or fails with a useful message. */
  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(configPath(), 'utf-8').includes(line);
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `config.toml never contained ${line}`,
      },
    );
  }

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

  it('Ctrl+F brings the catalog pane forward and focuses its search', async () => {
    // The search input renders only under the Catalog tab, and `addTab`
    // survives rail clicks, so the chord has to reset the tab as well as the
    // page (final review P5-3).
    await showPage('catalog');
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-catalog-search')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the Manual tab never replaced the catalog search',
    });

    await showPage('installed');
    await browser.keys(['Control', 'f']);

    await $(testId('emu-page-catalog')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Ctrl+F never brought the catalog pane forward',
    });
    await $(testId('emu-catalog-search')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Ctrl+F left the Manual tab up, so there was no search box',
    });
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.activeElement?.getAttribute('data-testid') ?? '')) ===
        'emu-catalog-search',
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the catalog search never took focus' },
    );

    await showPage('installed');
  });

  it('auto-fills name and args from a profile-matching path, then saves the row', async () => {
    await $(testId('emulator-add')).click();
    // Add now opens on the Install tab (task-7-brief.md); the manual form
    // this spec drives lives under the Manual tab.
    // `emulator-add` opens the Add from catalog pane on its Catalog tab.
    await $(testId('emu-page-catalog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('emu-form-name'))).toHaveValue('');
    await expect($(testId('emu-form-args'))).toHaveValue('');

    await $(testId('emu-form-path')).setValue(stubPath);
    // Blur the path field (autoFillFromPath runs onblur) by moving focus to
    // the name field.
    await $(testId('emu-form-name')).click();

    await browser.waitUntil(async () => (await $(testId('emu-form-name')).getValue()) !== '', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the name field never auto-filled from the matched profile',
    });
    await expect($(testId('emu-form-name'))).toHaveValue('RetroArch (Multi-System)');
    await expect($(testId('emu-form-args'))).toHaveValue('-L "%core%" "%rom%"');
    await expect($(testId('emu-form-args-label'))).toHaveText(
      'Arguments (%rom%, %core%, %ps3_launch_target%)',
    );

    // The five per-emulator cloud fields (parity gap 1), read back from
    // config.toml and the edit sheet by the case below.
    await selectValue('emu-form-save-strategy', 'folder');
    await $(testId('emu-form-ignore-files')).setValue('skip.bin;other.bin');
    await $(testId('emu-form-ignore-extensions')).setValue('.tmp;.log');
    await $(testId('emu-form-save-paths')).setValue('saves');
    await $(testId('emu-form-state-paths')).setValue('states');

    await $(testId('emu-form-save')).click();
    // The global toast surface (parity gap 5) with the reference's text
    // (emulator_ui_mixin.py:1591). Asserted before the row wait so it is
    // read well inside TOAST_DURATION_MS.
    await $(testId('toast')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared after adding an emulator',
    });
    await expect($(testId('toast'))).toHaveText("Added emulator 'RetroArch (Multi-System)'.");
    await $(testId(`emulator-row-${sanitize('RetroArch (Multi-System)')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the saved emulator never appeared in the list',
    });
    // A manual save lands on Installed, where the new row is.
    await expect($(testId('emu-page-installed'))).toBeDisplayed();
    await expect($(testId('emu-nav-count-installed'))).toHaveText('1');
  });

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

  it('adds a second emulator and keeps row order when editing the first', async () => {
    await $(testId('emulator-add')).click();
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Name typed first so autoFillFromPath's "both blank" guard skips it —
    // this path matches no profile anyway.
    await $(testId('emu-form-name')).setValue('AAA Manual Emulator');
    await $(testId('emu-form-path')).setValue('/nonexistent/aaa-emulator');
    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('AAA Manual Emulator')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the second emulator never appeared in the list',
    });

    expect(await rowNames()).toEqual(['RetroArch (Multi-System)', 'AAA Manual Emulator']);

    // Edit the FIRST entry (rename it) and confirm it keeps its position —
    // apply_save_emulator (commands.rs) re-inserts an edited entry at its
    // original index rather than appending it.
    await $(testId(`emulator-edit-${sanitize('RetroArch (Multi-System)')}`)).click();
    // Design §9: Edit opens the form as a sheet beside the list.
    await $(testId('emu-edit-sheet')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the edit sheet never opened',
    });
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('emu-form-name'))).toHaveValue('RetroArch (Multi-System)');
    await $(testId('emu-form-name')).setValue('RetroArch Renamed');
    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('RetroArch Renamed')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the renamed emulator never appeared under its new name',
    });
    await $(testId('emu-edit-sheet')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the edit sheet stayed open after a successful save',
    });

    expect(await rowNames()).toEqual(['RetroArch Renamed', 'AAA Manual Emulator']);
  });

  it('rejects saving a duplicate name with the verbatim error', async () => {
    await $(testId('emulator-add')).click();
    await $(testId('emu-add-tab-manual')).click();
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('emu-form-name')).setValue('AAA Manual Emulator');
    await $(testId('emu-form-path')).setValue('/nonexistent/dup');
    await $(testId('emu-form-save')).click();

    await expect($(testId('emu-form-error'))).toHaveText("An emulator named 'AAA Manual Emulator' already exists.");
    await $(testId('emu-form-cancel')).click();
  });

  it('deletes an emulator with a two-click confirm', async () => {
    await showPage('installed');
    const deleteBtn = $(testId(`emulator-delete-${sanitize('AAA Manual Emulator')}`));
    await deleteBtn.click();
    await expect(deleteBtn).toHaveText('Confirm delete');
    await expect($(testId(`emulator-row-${sanitize('AAA Manual Emulator')}`))).toExist();

    await deleteBtn.click();
    await $(testId(`emulator-row-${sanitize('AAA Manual Emulator')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the deleted emulator row was still there after the second click',
    });
  });

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

  /** The `<option>` values of a select, in DOM order. */
  async function optionValues(testIdName: string): Promise<string[]> {
    return browser.execute((selector) => {
      const el = document.querySelector(selector) as HTMLSelectElement | null;
      if (!el) throw new Error(`no element matched ${selector}`);
      return Array.from(el.options).map((o) => o.value);
    }, testId(testIdName));
  }

  it('assigns a per-platform default and records a core in config.toml', async () => {
    await showPage('defaults');
    await selectValue('default-select-1', 'RetroArch Renamed');
    await waitForConfigLine('"Super Nintendo Entertainment System" = "RetroArch Renamed"');
    // D-RC-4: picking RetroArch also records the first installed compatible
    // core, which the slug map orders snes9x before bsnes.
    await waitForConfigLine('"Super Nintendo Entertainment System" = "snes9x"');
  });

  it('lists the installed cores for the RetroArch row, in slug-map order', async () => {
    await $(testId('default-core-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the core select never appeared for the RetroArch default',
    });
    await expect(await optionValues('default-core-1')).toEqual(['snes9x', 'bsnes']);
  });

  it('changing the core rewrites the retroarch_cores line', async () => {
    await selectValue('default-core-1', 'bsnes');
    await waitForConfigLine('"Super Nintendo Entertainment System" = "bsnes"');
  });

  it('leaving RetroArch for none records the <none> marker and clears the saved core', async () => {
    await selectValue('default-select-1', '');
    await browser.waitUntil(
      () => {
        try {
          const contents = readFileSync(configPath(), 'utf-8');
          return (
            contents.includes('"Super Nintendo Entertainment System" = "<none>"') &&
            !contents.includes('"Super Nintendo Entertainment System" = "bsnes"') &&
            !contents.includes('"Super Nintendo Entertainment System" = "RetroArch Renamed"')
          );
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg:
          'config.toml did not hold the <none> marker, or still had a retroarch_cores line for the platform, after clearing the default to none',
      },
    );
  });

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
    await showPage('defaults');
    await $(testId('default-select-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the per-platform defaults list never rendered after re-entering',
    });

    await expect($(testId('default-select-1'))).toHaveValue('');
    await expect($(testId('default-core-1'))).not.toExist();
    expect(readFileSync(configPath(), 'utf-8')).toContain(
      '"Super Nintendo Entertainment System" = "<none>"',
    );
  });

  it('does not offer RetroArch for a platform with no installed core', async () => {
    // Arcade (platform 2) needs fbneo/mame2003_plus/mame; only SNES cores
    // are installed, so D-RC-1's gate keeps RetroArch out of the list even
    // though its autoprofile sets all_platforms: true.
    const names = await optionValues('default-select-2');
    expect(names).not.toContain('RetroArch Renamed');
  });
});
