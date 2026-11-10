// e2e-app-args:
import { APP_START_TIMEOUT } from '../helpers/env.js';
import { invoke } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `tv-shell`, part C: part B quit in TV mode; a restart without --tv
 * opens the desktop, because the mode is never kept (item 524).
 */
describe('tv-shell (c): a restart without --tv', () => {
  it('opens the desktop', async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
    await expect($(testId('tv-shell'))).not.toExist();
    const info = await invoke<{ mode: string; started_with_tv: boolean; fullscreen: boolean }>('get_ui_mode');
    expect(info).toMatchObject({ mode: 'desktop', started_with_tv: false, fullscreen: false });
  });
});
