import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { expectConfigSwitched } from '../helpers/libraryMove.js';
import { connectPad, focused, hints, key, press, waitFocused, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, text, waitText } from '../helpers/tvDetails.js';

const missingRoot = () => path.join(dataDir(), 'relocate-missing');
const realRoot = () => path.join(dataDir(), 'relocate-real');

/**
 * Stage `tv-startup`, spec b (TV mode plan P12): `library-move-c`'s seed
 * stores a library root that does not exist (the files are in
 * `<data>/relocate-real`). Started with --tv, GRID shows only the blocking
 * dialog: B and Start do nothing, the hints show only A "Select". Relocate
 * the library… opens the folder browser, B there comes back to the dialog,
 * and a picked folder relocates the library ("Library relocated to <path>" /
 * "No files were moved"), after which Connect follows (no server is stored).
 */
describe('tv-startup (b): the unreachable library', () => {
  before(async () => {
    await $(testId('tv-unreachable')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the unreachable-library dialog never appeared in TV mode' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('blocks with the dialog: its words, Relocate focused, A "Select" and Menu (items 409–411)', async () => {
    expect(await $(`${testId('tv-unreachable')} h2`).getText()).toBe("Your library can't be reached");
    await waitText('tv-unreachable-body', `${missingRoot()} isn't there: its drive may not be connected. GRID can't run without it.`);
    expect(await text('tv-unreachable-relocate')).toBe('Relocate the library…');
    expect(await text('tv-unreachable-close')).toBe('Close without changing anything');
    await waitFocused('ur-relocate');
    expect(await hints()).toEqual(['A Select', 'MENU Menu']);
    expect(await $(testId('tv-tabs')).isExisting()).toBe(false);
    expect(await $(testId('tv-connect')).isExisting()).toBe(false);
  });

  it('B does nothing on it; Start opens the startup Menu over it, and B comes back (items 92, 411; user decision 2026-11-10)', async () => {
    await key('Escape');
    await press('east');
    await browser.pause(300);
    expect(await $(testId('tv-unreachable')).isExisting()).toBe(true);
    expect(await $(testId('tv-menu')).isExisting()).toBe(false);
    await waitFocused('ur-relocate');
    await press('start');
    await $(testId('tv-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Start opened no Menu over the dialog' });
    expect(
      await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-menu"] .opt')).map((e) => e.textContent?.trim() ?? '')),
    ).toEqual(['Switch to desktop mode', 'Quit GRID']);
    await press('east');
    await browser.waitUntil(async () => !(await $(testId('tv-menu')).isExisting()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B never closed the Menu' });
    expect(await $(testId('tv-unreachable')).isExisting()).toBe(true);
    await waitFocused('ur-relocate');
    // The two buttons wrap onto two lines in the 40rem dialog (as on the
    // prototype): Right or Down reaches Close, Left or Up comes back.
    await key('ArrowRight');
    if ((await focused()) !== 'ur-close') await key('ArrowDown');
    await waitFocused('ur-close');
    await key('ArrowLeft');
    if ((await focused()) !== 'ur-relocate') await key('ArrowUp');
    await waitFocused('ur-relocate');
  });

  it('Relocate the library… opens the folder browser; B there comes back to the dialog (items 97, 412)', async () => {
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder browser never opened' });
    expect(await $(`${testId('tv-folder-browser')} h1`).getText()).toBe('Relocate the library');
    expect(await $(testId('tv-unreachable')).isExisting()).toBe(false);
    await key('Escape');
    await $(testId('tv-unreachable')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'B on the browser never brought the dialog back' });
    await waitFocused('ur-relocate');
  });

  it('a picked folder relocates the library: "Library relocated to <path>" / "No files were moved" (item 412)', async () => {
    await key('Enter');
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // It opens beside the missing folder: the data dir.
    await waitText('tv-fs-path', dataDir());
    await $(testId('tv-fs-fs-relocate-real')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await moveTo('fs-relocate-real', 'ArrowDown', 20);
    await key('Enter');
    await waitText('tv-fs-path', realRoot());
    await key('y');
    await waitToast(/^Library relocated to .*relocate-real$/);
    const sub = (await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="tv-toast"]'))
        .find((c) => /^Library relocated/.test(c.querySelector('[data-testid="tv-toast-title"]')?.textContent ?? ''))
        ?.querySelector('.ts')?.textContent ?? '',
    )) as string;
    expect(sub.trim()).toBe('No files were moved');
    await $(testId('tv-connect')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'Connect never followed the relocate' });
    expect(await $(testId('tv-unreachable')).isExisting()).toBe(false);
  });

  it('points the config at the chosen folder', () => {
    expectConfigSwitched(missingRoot(), realRoot());
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
