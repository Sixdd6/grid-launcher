/**
 * Shared steps for the desktop GOG specs (GameStores card, package E). They
 * run against package D's mock GOG server (`e2e/mock-romm/mock-gog.mjs`)
 * and fake tools (`e2e/fake-tools/`), wired by the gog stages of
 * `scripts/e2e.sh`. They rely only on the documented contract
 * (docs/stores-phase1-plan.md) and the UI's test ids, not on fixture titles:
 * a stage that seeds a GOG library with at least one installed and one
 * available game is enough.
 */
import { TRANSITION_TIMEOUT } from './env.js';
import { connectRomm as connectRommStore } from './romm.js';

export const testId = (id: string) => `[data-testid="${id}"]`;
export const testIdPrefix = (id: string) => `[data-testid^="${id}"]`;

/**
 * Connects to the mock RomM server (the stage's GOG state comes from its
 * seed): through the welcome's RomM row on the seed's fresh profile, else
 * Settings › Game stores (`helpers/romm.ts`). Ends in the Library.
 */
export async function connectRomm(): Promise<void> {
  await connectRommStore();
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
export function storeCards() {
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
