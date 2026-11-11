import { readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, key, waitFocused, XBOX_PAD } from '../helpers/tv.js';
import { testId } from '../helpers/tvDetails.js';

/**
 * Stage `tv-startup`, spec e (TV mode plan P12; ruling 18): the
 * unreachable library again (`library-move-c`'s seed). Quit GRID
 * sends the quit at once, with no second question, and
 * the stored library is untouched.
 *
 * The backend's exit would end the WebDriver session mid-spec (the runner
 * then counts the spec as failed), so the e2e build's `__tvE2EHoldQuit`
 * counts the quit instead of sending it; the exit itself is
 * `library_unreachable_close`'s, which the desktop dialog shares.
 */
describe('tv-startup (e): Quit GRID quits at once', () => {
  it('sends the quit with no question and changes nothing', async () => {
    await $(testId('tv-unreachable')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the unreachable-library dialog never appeared in TV mode' });
    await connectPad(XBOX_PAD);
    await waitFocused('ur-relocate');
    await browser.execute(() => {
      (window as unknown as { __tvE2EHoldQuit: boolean }).__tvE2EHoldQuit = true;
    });
    const before = readFileSync(configPath(), 'utf-8');
    // The buttons wrap onto two lines in the 40rem dialog (as on the prototype).
    await key('ArrowRight');
    if ((await focused()) !== 'ur-close') await key('ArrowDown');
    await waitFocused('ur-close');
    await key('Enter');
    await browser.waitUntil(async () => ((await browser.execute(() => (window as unknown as { __tvE2EQuits?: number }).__tvE2EQuits ?? 0)) as number) === 1, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Quit GRID never asked the backend to quit',
    });
    expect(await $(testId('tv-confirm')).isExisting()).toBe(false);
    expect(readFileSync(configPath(), 'utf-8')).toBe(before);
    expect(before).toContain(JSON.stringify(path.join(dataDir(), 'relocate-missing')));
  });
});
