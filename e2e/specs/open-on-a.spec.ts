import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { expectStartView, openAppearance, setServerOffline } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part A: Where GRID opens (TV mode README § Decided), with
 * Open GRID on at its default, Automatic. A connect lands on Server ›
 * Discover, and Settings › Appearance shows Open GRID on as its first row.
 * Parts B–D relaunch against the same data directory.
 */
describe('open-on (a): Automatic and connected opens Server › Discover', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
  });

  it('opens on Server › Discover', async () => {
    await expectStartView('server');
    await $(testId('server-rail-discover')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('server-rail-discover'))).toHaveAttribute('aria-current', 'page');
  });

  it('shows Open GRID on first in Settings › Appearance', async () => {
    await openAppearance();
    const pane = await browser.execute(() => {
      const select = document.querySelector('[data-testid="open-on-select"]') as HTMLSelectElement;
      const row = select.closest('.setting')!;
      const first = row.parentElement!.querySelector('.setting');
      return {
        first: first === row,
        label: row.querySelector('.name label')?.textContent?.trim() ?? '',
        hint: row.querySelector('.name small')?.textContent?.trim() ?? '',
        value: select.value,
        options: [...select.options].map((o) => [o.value, o.textContent?.trim() ?? '']),
      };
    });
    expect(pane).toEqual({
      first: true,
      label: 'Open GRID on',
      hint: 'Here and in TV mode',
      value: 'automatic',
      options: [
        ['automatic', 'Automatic: Discover when connected, else Library'],
        ['discover', 'Server › Discover'],
        ['library', 'Library'],
      ],
    });
  });

  // Last: part B starts against an unreachable server.
  it('takes the mock offline for part B', async () => {
    await setServerOffline(true);
  });
});
