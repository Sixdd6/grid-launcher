import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, invoke, key, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { tvOpenRommPlatform } from '../helpers/romm.js';
import { captureErrors, moveTo, openLibraryDetails, pageErrors, testId, text, toAllGames, typeKeys, waitGone, waitOskClosed, waitText } from '../helpers/tvDetails.js';

/** The SNES platform's counts line: RomM's five games, how many are installed, and
 *  the installed games RomM doesn't have (Mystery Cart has no server id). */
const COUNT = /^5 games on RomM · \d installed( · plus \d only on this computer)?$/;

/** The save locations listed, raw over expanded. */
async function locations(): Promise<{ raw: string; expanded: string }[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-saves-loc"]')).map((r) => ({
      raw: r.getAttribute('data-raw') ?? '',
      expanded: (r.querySelector('.s')?.textContent ?? '').trim(),
    })),
  )) as { raw: string; expanded: string }[];
}

/** Moves focus to `k`: down first, then up. */
async function seek(k: string): Promise<void> {
  for (const dir of ['ArrowDown', 'ArrowUp']) {
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

/**
 * Stage `tv-details`, spec c: starts with --tv on the saved session. The
 * Server platform search follows the Library's rule (a blank search is no
 * filter; B restores the search it had); a launch with no emulator
 * offers "Set up <Platform>" on its card, which View opens; an install with
 * no library folder asks for one (checklist 530); a native game's Saves
 * lists its save locations, adds one through the folder browser and removes
 * it, asked first (531).
 */
describe('tv-details (c): search rules, Set up <Platform>, prompts, save locations', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
    await captureErrors();
    await waitTab('library');
    await connectPad(XBOX_PAD);
  });

  it('a blank platform search is no filter, and B restores the search it had (consultant ruling, item 385)', async () => {
    await browser.waitUntil(async () => (await focused()) !== null, { timeout: TRANSITION_TIMEOUT });
    await tvOpenRommPlatform('snes');
    await waitText('tv-lib-count', COUNT);
    await key('y');
    await typeKeys('save');
    await key('Enter');
    await waitOskClosed();
    await waitText('tv-lib-search', 'Search: save');
    await waitText('tv-lib-count', /^3 of \d+ games match$/);
    await key('y');
    await browser.keys(['Backspace', 'Backspace', 'Backspace', 'Backspace']);
    await waitGone('tv-lib-search', 'an emptied search still showed its chip');
    await waitText('tv-lib-count', COUNT);
    expect(await $(testId('tv-lib-empty')).isExisting()).toBe(false);
    await key('Escape');
    await waitOskClosed();
    await waitText('tv-lib-search', 'Search: save');
  });

  it('a launch with no emulator offers "Set up <Platform>"; View opens the system page (item 318)', async () => {
    await toAllGames();
    await openLibraryDetails('No Emu Game');
    await waitFocused('db-play');
    await key('Enter');
    await waitToast(/^Couldn't launch No Emu Game/);
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-toast-action"]')).map((a) => a.textContent?.trim()))).includes(
          'Set up Nintendo 64',
        ),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the card never offered Set up Nintendo 64' },
    );
    await key('v');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'View never opened the system page' });
    await waitText('tv-crumb', /Nintendo 64/);
    // The real System page (P11): no emulator, so the list to choose one.
    await $(testId('tv-row-choose')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the page never offered Choose another emulator' });
    expect(await text('tv-system-page')).toContain('No emulator');
    await key('Escape');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await $(testId('tv-library')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('an install with no library folder asks for one; the folder browser saves it (checklist 530)', async () => {
    const library = path.join(dataDir(), 'library');
    // Something to install: SaveSyncLaunch off this computer, then no library folder.
    await invoke('uninstall_game', { romId: 602 });
    await invoke('set_library_path', { path: '' });
    try {
      // The Library reads the registry again each time its tab shows.
      await key('e');
      await waitTab('downloads');
      await key('q');
      await waitTab('library');
      await tvOpenRommPlatform('snes');
      await key('ArrowRight');
      await browser.waitUntil(async () => ((await focused()) ?? '').startsWith('pg'), { timeout: TRANSITION_TIMEOUT });
      for (let i = 0; i < 8 && (await focused()) !== 'pg602'; i++) {
        await key('ArrowRight');
        await browser.pause(80);
      }
      await waitFocused('pg602');
      expect(await hints()).toContain('X Install');
      await key('x');
      await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no library-folder prompt' });
      expect((await $(`${testId('tv-confirm')} h2`).getText()).trim()).toBe('Set a library folder');
      expect((await $(`${testId('tv-confirm')} p`).getText()).trim()).toBe(
        'Choose where games install, then install again.',
      );
      expect((await $(testId('tv-confirm-ok')).getText()).trim()).toBe('Choose a folder…');
      await waitFocused('dlg-cancel');
      await key('ArrowRight');
      await key('Enter');
      await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
      await waitText('tv-crumb', /Library folder/);
      const home = await text('tv-fs-path');
      await key('y');
      await waitToast('Library folder set');
      expect(await text('tv-toasts')).toContain(home);
    } finally {
      await invoke('set_library_path', { path: library });
    }
  });

  it('a native game’s Saves lists its save locations; one is added through the browser and removed, asked first (checklist 531)', async () => {
    await toAllGames();
    await openLibraryDetails('My Game');
    for (let i = 0; i < 4 && !(await $(testId('tv-saves-locs-heading')).isExisting()); i++) {
      await key('e');
      await browser.pause(200);
    }
    await waitText('tv-saves-locs-heading', 'Save locations');
    // The PCGamingWiki lookup answers (or fails) first.
    await browser.waitUntil(async () => !(await text('tv-saves-locs-status')).startsWith('Looking up'), {
      timeout: 60_000,
      timeoutMsg: 'the save location lookup never finished',
    });
    const before = await locations();
    await seek('sl-add');
    await waitText('tv-saves-loc-add', 'Add a save folder…');
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
    await waitText('tv-crumb', /Save folder for My Game/);
    const game = path.join(dataDir(), 'library', 'games', 'Windows', 'My Game');
    await waitText('tv-fs-path', game);
    await moveTo('fs-tools', 'ArrowDown', 8);
    await key('Enter');
    await waitText('tv-fs-path', path.join(game, 'tools'));
    await key('y');
    await waitToast('Save folder added');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await locations()).length === before.length + 1, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder never joined the list' });
    const added = (await locations()).find((l) => !before.some((b) => b.raw === l.raw))!;
    expect(added.expanded.endsWith('tools') || added.raw.endsWith('tools')).toBe(true);
    await waitText('tv-saves-locs-status', `${before.length + 1} save location(s) configured.`);
    await seek(`sl${added.raw}`);
    expect(await hints()).toContain('A Remove');
    await key('Enter');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect((await $(`${testId('tv-confirm')} h2`).getText()).trim()).toBe('Remove this save location?');
    expect((await $(testId('tv-confirm-ok')).getAttribute('class')) ?? '').toContain('dng');
    await waitFocused('dlg-cancel');
    await key('ArrowRight');
    await key('Enter');
    await browser.waitUntil(async () => (await locations()).length === before.length, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the location never left the list' });
    // Focus moves to the row in its place (the Add button when it was the last).
    await browser.waitUntil(async () => /^sl/.test((await focused()) ?? ''), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'focus left the list' });
    if (before.length === 0) {
      await waitFocused('sl-add');
      expect(await $(testId('tv-saves-upload')).getAttribute('data-why')).toBe('Add a save location to enable uploads.');
    }
  });

  it('had no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
