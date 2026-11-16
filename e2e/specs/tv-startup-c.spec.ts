import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { tvRailKeys } from '../helpers/romm.js';
import { connectPad, focused, hintLeft, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, chooseWelcomeLibrary, moveTo, pageErrors, testId, text, waitText } from '../helpers/tvDetails.js';
import { tapK } from '../helpers/tvBackups.js';

/**
 * Stage `tv-startup`, spec c (TV mode plan P12; romm-storefront-plan
 * decisions 2, 6 and 16): a fresh profile with one Backup drive
 * (COUCHDRIVE, the "Couch kit" pack), started with --tv. The welcome needs
 * only the library folder: Skip for now on step 2 opens GRID with nothing
 * connected, on All games (never connected), with the drive in the rail and
 * no connection text or offline line.
 */
describe('tv-startup (c): Skip for now opens the Library on All games, the drive in the rail', () => {
  const drive = path.join(dataDir(), 'media', 'COUCHDRIVE');

  before(async () => {
    await $(testId('tv-welcome')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed the welcome' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('with only the library folder, step 2 offers Skip for now (item 515)', async () => {
    await chooseWelcomeLibrary(path.join(dataDir(), 'library'));
    await key('Enter');
    await $(testId('tv-welcome-stores')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Next never showed step 2' });
    await waitText('tv-welcome-go', 'Skip for now');
    expect(await text('tv-welcome-row-romm-pill')).toBe('Not connected');
  });

  it('opens the Library on All games, the drive in its folder, with no offline line (items 415, 516, decision 16)', async () => {
    await tapK('w-go');
    await waitTab('library');
    await waitFocused('r-all', `All games never took focus (focused: ${await focused()})`);
    await waitText('tv-lib-name', 'All games');
    expect(await $(testId('tv-lib-fold-drives')).isExisting()).toBe(true);
    await browser.waitUntil(async () => (await tvRailKeys()).includes(`r-drive:${drive}`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the drive never reached the rail',
    });
    expect(await $(testId('tv-connection')).isExisting()).toBe(false);
    expect(await hintLeft()).not.toMatch(/Offline|No server/);
  });

  it('the drive row shows its pack', async () => {
    await moveTo(`r-drive:${drive}`, 'ArrowDown', 8);
    await waitText('tv-lib-name', 'Couch kit');
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
