import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openRow, waitForStatus } from '../helpers/systems.js';

/**
 * Stage `settings-screens` (opt-in, screenshots only): Choose your systems and
 * every Settings page at 1280×800 in both themes, saved under
 * ctr-out/<E2E_SETTINGS_SHOTS or settings-screens>/ (git-ignored), for a side
 * by side with the design prototype (attachments/proposed-ui/30–45). Runs on
 * the first-run-setup fixtures and the mock forge, so Systems has a Ready
 * row, a row with no emulator and Windows. Nothing is asserted beyond what
 * the screens need to appear.
 */

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', process.env.E2E_SETTINGS_SHOTS || 'settings-screens');
const INSTALL_WAIT = 60_000;
const PAGES = ['appearance', 'cloud-saves', 'connection', 'library', 'retroachievements', 'systems', 'updates'];

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.pause(300);
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/** Sets a select the way the browser reports a user's change. */
async function setSelect(id: string, value: string): Promise<void> {
  await browser.execute(
    (sel: string, v: string) => {
      const el = document.querySelector(sel) as HTMLSelectElement;
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    },
    testId(id),
    value,
  );
}

async function page(p: string): Promise<void> {
  await $(testId(`settings-nav-${p}`)).click();
  await $(testId(`settings-page-${p}`)).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await browser.execute((sel: string) => {
    const pane = document.querySelector(sel);
    if (pane) pane.scrollTop = 0;
  }, testId(`settings-page-${p}`));
}

async function scrollPane(p: string, to: 'top' | 'bottom'): Promise<void> {
  await browser.execute(
    (sel: string, where: string) => {
      const pane = document.querySelector(sel);
      if (pane) pane.scrollTop = where === 'top' ? 0 : pane.scrollHeight;
    },
    testId(`settings-page-${p}`),
    to,
  );
}

describe('settings screens: every Settings page for a design comparison', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT });
    await browser.setWindowSize(1280, 800);
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(path.join(dataDir(), 'library'));
    await $(testId('connect-submit')).click();
  });

  it('Choose your systems, empty and ticked', async () => {
    await $(testId('setup-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('sys-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await shot('setup-1-empty');
    await $(testId('sys-tick-ps2')).click();
    await $(testId('sys-tick-dc')).click();
    await browser.waitUntil(async () => $(testId('sys-tick-windows')).isEnabled(), { timeout: TRANSITION_TIMEOUT });
    await $(testId('sys-tick-windows')).click();
    await shot('setup-2-ticked');
    await $(testId('setup-install')).click();
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('waits for the installs so Systems shows Ready rows', async () => {
    await $(testId('nav-settings')).click();
    await page('systems');
    await waitForStatus('ps2', 'ready', INSTALL_WAIT);
    await waitForStatus('dc', 'ready', INSTALL_WAIT);
    // Let the setup card finish and leave the corner.
    await browser.pause(12_000);
  });

  for (const theme of ['light', 'dark']) {
    it(`every page, ${theme}`, async () => {
      await page('appearance');
      await setSelect('theme-select', theme);
      await browser.pause(500);
      for (const p of PAGES) {
        await page(p);
        await shot(`${theme}-${p}`);
      }

      await page('connection');
      await $(testId('settings-connection-edit')).click();
      await shot(`${theme}-connection-edit`);
      await $(testId('settings-connection-cancel')).click();

      await page('systems');
      await openRow('ps2');
      await shot(`${theme}-systems-row-open`);
      await openRow('windows');
      await scrollPane('systems', 'bottom');
      await shot(`${theme}-systems-bottom`);
      await scrollPane('systems', 'top');
      await $(testId('systems-add-manual')).click();
      await $(testId('systems-sheet')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
      await shot(`${theme}-systems-add-manually`);
      await $(testId('systems-sheet-close')).click();
    });
  }
});
