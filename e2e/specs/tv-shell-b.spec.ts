// e2e-app-args:
import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { invoke, key, waitFocused, waitTab } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

async function waitDesktop(view: string): Promise<void> {
  await $(testId('shell-topbar')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the desktop never came back' });
  await expect($(testId(`nav-${view}`))).toHaveAttribute('aria-current', 'page');
  expect(await browser.execute(() => document.documentElement.dataset.ui ?? null)).toBeNull();
}

/**
 * Stage `tv-shell`, part B: a desktop start (no --tv). The desktop top bar's
 * TV button opens TV mode on the matching tab (item 521), and Settings › TV
 * mode › Switch to desktop mode goes back to the desktop on the same view
 * (item 523). It ends in TV mode, for part C's restart.
 */
describe('tv-shell (b): entering and leaving TV mode', () => {
  before(async () => {
    await $(testId('shell-topbar')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the desktop shell never appeared' });
  });

  it('starts in desktop mode', async () => {
    const info = await invoke<{ mode: string; started_with_tv: boolean }>('get_ui_mode');
    expect(info).toMatchObject({ mode: 'desktop', started_with_tv: false });
    await expect($(testId('tv-shell'))).not.toExist();
  });

  it('the TV button opens TV mode on the matching tab (item 521)', async () => {
    await $(testId('nav-downloads')).click();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('downloads');
    const info = await invoke<{ mode: string }>('get_ui_mode');
    expect(info.mode).toBe('tv');
    expect(await browser.execute(() => document.documentElement.dataset.ui)).toBe('tv');
    await expect($(testId('shell-topbar'))).not.toExist();
  });

  it('Switch to desktop mode lands on the same view (item 523)', async () => {
    await key('e');
    await waitTab('settings');
    await $(testId('tv-set-rail-tv')).click();
    await waitFocused('s-tv-desktop');
    await key('Enter');
    await waitDesktop('settings');
    const info = await invoke<{ mode: string }>('get_ui_mode');
    expect(info.mode).toBe('desktop');
  });

  it('enters again from the Library on the Library tab, and stays there for the restart', async () => {
    await $(testId('nav-library')).click();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitTab('library');
  });
});
