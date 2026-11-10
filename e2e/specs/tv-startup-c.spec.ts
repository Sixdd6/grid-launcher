import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hintLeft, invoke, key, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, text, waitText } from '../helpers/tvDetails.js';

/**
 * Stage `tv-startup`, spec c (TV mode plan P12): a fresh profile with one
 * Backup drive (COUCHDRIVE, the "Couch kit" pack), started with --tv. Use
 * without a server needs only the library folder; it says "Using GRID
 * without a server" and opens the Library on the first connected Backup
 * drive, its folder open and its row focused.
 */
describe('tv-startup (c): Use without a server opens the Library on the first drive', () => {
  const library = path.join(dataDir(), 'library');
  const drive = path.join(dataDir(), 'media', 'COUCHDRIVE');

  before(async () => {
    await $(testId('tv-connect')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed Connect' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('Use without a server sits after "or", beside Connect, with its note (item 515)', async () => {
    await moveTo('cf-lib', 'ArrowDown');
    await invoke('e2e_clipboard_write', { text: library });
    await key('v');
    await waitText('tv-cf-lib-value', library);
    await key('ArrowDown');
    await waitFocused('cf-go');
    await key('ArrowRight');
    await waitFocused('cf-none');
    expect(await $(testId('tv-connect-offline')).getAttribute('aria-disabled')).toBeNull();
    expect(await $(`${testId('tv-connect')} .or`).getText()).toBe('or');
  });

  it('says "Using GRID without a server" and opens the Library on the first drive, its row focused (items 415, 516)', async () => {
    await key('Enter');
    await waitToast('Using GRID without a server');
    await waitTab('library');
    await waitFocused(`r-drive:${drive}`, `the first drive's row never took focus (focused: ${await focused()})`);
    expect(await $(testId('tv-lib-fold-drives')).isExisting()).toBe(true);
    await waitText('tv-connection', 'Offline · no server');
    expect(await hintLeft()).toBe('No server. Your Library and Backup drives work without one.');
    await browser.waitUntil(async () => (await text('tv-lib-name')).length > 0, { timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-lib-name')).toBe('Couch kit');
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
