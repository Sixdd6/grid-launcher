/**
 * Shared steps for the desktop GOG specs (GameStores card, package E). They
 * run against package D's mock GOG server (`e2e/mock-romm/mock-gog.mjs`)
 * and fake tools (`e2e/fake-tools/`), wired by the gog stages of
 * `scripts/e2e.sh`. They rely only on the documented contract
 * (docs/stores-phase1-plan.md) and the UI's test ids, not on fixture titles:
 * a stage that seeds a GOG library with at least one installed and one
 * available game is enough.
 */
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from './env.js';

export const testId = (id: string) => `[data-testid="${id}"]`;
export const testIdPrefix = (id: string) => `[data-testid^="${id}"]`;

/** Connects to the mock RomM server (the stage's GOG state comes from its seed). */
export async function connectRomm(): Promise<void> {
  await $(testId('connect-server-url')).waitForExist({
    timeout: APP_START_TIMEOUT,
    timeoutMsg: 'the connect form never appeared',
  });
  await $(testId('connect-server-url')).setValue(mockUrl());
  await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
  await $(testId('connect-submit')).click();
  await $(testId('library-section')).waitForExist({ timeout: TRANSITION_TIMEOUT });
}

/** Opens Library › GOG › <part>. */
export async function openGog(part: 'installed' | 'available'): Promise<void> {
  const pill = await $('button=Library');
  if (await pill.isExisting()) await pill.click();
  const row = await $(testId(`library-rail-store-gog-${part}`));
  await row.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Library › GOG › ${part} never appeared in the rail` });
  await row.click();
  await expect($(testId('library-title'))).toHaveText(expect.stringContaining('GOG'));
}

/** The store cards on screen, in grid order. */
export async function storeCards(): Promise<WebdriverIO.ElementArray> {
  return $$(testIdPrefix('library-store-card-gog-'));
}

/** Opens Settings › Stores. */
export async function openStoresSettings(): Promise<void> {
  await $('button=Settings').click();
  await $(testId('settings-nav-stores')).click();
  await $(testId('settings-stores-pane')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
}

/** Opens Downloads. */
export async function openDownloads(): Promise<void> {
  await $('button=Downloads').click();
}

/** Every Downloads row's status line. */
export async function downloadLines(): Promise<string[]> {
  const out: string[] = [];
  for (const el of await $$(testIdPrefix('download-detail-'))) {
    try {
      out.push((await el.getText()).trim());
    } catch {
      // The row went away mid-read.
    }
  }
  return out;
}

/** Waits until some Downloads line matches `re`. */
export async function waitForDownloadLine(re: RegExp, timeout = TRANSITION_TIMEOUT): Promise<string> {
  let hit = '';
  await browser.waitUntil(
    async () => {
      hit = (await downloadLines()).find((l) => re.test(l)) ?? '';
      return hit !== '';
    },
    { timeout, timeoutMsg: `no Downloads line matched ${re}` },
  );
  return hit;
}
