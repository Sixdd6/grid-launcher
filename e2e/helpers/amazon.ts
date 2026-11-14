/**
 * Shared steps for the Amazon Games specs (GameStores and TVStores cards,
 * Phase 3 package D). They run against package C's mock Amazon server
 * (`e2e/mock-romm/mock-amazon.mjs`), fake nile (`e2e/fake-tools/nile.mjs`)
 * and seed (`e2e/seed/amazon-seed.mjs`: signed in as sam.plays, Fogline
 * installed, Kettle Drum and Lowsun owned; `--expired` starts with the login
 * expired), wired by the amazon stages of `scripts/e2e.sh`
 * (docs/stores-phase3-plan.md, "E2E plan").
 *
 * Amazon product ids hold dots (`amzn1.adg.product.<uuid>`) and are package
 * C's choice, so the specs find covers by title, never by id.
 */
import { TRANSITION_TIMEOUT } from './env.js';
import { closeWithEscape, coverTags, detailsLine, startApp } from './epic.js';
import { testId, testIdPrefix } from './gog.js';

export { closeWithEscape, coverTags, detailsLine, startApp, testId, testIdPrefix };

/** The titles and account the seed gives (the prototype's `stores.js` a1–a3). */
export const AMAZON = {
  installed: ['Fogline'],
  available: ['Kettle Drum', 'Lowsun'],
  account: 'sam.plays',
  noBackup: 'Amazon Games games can’t be backed up: nile installs only from Amazon’s servers.',
} as const;

/** Opens Library › Amazon Games › <part>. */
export async function openAmazon(part: 'installed' | 'available'): Promise<void> {
  const pill = await $('button=Library');
  if (await pill.isExisting()) await pill.click();
  const row = await $(testId(`library-rail-store-amazon-${part}`));
  await row.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Library › Amazon Games › ${part} never appeared in the rail` });
  await row.click();
  await expect($(testId('library-title'))).toHaveText(expect.stringContaining('Amazon Games'));
}

/** The Amazon Games covers on screen, in grid order. */
export function amazonCards() {
  return $$(testIdPrefix('library-store-card-amazon-'));
}

/** Every Amazon cover's title, in grid order. */
export async function amazonTitles(): Promise<string[]> {
  const out: string[] = [];
  for (const card of await amazonCards()) out.push((await card.$('.label').getText()).trim());
  return out;
}

/**
 * The cover with `title`, and its badge (`amazon:<product id>` made a test
 * id suffix). Waits for it: the owned list loads after sign-in.
 */
export async function amazonCard(title: string): Promise<{ card: WebdriverIO.Element; badge: string }> {
  let found: WebdriverIO.Element | null = null;
  await browser.waitUntil(
    async () => {
      for (const card of await amazonCards()) {
        if ((await card.$('.label').getText()).trim() === title) {
          found = card;
          return true;
        }
      }
      return false;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no Amazon Games cover titled ${title}` },
  );
  const card = found as unknown as WebdriverIO.Element;
  const badge = ((await card.getAttribute('data-testid')) ?? '').replace('library-store-card-', '');
  return { card, badge };
}

/** Opens Details for the cover with `title`; returns its badge. */
export async function openAmazonDetails(title: string): Promise<string> {
  const { card, badge } = await amazonCard(title);
  await card.click();
  await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  return badge;
}

/** Whether the store currently shows as expired (the seed's `--expired` start). */
export async function amazonExpired(): Promise<boolean> {
  return $(testId('store-chip-expired')).isExisting();
}
