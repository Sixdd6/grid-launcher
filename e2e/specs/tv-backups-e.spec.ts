// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, libCover, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, confirmDialog, entries, gridCovers, pageRow, screenState, tap, tapK, waitNotice } from '../helpers/tvBackups.js';

const DRIVE = 'TESTDRIVE';
const BACKED_UP = 'Cinder Peaks';
const INSTALLING = 'Big Arcade Game';

/**
 * Stage `tv-backups/tv-backups-e` (checklist 438; consultant, after P13), on
 * backups-seed.mjs `tv-backups-e` (= `seedD`: a fresh profile, PCSX2 from
 * the server, Cinder Peaks installed on this computer, TESTDRIVE set up for
 * Linux). Connects on the desktop and enters TV mode. X installs Big Arcade
 * Game (throttled, about ten seconds), then Cinder Peaks is backed up from
 * this computer, queued behind that install. While the backup is queued,
 * Uninstall on Cinder Peaks is refused with the backend's words.
 */
describe('tv-backups-e: a game being backed up refuses Uninstall', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the server view never rendered after connecting' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await connectPad(XBOX_PAD);
    await waitTab('server');
  });

  it('X on a PlayStation 2 cover installs Big Arcade Game', async () => {
    await tapK('r-1');
    await browser.waitUntil(async () => (await gridCovers('#srv-grid')).length === 6, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the PlayStation 2 grid never filled' });
    // Right from the rail reaches the grid's first cover, then along the row.
    for (let i = 0; i < 8 && (await focused()) !== 'sg903'; i++) {
      await key('ArrowRight');
      await browser.pause(80);
    }
    await waitFocused('sg903');
    await key('x');
    await browser
      .waitUntil(async () => (await entries()).some((e) => e.title === INSTALLING && e.status === 'downloading'), { timeout: 20_000, interval: 100 })
      .catch(async () => {
        throw new Error(`"${INSTALLING}" never started downloading: ${JSON.stringify(await entries())}`);
      });
  });

  it('Backup to drive… on Cinder Peaks queues its backup from this computer behind the install', async () => {
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    const k = await libCover(BACKED_UP);
    await tapK(k);
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Details never opened for ${BACKED_UP}` });
    await waitText('tv-det-title', BACKED_UP);
    await tapK('db-backup');
    await $(testId('tv-backup-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', `Backup ${BACKED_UP}`);
    // Copied from this computer's install: that is what Uninstall must wait for.
    await $(testId('tv-pr-bk-local')).waitForExist({ timeout: 15_000, timeoutMsg: `the page never offered to copy from this computer (${await screenState()})` });
    if ((await pageRow('bk-local')).on !== true) await tapK('bk-local');
    await browser.waitUntil(async () => (await pageRow('bk-local')).on === true, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Copy from this computer never turned on' });
    await browser.waitUntil(async () => (await $(testId('tv-rows-main')).getAttribute('aria-disabled')) === null, {
      timeout: 15_000,
      timeoutMsg: `the main button never enabled (it reads "${await text('tv-rows-main')}")`,
    });
    await key('m');
    await waitGone('tv-backup-page');
    await waitNotice(`${BACKED_UP} added to the backup queue for ${DRIVE}`, 10_000);
    // Queued behind the install, which is still running.
    const live = await entries();
    expect(live.some((e) => e.title === INSTALLING && e.status === 'downloading')).toBe(true);
  });

  it('Uninstall on Cinder Peaks is refused while its backup waits (item 438)', async () => {
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-det-title', BACKED_UP);
    await tapK('db-uninstall');
    const words = await confirmDialog();
    expect(words.title).toBe(`Uninstall ${BACKED_UP}?`);
    await waitNotice(/This game is being backed up\. Wait for it to finish\.$/, 10_000);
    // Still installed: the button row keeps Play and Uninstall.
    await $(byK('db-uninstall')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('the queue drains', async () => {
    await browser
      .waitUntil(async () => (await entries()).every((e) => !['queued', 'downloading', 'installing', 'extracting'].includes(e.status)), { timeout: 90_000, interval: 500 })
      .catch(async () => {
        throw new Error(`the queue never drained: ${JSON.stringify(await entries())}`);
      });
    await key('Escape');
    await waitGone('tv-details');
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
