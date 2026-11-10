import { readFileSync } from 'node:fs';
import { configPath, mockUrl, TRANSITION_TIMEOUT } from './env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Switches the mock RomM between answering and dropping every API request. */
export async function setServerOffline(offline: boolean): Promise<void> {
  const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ offline }),
  });
  expect(res.ok).toBe(true);
}

/** Waits until `view` is the top bar's selected pill and its view is shown. */
export async function expectStartView(view: 'library' | 'server'): Promise<void> {
  await $(testId(`${view}-view`)).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `GRID did not open on ${view}`,
  });
  await expect($(testId(`nav-${view}`))).toHaveAttribute('aria-current', 'page');
}

/** Settings › Appearance, the pane where Open GRID on lives. */
export async function openAppearance(): Promise<void> {
  await $(testId('nav-settings')).click();
  await $(testId('settings-nav-appearance')).click();
  await $(testId('open-on-select')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
}

/**
 * Picks an Open GRID on choice and waits for it to reach config.toml. The
 * value is set and `change` dispatched directly: a WebDriver click on an
 * `<option>` does not reliably fire `change` on this WebKitGTK driver (see
 * launch.spec.ts).
 */
export async function chooseOpenOn(value: 'automatic' | 'discover' | 'library'): Promise<void> {
  await browser.execute((val) => {
    const el = document.querySelector('[data-testid="open-on-select"]') as HTMLSelectElement;
    el.value = val;
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
  await browser.waitUntil(() => readFileSync(configPath(), 'utf-8').includes(`open_on = "${value}"`), {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `open_on = "${value}" never reached config.toml`,
  });
}
