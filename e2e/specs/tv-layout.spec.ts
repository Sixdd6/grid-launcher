import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { mountOf, plugIn } from '../helpers/drives.js';
import {
  assertBands,
  assertMinText,
  assertNoClipping,
  assertNoDocumentScroll,
  connectPad,
  focused,
  invoke,
  key,
  waitTab,
  XBOX_PAD,
} from '../helpers/tv.js';
import { captureErrors, libCover, pageErrors, testId, text, typeKeys, waitGone, waitOskClosed } from '../helpers/tvDetails.js';
import { byK, driveRow, tap, tapK } from '../helpers/tvBackups.js';

/**
 * Stage `tv-layout` (docs/tv-mode/plan.md P14; checklist items 1, 3, 4, 5,
 * 7, 26, 113, 114; plan risk R7): the layout sweep. A fresh --tv start on
 * the `backups` fixtures (seed: backups-seed.mjs `tv-layout`, which is
 * backups-d's profile: Cinder Peaks, Tux Trail and Moss and Marrow
 * installed, TESTDRIVE set up for Linux, PCSX2 from the server), connected
 * through TV mode's own Connect, then Choose your systems, and every tab,
 * page and overlay after it.
 *
 * On each screen, at each window size (E2E_WINDOW, else 1280x720, 1920x1080
 * and 3840x2160), it checks:
 * - the window never scrolls (item 4);
 * - the top bar, stage and hint bar keep their rem heights (item 5);
 * - no text is under 0.85rem (item 3, ruling 2) — `assertMinText`;
 * - the focused item is cut by no clipping ancestor after every move
 *   (R7, items 26, 113, 114) — `assertNoClipping`, the prototype's checker.
 * Failures are collected, so one run lists every one; the last test fails
 * when there were any. With E2E_TV_SCREENS=1 each screen is saved under
 * ctr-out/tv-screens/<size>/<name>.png.
 */

const SIZES: [number, number][] = (process.env.E2E_WINDOW ?? '1280x720 1920x1080 3840x2160')
  .split(/[ ,]+/)
  .filter((s) => s !== '')
  .map((s) => s.split('x').map(Number) as [number, number]);
const SHOTS = process.env.E2E_TV_SCREENS === '1' ? path.resolve(process.cwd(), '..', 'ctr-out', 'tv-screens') : '';
const DRIVE = 'TESTDRIVE';
const NEW_DRIVE = 'NEWDRIVE';

const failures: string[] = [];
const OPPOSITE: Record<string, string> = { ArrowUp: 'ArrowDown', ArrowDown: 'ArrowUp', ArrowLeft: 'ArrowRight', ArrowRight: 'ArrowLeft' };

async function sizeTo(w: number, h: number): Promise<void> {
  await browser.setWindowSize(w, h);
  await browser
    .waitUntil(
      async () => {
        const v = (await browser.execute(() => [window.innerWidth, window.innerHeight])) as number[];
        return v[0] === w && v[1] === h;
      },
      { timeout: TRANSITION_TIMEOUT, interval: 100 },
    )
    .catch(async () => {
      const v = (await browser.execute(() => [window.innerWidth, window.innerHeight])) as number[];
      throw new Error(`the window never became ${w}x${h} (it is ${v.join('x')})`);
    });
  // Layout, the reveal and any glide settle.
  await browser.pause(350);
}

async function record(where: string, check: () => Promise<void>): Promise<void> {
  try {
    await check();
  } catch (e) {
    failures.push(`${where}: ${(e as Error).message}`);
  }
}

/**
 * Checks the screen on show at every size: the static rules, then each of
 * `moves` (a key per move) with the clipping check after it, then the same
 * moves back, so the screen ends where it started.
 */
async function sweep(name: string, moves: string[] = []): Promise<void> {
  for (const [w, h] of SIZES) {
    const at = `${name} @${w}x${h}`;
    await sizeTo(w, h);
    await record(at, assertNoDocumentScroll);
    await record(at, assertBands);
    await record(at, () => assertMinText(0.85));
    if ((await focused()) !== null) await record(`${at} [${await focused()}]`, assertNoClipping);
    if (SHOTS) {
      const dir = path.join(SHOTS, `${w}x${h}`);
      mkdirSync(dir, { recursive: true });
      await browser.saveScreenshot(path.join(dir, `${name}.png`));
    }
    for (const k of [...moves, ...[...moves].reverse().map((m) => OPPOSITE[m] ?? m)]) {
      await key(k);
      await browser.pause(90);
      await record(`${at} after ${k} [${await focused()}]`, assertNoClipping);
    }
    // Text a move revealed (a deeper row, a tab body) is checked too.
    await record(`${at} after moves`, () => assertMinText(0.85));
    await record(`${at} after moves`, assertNoDocumentScroll);
  }
}

const repeat = (k: string, n: number) => Array.from({ length: n }, () => k);

async function waitShown(id: string, what = id): Promise<void> {
  await $(testId(id)).waitForExist({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: `${what} never showed` });
  await browser.pause(150);
}

/** B until `id` is gone. */
async function back(id: string): Promise<void> {
  await key('Escape');
  await waitGone(id);
  await browser.pause(150);
}

describe('tv-layout: every screen fits at 720p, 1080p and 4K', () => {
  before(async () => {
    await $(testId('tv-connect')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed Connect' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  after(async () => {
    await browser.setWindowSize(1920, 1080).catch(() => {});
  });

  it('Connect', async () => {
    await sweep('connect', repeat('ArrowDown', 4));
    // Fill it in by pasting (View): the server, then the token.
    await invoke('e2e_clipboard_write', { text: mockUrl() });
    await tapK('cf-url');
    await waitShown('tv-osk', 'the keyboard');
    await sweep('connect-keyboard', ['ArrowDown', 'ArrowRight', 'ArrowRight']);
    await key('Escape');
    await waitOskClosed();
    await key('v');
    await browser.waitUntil(async () => (await text('tv-cf-url-value')) === mockUrl(), { timeout: TRANSITION_TIMEOUT });
    await tapK('cf-how');
    await waitShown('tv-list', 'Sign in with');
    await sweep('connect-list');
    await back('tv-list');
    await invoke('e2e_clipboard_write', { text: FIXTURE_TOKEN });
    await tapK('cf-token');
    await waitShown('tv-osk', 'the keyboard');
    await key('Escape');
    await waitOskClosed();
    await key('v');
    if ((await text('tv-cf-lib-value')).trim() === '') {
      await invoke('e2e_clipboard_write', { text: path.join(dataDir(), 'library') });
      await tapK('cf-lib');
      await waitShown('tv-folder-browser', 'the folder browser');
      await sweep('folder-browser', repeat('ArrowDown', 3));
      await back('tv-folder-browser');
      await key('v');
    }
    // Connect by its button: Start on Connect opens the startup Menu (user
    // decision 2026-11-10).
    await $(testId('tv-connect-go')).click();
  });

  it('Choose your systems (when this profile is offered it)', async () => {
    // A profile whose systems all have an emulator goes straight to the shell.
    await browser.waitUntil(async () => (await $(testId('tv-choose')).isExisting()) || (await $(testId('tv-discover')).isExisting()), {
      timeout: TRANSITION_TIMEOUT * 3,
      timeoutMsg: 'neither Choose your systems nor Discover showed after Connect',
    });
    if (!(await $(testId('tv-choose')).isExisting())) return;
    await browser.waitUntil(async () => (await focused())?.startsWith('ch-') ?? false, { timeout: TRANSITION_TIMEOUT * 2 });
    await sweep('choose', repeat('ArrowDown', 6));
    await key('x');
    if (await $(testId('tv-list')).isExisting()) {
      await sweep('choose-emulator-list', repeat('ArrowDown', 2));
      await back('tv-list');
    }
    await key('m');
    await waitShown('tv-shell');
    await waitTab('server');
  });

  it('Server › Discover, Search everything, the Menu and its pages', async () => {
    await waitShown('tv-discover', 'Discover');
    await sweep('discover', ['ArrowDown', 'ArrowRight', 'ArrowRight', 'ArrowDown', 'ArrowRight']);
    await key('y');
    await waitShown('tv-osk', 'the search keyboard');
    await sweep('search-keyboard', ['ArrowDown', 'ArrowDown', 'ArrowRight']);
    await typeKeys('a');
    await key('Enter');
    await waitOskClosed();
    await waitShown('tv-search-all', 'Search everything');
    await browser.pause(800);
    await sweep('search-results', ['ArrowRight', 'ArrowDown']);
    await back('tv-search-all');
    await key('m');
    await waitShown('tv-menu', 'the Menu');
    await sweep('menu', repeat('ArrowDown', 6));
    await tap(testId('tv-menu-notifications'));
    await waitShown('tv-notifications', 'Notifications');
    await sweep('notifications', repeat('ArrowDown', 3));
    await back('tv-notifications');
    await key('m');
    await waitShown('tv-menu', 'the Menu');
    await tap(testId('tv-menu-quit'));
    await waitShown('tv-confirm', 'Quit GRID?');
    await sweep('confirm-quit', ['ArrowRight']);
    await back('tv-confirm');
  });

  it('Server › a platform, its search and select mode', async () => {
    await tapK('r-1');
    await browser.waitUntil(async () => (await $$('#srv-grid [data-cover]').length) > 0, { timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'the platform grid never filled' });
    await browser.pause(300);
    await sweep('server-platform', ['ArrowRight', 'ArrowRight', 'ArrowDown', 'ArrowRight', 'ArrowUp', 'ArrowUp']);
    await key('y');
    await waitShown('tv-osk', 'the platform search');
    await sweep('platform-search-keyboard');
    await key('Escape');
    await waitOskClosed();
    // Select mode: its bar pinned under the grid.
    await tapK('sg901');
    if (await $(testId('tv-details')).isExisting()) await back('tv-details');
    await key('r');
    await waitShown('tv-selbar', 'the select bar');
    await key('Enter');
    await sweep('server-select-mode', ['ArrowRight', 'ArrowDown']);
    await key('r');
    await waitGone('tv-selbar');
  });

  it('Details of a server game, then Downloads with a live install', async () => {
    // An install, so Downloads and the top bar have a live row.
    await tapK('sg903');
    await waitShown('tv-details', 'Details');
    await browser.pause(500);
    await sweep('details-server-game', ['ArrowRight', 'ArrowDown', 'ArrowDown']);
    const install = await $(byK('db-install'));
    if (await install.isExisting()) await tapK('db-install');
    await browser.pause(800);
    await back('tv-details');
    await tap(testId('tv-tab-downloads'));
    await waitTab('downloads');
    await waitShown('tv-downloads', 'Downloads');
    await sweep('downloads', repeat('ArrowDown', 3));
  });

  it('Library: All games, sort, search, a drive, select mode', async () => {
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    await waitShown('tv-library', 'the Library');
    await tapK('r-all');
    await browser.pause(300);
    await sweep('library', ['ArrowRight', 'ArrowRight', 'ArrowDown']);
    await key('z');
    await waitShown('tv-list', 'Sort');
    await sweep('library-sort', repeat('ArrowDown', 3));
    await back('tv-list');
    await key('y');
    await waitShown('tv-osk', 'the Library search');
    await key('Escape');
    await waitOskClosed();
    await key('r');
    if (await $(testId('tv-selbar')).isExisting()) {
      await sweep('library-select-mode', ['ArrowRight']);
      await key('r');
      await waitGone('tv-selbar');
    }
    await tapK(`r-drive:${mountOf(DRIVE)}`);
    await browser.pause(400);
    await sweep('library-drive', ['ArrowRight']);
  });

  it('Details and its tabs, Uninstall, the Backup page, Game settings', async () => {
    await tapK('r-all');
    await browser.pause(300);
    await tapK(await libCover('Cinder Peaks'));
    await waitShown('tv-details', 'Details');
    await browser.pause(400);
    await sweep('details-overview', ['ArrowRight', 'ArrowRight', 'ArrowDown', 'ArrowDown']);
    for (const tab of ['media', 'saves', 'achievements']) {
      await key('e');
      await browser.pause(400);
      await sweep(`details-${tab}`, ['ArrowDown', 'ArrowDown']);
    }
    await key('q');
    await key('q');
    await key('q');
    await tapK('db-uninstall');
    await waitShown('tv-confirm', 'Uninstall?');
    await sweep('confirm-uninstall', ['ArrowRight']);
    await back('tv-confirm');
    await tapK('db-backup');
    await waitShown('tv-backup-page', 'the Backup page');
    await browser.waitUntil(async () => (await text('tv-rows-main')) !== 'Checking the drive…', { timeout: 15_000 }).catch(() => {});
    await sweep('backup-page', repeat('ArrowDown', 14));
    await back('tv-backup-page');
    await back('tv-details');
    await tapK(await libCover('Tux Trail'));
    await waitShown('tv-details', 'Details for Tux Trail');
    await browser.pause(400);
    await key('e');
    await key('e');
    await browser.pause(400);
    await sweep('details-saves-native', repeat('ArrowDown', 3));
    await key('q');
    await key('q');
    const gs = await $(byK('db-game-settings'));
    if (await gs.isExisting()) {
      await tapK('db-game-settings');
      await waitShown('tv-game-settings', 'Game settings');
      await sweep('game-settings', repeat('ArrowDown', 5));
      await back('tv-game-settings');
    } else {
      failures.push('Tux Trail has no Game settings button');
    }
    await back('tv-details');
  });

  it('Settings: every pane', async () => {
    await tap(testId('tv-tab-settings'));
    await waitTab('settings');
    await waitShown('tv-settings', 'Settings');
    for (const pane of ['appearance', 'backup-drives', 'cloud-saves', 'connection', 'library', 'retroachievements', 'systems', 'tv', 'updates']) {
      await tapK(`r-${pane}`);
      await browser.pause(400);
      // Up and Down only: Left and Right adjust a slider or a choice.
      await sweep(`settings-${pane}`, repeat('ArrowDown', 10));
    }
  });

  it('Settings › Systems: the System page, the emulator form, the Program browser', async () => {
    await tapK('r-systems');
    await browser.waitUntil(async () => (await $$('#sy-list [data-k]').length) > 0, { timeout: TRANSITION_TIMEOUT * 2 }).catch(() => {});
    const first = (await browser.execute(() => document.querySelector('#sy-list [data-k]')?.getAttribute('data-k') ?? null)) as string | null;
    if (first) {
      await tapK(first);
      await waitShown('tv-system-page', 'the System page');
      await sweep('system-page', repeat('ArrowDown', 8));
      await back('tv-system-page');
    } else {
      failures.push('Settings › Systems listed no system');
    }
    await tapK('sy-add');
    await waitShown('tv-emu-form', 'the emulator form');
    await sweep('emulator-form', repeat('ArrowDown', 9));
    const prog = await $(byK('s-prog'));
    if (await prog.isExisting()) {
      await tapK('s-prog');
      await waitShown('tv-folder-browser', 'the Program browser');
      await sweep('program-browser', repeat('ArrowDown', 3));
      await back('tv-folder-browser');
    }
    await back('tv-emu-form');
  });

  it('Settings › Backup drives: the drive page, its computers, Set up for GRID', async () => {
    await tapK('r-backup-drives');
    await $(driveRow(DRIVE)).waitForExist({ timeout: TRANSITION_TIMEOUT * 2 });
    await tap(driveRow(DRIVE));
    await waitShown('tv-drive-page', 'the drive page');
    await sweep('drive-page', repeat('ArrowDown', 10));
    await tapK('dp-os');
    await waitShown('tv-drive-os-page', 'the computers page');
    await sweep('drive-os-page', repeat('ArrowDown', 2));
    await back('tv-drive-os-page');
    await back('tv-drive-page');
    plugIn(NEW_DRIVE, { fs: 'vfat' });
    await $(driveRow(NEW_DRIVE)).waitForExist({ timeout: 10_000, timeoutMsg: `${NEW_DRIVE} never appeared` });
    await tap(driveRow(NEW_DRIVE));
    await waitShown('tv-drive-setup-page', 'Set up for GRID');
    await sweep('drive-setup-page', repeat('ArrowDown', 5));
    await back('tv-drive-setup-page');
  });

  it('Settings › Library and TV mode: the folder browser and a choice list', async () => {
    await tapK('r-library');
    await browser.pause(300);
    await tapK('s-lib-change');
    await waitShown('tv-folder-browser', 'Change library folder');
    await sweep('library-folder-browser', repeat('ArrowDown', 3));
    await back('tv-folder-browser');
    await tapK('r-tv');
    await browser.pause(300);
    await tapK('s-prompts');
    await waitShown('tv-list', 'Button prompts');
    await sweep('prompts-list', repeat('ArrowDown', 4));
    await back('tv-list');
  });

  it('draws the button glyphs (a mask image that loads)', async () => {
    const info = (await browser.execute(async () => {
      const g = document.querySelector<HTMLElement>('[data-testid="tv-hints"] .gli');
      if (!g) return { found: false, mask: '', webkit: '', url: '', loads: false, w: 0 };
      const cs = getComputedStyle(g);
      const url = g.style.getPropertyValue('--g').trim().replace(/^url\(["']?|["']?\)$/g, '');
      const loads = await new Promise<boolean>((res) => {
        const img = new Image();
        img.onload = () => res(img.naturalWidth > 0);
        img.onerror = () => res(false);
        img.src = url;
      });
      return {
        found: true,
        mask: cs.getPropertyValue('mask-image'),
        webkit: cs.getPropertyValue('-webkit-mask-image'),
        url: url.slice(0, 60),
        loads,
        w: g.getBoundingClientRect().width,
      };
    })) as { found: boolean; mask: string; webkit: string; url: string; loads: boolean; w: number };
    console.log(`GLYPH ${JSON.stringify(info)}`);
    const masked = [info.mask, info.webkit].some((m) => m !== '' && m !== 'none');
    if (!info.found || !masked || !info.loads || info.w <= 0) failures.push(`glyphs: ${JSON.stringify(info)}`);
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });

  it('found no clipping, small text, document scroll or band change', async () => {
    if (failures.length > 0) throw new Error(`${failures.length} layout failures:\n${failures.join('\n')}`);
  });
});
