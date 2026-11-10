// e2e-app-args:
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, FIXTURE_TOKEN, forgeUrl, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { assertNoClipping, connectPad, focused, hints, invoke, key, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, text, typeKeys, waitGone, waitOskClosed, waitText } from '../helpers/tvDetails.js';

/** One catalog install from the mock forge (slowed for the Installing label). */
const INSTALL_WAIT = 90_000;

const PS2 = 'Sony PlayStation 2';
const DC = 'Sega Dreamcast';
const PS4 = 'Sony PlayStation 4';
const PCSX2 = 'PCSX2 (Playstation 2)';

const config = () => readFileSync(configPath(), 'utf-8');

async function waitConfig(check: (c: string) => boolean, timeoutMsg: string, timeout = INSTALL_WAIT): Promise<void> {
  await browser.waitUntil(() => check(config()), { timeout, timeoutMsg });
}

/** A settings row's words: its label, the line under it, its value, spaced. */
async function rowText(id: string): Promise<string> {
  return (await browser.execute((sel: string) => {
    const r = document.querySelector(sel);
    if (!r) return '';
    return ['.t', '.s', '.val']
      .map((c) => (r.querySelector(c)?.textContent ?? '').replace(/\s+/g, ' ').trim())
      .filter((t) => t !== '')
      .join(' ');
  }, testId(id))) as string;
}

async function waitRow(id: string, want: string | RegExp): Promise<void> {
  let last = '';
  await browser
    .waitUntil(
      async () => {
        last = await rowText(id);
        return typeof want === 'string' ? last === want : want.test(last);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${id} never read ${String(want)}` },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (it reads "${last}")`);
    });
}

/** The pane's system rows, top to bottom (their test keys). */
async function rowOrder(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-sys-list"] > [data-testid^="tv-sys-row-"]')).map((r) =>
      (r as HTMLElement).dataset.testid!.replace('tv-sys-row-', ''),
    ),
  )) as string[];
}

async function statusOf(key: string): Promise<string> {
  return (await $(testId(`tv-sys-status-${key}`)).getAttribute('data-status')) ?? '';
}

async function waitStatus(key: string, status: string, timeout = INSTALL_WAIT): Promise<void> {
  await browser.waitUntil(async () => (await statusOf(key)) === status, {
    timeout,
    timeoutMsg: `${key} never read ${status} (it reads ${await statusOf(key)})`,
  });
}

/** The open list: its title and options (group headings in brackets). */
async function list(): Promise<{ title: string; items: string[] }> {
  return (await browser.execute(() => {
    const sheet = document.querySelector('[data-testid="tv-list"]');
    const clean = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
    return {
      title: clean(sheet?.querySelector('h2')?.textContent),
      items: Array.from(sheet?.querySelectorAll('.sgroup, .opt') ?? []).map((o) => (o.classList.contains('sgroup') ? `[${clean(o.textContent)}]` : clean(o.textContent))),
    };
  })) as { title: string; items: string[] };
}

async function waitList(title: string): Promise<void> {
  await browser.waitUntil(async () => (await list()).title === title, { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no list titled ${title} (${JSON.stringify(await list())})` });
}

/** Focus the list option whose text starts with `label`, then A. */
async function pickOption(label: string): Promise<void> {
  const k = (await browser.execute(
    (l: string) =>
      Array.from(document.querySelectorAll('[data-testid="tv-list"] .opt'))
        .find((o) => (o.textContent ?? '').replace(/\s+/g, ' ').trim().startsWith(l))
        ?.getAttribute('data-k') ?? '',
    label,
  )) as string;
  expect(k).not.toBe('');
  await seek(k);
  await key('Enter');
  await waitGone('tv-list');
}

/** Moves focus to `k`: down first, then up, then right and left. */
async function seek(k: string): Promise<void> {
  for (const dir of ['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft']) {
    for (let i = 0; i < 16 && (await focused()) !== k; i++) {
      const was = await focused();
      await key(dir);
      await browser.pause(80);
      if ((await focused()) === was) break;
    }
    if ((await focused()) === k) break;
  }
  await waitFocused(k);
}

/** Confirms the open dialog: Cancel has focus, Right reaches the action. */
async function confirm(title: string, body: string): Promise<void> {
  await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  expect((await $(`${testId('tv-confirm')} h2`).getText()).trim()).toBe(title);
  expect((await $(`${testId('tv-confirm')} p`).getText()).trim()).toBe(body);
  await waitFocused('dlg-cancel');
  await key('ArrowRight');
  await waitFocused('dlg-ok');
  await key('Enter');
  await waitGone('tv-confirm');
}

/** A page's row texts by key (`tv-row-<key>`). */
async function pageRows(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-system-page"] [data-testid^="tv-row-"]')).map((r) => (r as HTMLElement).dataset.testid!.replace('tv-row-', '')),
  )) as string[];
}

/** The folder browser: up to the root, then down into `target`, clicking
 *  rows (a click focuses and presses A). */
async function browseTo(target: string): Promise<void> {
  for (let i = 0; i < 30; i++) {
    const at = await text('tv-fs-path');
    if (target === at || target.startsWith(at.endsWith('/') ? at : `${at}/`)) break;
    await $('#fs-list [data-k="up"]').click();
    await browser.waitUntil(async () => (await text('tv-fs-path')) !== at, { timeout: TRANSITION_TIMEOUT });
  }
  for (let i = 0; i < 30; i++) {
    const at = await text('tv-fs-path');
    if (at === target) return;
    const rest = target.slice(at.length).replace(/^\/+/, '');
    const seg = rest.split('/')[0];
    await $(`#fs-list [data-k="fs-${seg}"]`).click();
    await browser.waitUntil(async () => (await text('tv-fs-path')) !== at, { timeout: TRANSITION_TIMEOUT });
  }
  await waitText('tv-fs-path', target);
}

/**
 * Stage `tv-systems` (docs/tv-mode/plan.md P11): Settings › Systems in TV
 * mode on the systems fixtures (PS2, Dreamcast and PS4, whose emulators the
 * mock forge serves; SNES, which only RetroArch serves and the forge
 * cannot; Zeebo, which nothing installable serves) with the mock forge.
 * Connects on the desktop, enters TV mode, and works through the pane, a
 * System page, a Download pick, the update check, Remove, Add manually
 * through the file-mode browser and Install recommended (N).
 */
describe('tv-systems: Settings › Systems, the System page and the emulator form', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(path.join(dataDir(), 'library'));
    await $(testId('connect-submit')).click();
    await $(testId('tv-mode-button')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the desktop shell never loaded' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await connectPad(XBOX_PAD);
    await key('e');
    await waitTab('downloads');
    await key('e');
    await waitTab('settings');
    await waitFocused('r-appearance');
    await moveTo('r-systems', 'ArrowDown', 10);
    await waitText('tv-pane-title', 'Systems');
  });

  it('lists no emulator first, then the rest, then Windows, with pills, and the header buttons (items 252, 255–256, 258)', async () => {
    await browser.waitUntil(async () => (await rowOrder()).length === 6, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the system rows never appeared' });
    expect(await rowOrder()).toEqual(['dc', 'ps2', 'ps4', 'snes', 'zeebo', 'windows']);
    for (const k of ['dc', 'ps2', 'ps4', 'snes', 'zeebo']) {
      expect(await statusOf(k)).toBe('none');
      await waitText(`tv-sys-status-${k}`, 'No emulator');
    }
    await waitText('tv-sys-sub-dc', "Games for this system can't start yet. Redream is recommended.");
    await waitText('tv-sys-sub-zeebo', 'No emulator GRID installs plays this system. If you have one, use Add an emulator manually.');
    await waitText('tv-sys-rec', 'Install recommended (4)');
    await waitText('tv-sys-check', 'Check for updates');
    await waitText('tv-sys-add', 'Add manually');
    await waitText('tv-sys-other-empty', 'Every installed emulator is used by a system.');
  });

  it('A opens a system, X installs its recommended emulator or changes it (item 257)', async () => {
    await key('ArrowRight');
    await waitFocused('sy-rec');
    await key('ArrowDown');
    await waitFocused('sy-dc');
    expect(await hints()).toEqual(expect.arrayContaining(['A Open', 'X Install Redream']));
    await assertNoClipping();
    await seek('sy-windows');
    expect(await hints()).toEqual(expect.arrayContaining(['A Open', 'X Change emulator']));
  });

  it('Windows: the Proton version, the umu-run line and the first-game note (item 336)', async () => {
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-crumb', /Windows/);
    await browser.waitUntil(async () => (await pageRows()).includes('umu'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no umu-run line' });
    expect(await pageRows()).toEqual(expect.arrayContaining(['proton', 'umu', 'first']));
    expect(await rowText('tv-row-proton')).toContain('Runs Windows games on Linux');
    expect(await rowText('tv-row-first')).toBe('First Windows game Downloads the Steam Linux Runtime (about 200 MB)');
    await waitFocused('s-proton');
    await key('Enter');
    await waitList('Default Proton version');
    await browser.waitUntil(async () => (await list()).items.includes('[Download]'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Download group' });
    expect((await list()).items).toContain('GE-Proton (latest, recommended)');
    await key('Escape');
    await waitGone('tv-list');
    await key('Escape');
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('sy-windows');
  });

  it('a system with no emulator: Install <recommended> and Choose another; a Download pick installs it (items 326–328, 335, 260)', async () => {
    await seek('sy-ps2');
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-crumb', new RegExp(PS2));
    expect(await text('tv-system-page')).toContain('No emulator');
    expect(await pageRows()).toEqual(['install', 'choose']);
    await waitRow('tv-row-install', 'Install PCSX2 (recommended)');
    await waitFocused('s-install');
    await key('ArrowDown');
    await waitFocused('s-choose');
    await key('Enter');
    await waitList(`Emulator for ${PS2}`);
    const items = (await list()).items;
    expect(items[0]).toBe('[Download]');
    expect(items[1]).toBe('PCSX2 (recommended)');
    expect(items.slice(-2)).toEqual(['[]', 'None']);
    // None is checked: the list opens on it.
    await waitFocused(`o${items.length - 1}`);
    await pickOption('PCSX2 (recommended)');
    await waitToast(/^Installing PCSX2.*…$/);
    await waitConfig((c) => c.includes(`"${PS2}" = "${PCSX2}"`), 'PCSX2 never became PS2’s emulator');
    await waitToast(`${PCSX2} installed`);
    await browser.waitUntil(async () => (await pageRows()).includes('emu'), { timeout: INSTALL_WAIT, timeoutMsg: 'the page never showed the emulator' });
    await waitRow('tv-row-emu', `Emulator ${PCSX2}`);
    expect(await text('tv-system-page')).toContain(`${PCSX2} · Ready`);
    expect(await pageRows()).toEqual(expect.arrayContaining(['emu', 'ver', 'open', 'settings', 'remove']));
    expect(await rowText('tv-row-open')).toBe(`Open ${PCSX2} Opens the emulator on its own to change its settings. Its window needs a mouse.`);
    expect(await rowText('tv-row-settings')).toBe('Emulator settings Program, launch options, cloud save folders');
    // The emulator list now has it under Installed, checked.
    await seek('s-emu');
    await key('Enter');
    await waitList(`Emulator for ${PS2}`);
    expect((await list()).items.slice(0, 2)).toEqual(['[Installed]', `${PCSX2} (recommended)`]);
    await waitFocused('o1');
    await key('Escape');
    await waitGone('tv-list');
  });

  it('Check for updates says how many it checks, then what it found; the page offers Update to (items 259, 329)', async () => {
    await key('Escape');
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('sy-ps2');
    await waitStatus('ps2', 'ready', TRANSITION_TIMEOUT);
    await fetch(`${forgeUrl()}/__e2e__/pcsx2-release/updated`, { method: 'POST' });
    await seek('sy-chk');
    await key('Enter');
    await waitToast('Checking 1 emulator for updates…');
    await waitToast('1 update available.');
    await waitStatus('ps2', 'update', TRANSITION_TIMEOUT);
    await waitText('tv-sys-status-ps2', 'Update available');
    await seek('sy-ps2');
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await pageRows()).includes('update'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Update to row' });
    expect(await rowText('tv-row-update')).toMatch(/^Update to \S+/);
  });

  it('Remove asks, notifies and goes back (item 333)', async () => {
    await seek('s-remove');
    await key('Enter');
    await confirm(`Remove ${PCSX2}?`, `Games and saves stay. ${PS2} games can't start until another emulator is picked.`);
    await waitToast(`${PCSX2} removed`);
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Remove did not go back' });
    await waitGone('tv-system-page');
    await waitConfig((c) => !c.includes(`name = "${PCSX2}"`), 'PCSX2 was never removed', TRANSITION_TIMEOUT * 2);
    await waitStatus('ps2', 'none', TRANSITION_TIMEOUT);
  });

  it('None from the emulator list stores the explicit none', async () => {
    // Dreamcast and PS4 by command, PS2 through its page: a hand-added
    // emulator with no profile plays every system, so only these three keep
    // no emulator once it is added.
    await invoke('set_default_emulator', { platform: DC, name: '' });
    await invoke('set_default_emulator', { platform: PS4, name: '' });
    await seek('sy-ps2');
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await seek('s-choose');
    await key('Enter');
    await waitList(`Emulator for ${PS2}`);
    await pickOption('None');
    await waitConfig((c) => c.includes(`"${PS2}" = "<none>"`), 'None never reached config.toml', TRANSITION_TIMEOUT);
    await key('Escape');
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('Add manually: the form, Program through the file-mode browser, Add emulator goes back (items 337–341)', async () => {
    const program = path.join(dataDir(), 'stub-emu');
    writeFileSync(program, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    await seek('sy-add');
    await key('Enter');
    await $(testId('tv-emu-form')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-emu-form')).toContain('Add an emulator manually');
    expect(await text('tv-emu-form')).toContain("For an emulator GRID doesn't install. It never becomes a system's default by itself.");
    await waitRow('tv-row-which', 'Which emulator is it? Something else');
    await waitRow('tv-row-name', 'Name Not set');
    await waitRow('tv-row-prog', "Program Browse for the emulator's program Not set");
    await waitRow('tv-row-args', 'Launch options %rom% is replaced by the game file %rom%');
    await waitRow('tv-row-strategy', 'Cloud saves (advanced) Save and state folders, files not to upload Save strategy: Automatic');
    // "Which emulator is it?" lists the known emulators and Something else.
    await waitFocused('s-which');
    await key('Enter');
    await waitList('Which emulator is it?');
    expect((await list()).items.slice(-1)).toEqual(['Something else']);
    await key('Escape');
    await waitGone('tv-list');
    // Name, by the keyboard.
    await seek('s-name');
    await key('Enter');
    await typeKeys('Stub Emu');
    await key('Enter');
    await waitOskClosed();
    await waitRow('tv-row-name', 'Name Stub Emu');
    // Program, through the folder browser in file mode.
    await seek('s-prog');
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-crumb', /Choose the program/);
    expect(await hints()).not.toContain('Y Use this folder');
    await browseTo(dataDir());
    await moveTo('fs-stub-emu', 'ArrowDown', 80);
    expect(await hints()).toContain('A Choose');
    await key('Enter');
    await $(testId('tv-emu-form')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitRow('tv-row-prog', `Program ${program} stub-emu`);
    // What was typed survived the browser.
    await waitRow('tv-row-name', 'Name Stub Emu');
    // The save strategy list.
    await seek('s-strategy');
    await key('Enter');
    await waitList('How does it store saves?');
    expect((await list()).items).toEqual(['Automatic', 'One file per game', 'A folder per game']);
    await pickOption('One file per game');
    await waitRow('tv-row-strategy', /Save strategy: One file per game$/);
    await seek('s-save');
    await waitRow('tv-row-save', 'Add emulator');
    await key('Enter');
    await waitToast("Added emulator 'Stub Emu'.");
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Add emulator did not go back' });
    await waitConfig((c) => c.includes('name = "Stub Emu"') && c.includes('save_strategy = "single_file"'), 'the manual entry never reached config.toml', TRANSITION_TIMEOUT);
    // A manual add is nobody's default.
    expect(config().match(/\[default_emulators\][^[]*/)?.[0] ?? '').not.toContain('Stub Emu');
  });

  it('Install recommended reads "Installing N emulators…", can’t be pressed again, and focus moves to the control in its place (items 253–254)', async () => {
    await waitText('tv-sys-rec', 'Install recommended (3)');
    await fetch(`${forgeUrl()}/__e2e__/slow?ms=4000`, { method: 'POST' });
    try {
      await seek('sy-rec');
      await key('Enter');
      await waitText('tv-sys-rec', /^Installing [1-3] emulators?…$/);
      expect(await $(testId('tv-sys-rec')).getAttribute('aria-disabled')).toBe('true');
      expect(await focused()).toBe('sy-rec');
      // A again says why instead.
      await key('Enter');
      await waitToast('Those emulators are installing. Downloads shows how far they are.');
      await browser.waitUntil(async () => !(await $(testId('tv-sys-rec')).isExisting()), {
        timeout: INSTALL_WAIT * 2,
        timeoutMsg: 'Install recommended never went away',
      });
    } finally {
      await fetch(`${forgeUrl()}/__e2e__/slow?ms=0`, { method: 'POST' });
    }
    await waitFocused('sy-chk', 'focus did not move to Check for updates');
    for (const k of ['dc', 'ps2', 'ps4']) expect(await statusOf(k)).not.toBe('none');
    await assertNoClipping();
  });

  it('a Server header chip opens the system page (item 200)', async () => {
    await key('q');
    await waitTab('downloads');
    await key('q');
    await waitTab('server');
    await browser.waitUntil(async () => (await focused()) !== null, { timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await waitFocused('r-discover');
    await moveTo('r-1', 'ArrowDown', 6);
    await key('ArrowRight');
    await browser.waitUntil(async () => ((await focused()) ?? '').startsWith('sg'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the grid never took focus' });
    await $(testId('tv-srv-emu')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the header never showed its emulator chip' });
    // Up from the top row reaches the header (tv-server walks it with the
    // D-pad); a click on the chip focuses it and presses A.
    await key('ArrowUp');
    await browser.waitUntil(async () => (await browser.execute(() => !!document.querySelector('[data-testid="tv-srv-header"] .f'))) as boolean, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Up never reached the header',
    });
    expect(await $(testId('tv-srv-emu')).getAttribute('data-al')).toBe('Open in Systems');
    await $(testId('tv-srv-emu')).click();
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chip never opened the system page' });
    await browser.waitUntil(async () => (await pageRows()).includes('emu'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the page never showed PS2’s emulator' });
    await waitRow('tv-row-emu', `Emulator ${PCSX2}`);
    await key('Escape');
    await waitGone('tv-system-page');
  });

  it('had no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
