import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl } from '../helpers/env.js';

/**
 * Stage `ui-screens` (opt-in, screenshots only): the main views at several
 * background-fade levels in both themes, saved under ctr-out/ui-screens/
 * (git-ignored) for a design review. Nothing is asserted.
 *
 * Against the mock by default. With E2E_LIVE_URL, E2E_LIVE_TOKEN and
 * E2E_LIVE_PLATFORM it connects to a real RomM server instead, so the
 * screens carry real covers and background art; the token is read from the
 * environment and typed into the Connect form, never logged. The run only
 * reads from the server.
 */

const testId = (id: string) => `[data-testid="${id}"]`;
const LIVE_URL = process.env.E2E_LIVE_URL ?? '';
const PLATFORM_ID = Number(process.env.E2E_LIVE_PLATFORM ?? '1');
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'ui-screens');

async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/** Sets a control's value the way the browser reports a user's change:
 *  WebKitGTK's WebDriver does not fire `input`/`change` itself. */
async function setControl(id: string, value: string): Promise<void> {
  await browser.execute(
    (sel: string, v: string) => {
      const el = document.querySelector(sel) as HTMLInputElement | HTMLSelectElement;
      el.value = v;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    },
    testId(id),
    value,
  );
}

async function appearance(theme: string, fade: number): Promise<void> {
  await $(testId('nav-settings')).click();
  await $(testId('settings-nav-appearance')).click();
  await setControl('theme-select', theme);
  await setControl('background-fade', String(fade));
  await browser.pause(400);
}

async function openServerPlatform(): Promise<void> {
  await $(testId('nav-server')).click();
  await $(testId(`platform-btn-${PLATFORM_ID}`)).click();
  await browser.waitUntil(
    () => browser.execute(() => (document.querySelector('[data-testid="server-grid"]')?.children.length ?? 0) > 0),
    { timeout: 60_000 },
  );
  // Covers and the background art load lazily.
  await browser.pause(4000);
}

/** Rests the pointer on the first card so the background art shows it. */
async function hoverFirstCard(): Promise<void> {
  const card = await $('[data-testid="server-grid"] [data-testid^="game-card-"]');
  if (await card.isExisting()) {
    await card.moveTo();
    await browser.pause(1500);
  }
}

describe('ui screens: the main views for a design review', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT });
    await browser.setWindowSize(1600, 1000);
    await shot('00-connect');
    await $(testId('connect-server-url')).setValue(LIVE_URL || mockUrl());
    await $(testId('connect-secret')).setValue(LIVE_URL ? (process.env.E2E_LIVE_TOKEN ?? '') : FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId(`platform-btn-${PLATFORM_ID}`)).waitForExist({ timeout: 60_000 });
  });

  for (const theme of ['dark', 'light']) {
    for (const fade of [0, 30, 60]) {
      it(`server grid and details, ${theme}, fade ${fade}`, async () => {
        await appearance(theme, fade);
        await openServerPlatform();
        await hoverFirstCard();
        await shot(`server-${theme}-fade${fade}`);
        const card = await $('[data-testid="server-grid"] [data-testid^="game-card-"]');
        if (await card.isExisting()) {
          await card.click();
          await $(testId('details-panel')).waitForDisplayed({ timeout: 20_000 });
          await browser.pause(2500);
          await shot(`details-${theme}-fade${fade}`);
          await $(testId('details-close')).click();
        }
      });
    }
  }

  it('labels on hover, both themes, fade 60', async () => {
    for (const theme of ['dark', 'light']) {
      await appearance(theme, 60);
      await setControl('card-labels', 'hover');
      await openServerPlatform();
      await hoverFirstCard();
      await shot(`server-${theme}-fade60-hover-labels`);
    }
    await $(testId('nav-settings')).click();
    await setControl('card-labels', 'always');
  });

  it('the other views, dark, fade 30', async () => {
    await appearance('dark', 30);
    await shot('settings-dark-fade30');
    for (const view of ['library', 'emulators', 'downloads']) {
      const nav = await $(testId(`nav-${view}`));
      if (!(await nav.isExisting())) continue;
      await nav.click();
      await browser.pause(1500);
      await shot(`${view}-dark-fade30`);
    }
  });
});
