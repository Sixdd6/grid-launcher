import { chmodSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
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
 * None of these flows ever spawn the emulator, so — unlike launch.spec.ts —
 * the stub file below only needs to exist on disk with its executable bit
 * set; `match_profile` (grid-core's `profile_for_entry`) matches purely on
 * the path string, not on file contents.
 */
describe('emulators', () => {
  let stubPath: string;

  before(async () => {
    const stubsDir = path.join(dataDir(), 'stubs');
    mkdirSync(stubsDir, { recursive: true });
    // Basename "retroarch" is a literal `emulator-autoprofiles.json`
    // match_tokens entry (root/emulator-autoprofiles.json), matching the
    // "RetroArch (Multi-System)" profile: name "RetroArch (Multi-System)",
    // args `-L "%core%" "%rom%"`.
    stubPath = path.join(stubsDir, 'retroarch');
    writeFileSync(stubPath, '#!/bin/sh\nexit 0\n');
    chmodSync(stubPath, 0o755);

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

    await $(testId('emulators-open')).click();
    await $(testId('emulators-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the emulators panel never opened',
    });
  });

  /** Row/edit/delete testids sanitize a name the same way Emulators.svelte does. */
  const sanitize = (name: string) => name.toLowerCase().replace(/\s+/g, '-');

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

  it('auto-fills name and args from a profile-matching path, then saves the row', async () => {
    await $(testId('emulator-add')).click();
    // Add now opens on the Install tab (task-7-brief.md); the manual form
    // this spec drives lives under the Manual tab.
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

    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('RetroArch (Multi-System)')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the saved emulator never appeared in the list',
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
    await $(testId('emu-form-name')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('emu-form-name'))).toHaveValue('RetroArch (Multi-System)');
    await $(testId('emu-form-name')).setValue('RetroArch Renamed');
    await $(testId('emu-form-save')).click();
    await $(testId(`emulator-row-${sanitize('RetroArch Renamed')}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the renamed emulator never appeared under its new name',
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

  it('assigns a per-platform default and persists it to config.toml', async () => {
    await selectValue('default-select-1', 'RetroArch Renamed');

    await browser.waitUntil(
      () => {
        try {
          const text = readFileSync(configPath(), 'utf-8');
          return text.includes('"Super Nintendo Entertainment System" = "RetroArch Renamed"');
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'config.toml never got the new default_emulators entry',
      },
    );
  });
});
