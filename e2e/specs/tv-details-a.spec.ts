// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { assertNoClipping, axis, connectPad, focused, hints, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import {
  bodyScroll,
  captureErrors,
  moveTo,
  openLibraryDetails,
  pageErrors,
  testId,
  text,
  waitButtons,
  waitGone,
  waitText,
} from '../helpers/tvDetails.js';

/** The text of each element matching `sel`, whitespace collapsed. */
async function lines(sel: string): Promise<string[]> {
  return (await browser.execute(
    (s: string) => Array.from(document.querySelectorAll(s)).map((e) => (e.textContent ?? '').replace(/\s+/g, ' ').trim()),
    sel,
  )) as string[];
}

/**
 * Stage `tv-details` (docs/tv-mode/plan.md P9), spec a: connects on the
 * desktop, enters TV mode, and works through Details on the cloud-saves
 * games (seed/tv-details-seed.mjs): SaveSyncRetention's buttons, its long
 * summary and Related row, the right stick, LB / RB, its four server saves
 * (delete, upload, restore), Uninstall's confirmation; then Playing for
 * SaveSyncLaunch (stopped with the controller, which reaches Playing while
 * GRID's window is focused) and SaveSyncManual (whose stub exits by itself).
 */
describe('tv-details (a): Details, Saves and Playing', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('tv-mode-button')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the desktop shell never loaded' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await connectPad(XBOX_PAD);
    await key('q');
    await waitTab('library');
    await waitFocused('r-all');
  });

  it('A on a cover pushes Details: left lines, header, buttons, the first button focused (items 270–277)', async () => {
    await openLibraryDetails('SaveSyncRetention');
    await waitText('tv-crumb', /SaveSyncRetention/);
    await waitButtons(['Play', 'Uninstall', 'Backup to drive…']);
    await waitFocused('db-play');
    expect(await lines('#tv-det-left-lines > div')).toEqual(['Last played never', 'Launches with TestEmu']);
    await waitText('tv-det-header', /^Super Nintendo Entertainment System · 1995 · Fixture Works · RPG · \S+$/);
    expect(await browser.execute(() => Array.from(document.querySelectorAll('.det-r .chips .chip')).map((c) => c.textContent?.trim()))).toContain('Installed');
    expect(await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-det-tabs"] .ttab')).map((t) => t.textContent))).toEqual([
      'Overview',
      'Media',
      'Saves',
    ]);
    expect(await hints()).toEqual(['A Select', 'RS Scroll', 'B Back', 'MENU Menu']);
    await assertNoClipping();
  });

  it('Down focuses the summary, which reads through before Related; Up from Related returns to it (items 294, 296–298)', async () => {
    await key('ArrowDown');
    await waitFocused('prose');
    expect(await hints()).toContain('DPAD Read');
    const before = await bodyScroll();
    await key('ArrowDown');
    await browser.waitUntil(async () => (await bodyScroll()) > before, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Down did not scroll the summary' });
    expect(await focused()).toBe('prose');
    // Read on to the end: focus then moves on to Related.
    await waitText('tv-det-body', /Related2/);
    for (let i = 0; i < 40 && (await focused()) === 'prose'; i++) {
      await key('ArrowDown');
      await browser.pause(120);
    }
    await waitFocused('rel601');
    await key('ArrowRight');
    await waitFocused('rel602');
    await key('ArrowUp');
    await waitFocused('prose', 'Up from Related did not return to the summary');
  });

  it('Up into the buttons scrolls the tab back to its top (item 299)', async () => {
    for (let i = 0; i < 40 && (await focused()) === 'prose'; i++) {
      await key('ArrowUp');
      await browser.pause(120);
    }
    await waitFocused('db-play');
    await browser.waitUntil(async () => (await bodyScroll()) === 0, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the tab never scrolled back to its top' });
  });

  it('the right stick scrolls the tab without moving focus (item 300)', async () => {
    await axis('right_stick_y', -0.9);
    await browser.pause(400);
    await axis('right_stick_y', 0);
    await browser.waitUntil(async () => (await bodyScroll()) > 0, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the right stick did not scroll the tab' });
    expect(await focused()).toBe('db-play');
  });

  it("LB / RB switch tabs, wrapping, focusing the tab's first item or the first button (items 292–293)", async () => {
    await press('left_bumper');
    await waitFocused('sk-save', 'LB from Overview did not wrap to Saves and focus its first item');
    expect(await bodyScroll()).toBe(0);
    await press('right_bumper');
    await waitFocused('prose');
    await press('right_bumper');
    // Media has nothing to focus here: the first button.
    await $(testId('tv-det-no-media')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('db-play');
    await press('right_bumper');
    await waitFocused('sk-save');
  });

  it('Saves lists the server saves, newest first and marked LATEST (items 306–307)', async () => {
    await waitText('tv-saves-heading', 'Cloud Saves');
    await $(testId('tv-save-9001')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the saves never listed' });
    const rows = await browser.execute(() => Array.from(document.querySelectorAll('#saves-list [data-k]')).map((r) => r.getAttribute('data-k')));
    expect(rows).toEqual(['sv9001', 'sv9002', 'sv9003', 'sv9004']);
    expect(await lines('[data-testid="tv-save-9001"] .t')).toEqual(['TestEmu LATEST']);
    expect((await lines('[data-testid="tv-save-9001"] .s'))[0]).toMatch(/^.+ · .+$/);
    expect(await text('tv-saves-upload')).toBe('Upload latest save');
  });

  it('Delete asks with Cancel focused; the entry leaves and focus moves to the one in its place (items 308, 310–311)', async () => {
    await key('ArrowDown');
    await waitFocused('sv9001');
    await key('ArrowDown');
    await waitFocused('sv9002');
    expect(await hints()).toContain('A Restore or delete');
    await key('Enter');
    await $(testId('tv-save-delete')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('ArrowDown');
    await key('Enter');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('dlg-cancel');
    expect(await text('tv-confirm')).toContain('Delete this save?');
    await key('ArrowRight');
    await waitFocused('dlg-ok');
    await key('Enter');
    await waitToast('Save deleted from the server');
    await waitGone('tv-save-9002', 'the deleted save stayed in the list');
    await waitFocused('sv9003', 'focus did not move to the entry in its place');
  });

  it('Upload notifies and the upload shows at the top as LATEST (item 312)', async () => {
    await key('ArrowUp');
    await key('ArrowUp');
    await moveTo('up', 'ArrowRight', 4);
    await key('Enter');
    await waitToast('Uploading the latest save…');
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.querySelector('#saves-list [data-k]')?.getAttribute('data-k'))) !== 'sv9001',
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the upload never showed at the top' },
    );
    expect(await browser.execute(() => document.querySelector('#saves-list [data-k] .latest')?.textContent)).toBe('LATEST');
  });

  it('Restore asks, then notifies (item 309)', async () => {
    await moveTo('sv9001', 'ArrowDown', 4);
    await key('Enter');
    await $(testId('tv-save-restore')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Enter');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-confirm')).toContain('Restore this save?');
    await waitFocused('dlg-cancel');
    await key('ArrowRight');
    await key('Enter');
    await waitToast('Save restored for SaveSyncRetention');
  });

  it('Uninstall asks first, with Cancel focused; B leaves it installed (item 286)', async () => {
    await press('left_bumper');
    await press('left_bumper');
    await waitFocused('prose');
    await key('ArrowUp');
    await waitFocused('db-play');
    await moveTo('db-uninstall', 'ArrowRight', 4);
    await key('Enter');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('dlg-cancel');
    expect(await text('tv-confirm')).toContain('Uninstall SaveSyncRetention?');
    await key('Escape');
    await waitGone('tv-confirm');
    await waitFocused('db-uninstall');
    await waitButtons(['Play', 'Uninstall', 'Backup to drive…']);
  });

  it('Play pushes Playing; the controller drives it while GRID is focused; Stop asks and goes back (items 316, 321–325; user decision 2026-11-10)', async () => {
    await key('Escape');
    await $(testId('tv-library')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await openLibraryDetails('SaveSyncLaunch');
    await waitFocused('db-play');
    await key('Enter');
    await waitToast('Launching SaveSyncLaunch…');
    await $(testId('tv-playing')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Play never pushed Playing' });
    expect(await text('tv-playing')).toContain('NOW PLAYING');
    await waitText('tv-playing-title', 'SaveSyncLaunch');
    await waitText('tv-playing-line', /^Super Nintendo Entertainment System · TestEmu · \d+:\d\d$/);
    await waitFocused('pl-resume');
    await waitText('tv-crumb', /Playing/);
    // GRID's window has focus (the game is a stub with no window): the
    // controller reaches Playing while the game runs.
    await press('dpad_right');
    await waitFocused('pl-stop');
    await press('south');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-confirm')).toContain('Stop SaveSyncLaunch?');
    await waitFocused('dlg-cancel');
    await press('dpad_right');
    await press('south');
    await waitGone('tv-playing', 'Stop game never went back');
    await waitText('tv-det-title', 'SaveSyncLaunch');
  });

  it('Playing closes by itself when the game exits (ruling 7)', async () => {
    await key('Escape');
    await $(testId('tv-library')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await openLibraryDetails('SaveSyncManual');
    await waitFocused('db-play');
    await key('Enter');
    await $(testId('tv-playing')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Play never pushed Playing' });
    await browser.waitUntil(async () => !(await $(testId('tv-playing')).isExisting()), {
      timeout: 20_000,
      timeoutMsg: 'Playing never closed after the game exited',
    });
    await waitText('tv-det-title', 'SaveSyncManual');
  });

  it('had no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
