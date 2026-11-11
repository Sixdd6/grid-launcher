import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import {
  captureErrors,
  moveTo,
  openLibraryDetails,
  pageErrors,
  testId,
  text,
  typeKeys,
  waitButtons,
  waitOskClosed,
  waitText,
} from '../helpers/tvDetails.js';

/**
 * Stage `tv-details`, spec b: starts with --tv on the session spec a saved.
 * Link to server game for Mystery Cart (no server id) with the link page's
 * search; Locate… for Lost Cart through the folder browser (its copy is in
 * <data>/elsewhere); Game settings for My Game (Windows).
 */
describe('tv-details (b): Link, Locate…, Game settings', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
    await captureErrors();
    await waitTab('server');
    await connectPad(XBOX_PAD);
    await key('q');
    await waitTab('library');
    await waitFocused('r-all');
  });

  it('an unlinked game offers Link to server game and Remove; its Saves tab says why (items 282, 314)', async () => {
    await openLibraryDetails('Mystery Cart');
    await waitButtons(['Link to server game', 'Remove from library']);
    expect(await text('tv-det-left')).toBe('Not linked to a server game. Link it to your server to play.');
    await press('right_bumper');
    await press('right_bumper');
    await waitText('tv-saves-note', 'Link this game to use cloud saves.');
    // Nothing to focus in the tab: the first button.
    await waitFocused('db-link');
  });

  it('the link page lists the server games no row holds; Y narrows it as you type (items 346, 348)', async () => {
    await key('Enter');
    await $(testId('tv-link-page')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Link to server game never opened' });
    await waitText('tv-crumb', /Link to server game/);
    await waitText('tv-link-sub', 'Choose the matching Super Nintendo Entertainment System game');
    await $(testId('tv-link-604')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the server games never listed' });
    const listed = await browser.execute(() => Array.from(document.querySelectorAll('#link-list [data-k]')).map((r) => r.getAttribute('data-k')));
    // 601–603 and 605 are held by installed rows.
    expect(listed).toEqual(['lk604']);
    expect(await hints()).toEqual(['A Link', 'Y Search', 'B Back', 'MENU Menu']);
    await key('y');
    await typeKeys('zz');
    await waitText('tv-link-none', 'No games match.');
    await waitText('tv-link-sub', /Search: zz$/);
    await browser.keys(['Backspace', 'Backspace']);
    await typeKeys('link');
    await key('Enter');
    await waitOskClosed();
    await waitFocused('lk604');
  });

  it('A links the game, notifies and goes back to Details, now playable (item 347)', async () => {
    await key('Enter');
    await waitToast('Mystery Cart linked to Link Target');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitButtons(['Play', 'Uninstall', 'Backup to drive…']);
    await key('Escape');
    await $(testId('tv-library')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('a missing game offers Locate…; the folder browser picks the folder holding it (items 281, 290, 349–352)', async () => {
    await openLibraryDetails('Lost Cart');
    await waitButtons(['Locate…', 'Remove from library', 'Backup to drive…']);
    await waitFocused('db-locate');
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
    await waitText('tv-crumb', /Where is Lost Cart\?/);
    const lib = path.join(dataDir(), 'library');
    await waitText('tv-fs-path', path.join(lib, 'games', 'Super Nintendo Entertainment System'));
    expect(await hints()).toContain('Y Use this folder');
    // Up one folder is the first row; A on it goes up.
    for (const want of [path.join(lib, 'games'), lib, dataDir()]) {
      await waitFocused('up');
      expect(await hints()).toContain('A Up');
      await key('Enter');
      await waitText('tv-fs-path', want);
    }
    await moveTo('fs-elsewhere', 'ArrowDown', 12);
    expect(await hints()).toContain('A Open');
    await key('Enter');
    await waitText('tv-fs-path', path.join(dataDir(), 'elsewhere'));
    await waitFocused('up', 'opening a folder did not focus its first row');
    await key('y');
    await waitToast('Lost Cart found');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitButtons(['Play', 'Uninstall', 'Backup to drive…']);
    await key('Escape');
    await $(testId('tv-library')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('Game settings: the launch file list, then Save notifies and goes back (items 342–345)', async () => {
    await openLibraryDetails('My Game');
    await waitButtons(['Play', 'Game settings', 'Uninstall', 'Backup to drive…']);
    await moveTo('db-game-settings', 'ArrowRight', 4);
    await key('Enter');
    await $(testId('tv-game-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Game settings never opened' });
    await $(testId('tv-row-exe')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-game-settings')).toContain('Game settings');
    await waitText('tv-row-exe', /^Launch file\s*Your choice\s*mygame\.exe/);
    expect(await text('tv-row-params')).toMatch(/^Launch parameters\s*Optional\s*None/);
    await waitFocused('s-exe');
    await key('Enter');
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const options = await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-list"] [data-k]')).map((o) => o.textContent?.trim()));
    expect(options.some((o) => o?.includes('tools/setup.exe'))).toBe(true);
    await moveTo(
      (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-list"] [data-k]')).find((o) => o.textContent?.includes('setup.exe'))?.getAttribute('data-k') ?? '')) as string,
      'ArrowDown',
      4,
    );
    await key('Enter');
    await waitText('tv-row-exe', /tools\/setup\.exe/);
    await moveTo('s-save', 'ArrowDown', 8);
    await key('Enter');
    await waitToast('Game settings saved');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Save did not go back' });
    expect(await focused()).toBe('db-game-settings');
  });

  it('had no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
