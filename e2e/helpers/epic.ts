/**
 * Shared steps for the desktop Epic Games specs (GameStores card, Phase 2
 * package D). They run against package C's mock Epic server
 * (`e2e/mock-romm/mock-epic.mjs`), fake legendary
 * (`e2e/fake-tools/legendary.mjs`) and seed (`e2e/seed/epic-seed.mjs`: signed
 * in as SamPlays, Starlight Courier and Vector Bloom installed, the
 * prototype's six games owned), wired by the epic stages of `scripts/e2e.sh`
 * (docs/stores-phase2-plan.md, "E2E plan").
 *
 * The seed's app names are package C's choice, so the specs find covers by
 * title, never by app name.
 */
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from './env.js';
import { testId, testIdPrefix } from './gog.js';

export { testId, testIdPrefix };

/** The titles the seed gives (the prototype's `stores.js` e1–e6). */
export const EPIC = {
  installed: ['Starlight Courier', 'Vector Bloom'],
  available: ['Glimmer Deep', 'Iron Orchard', 'Northern Static', 'Rustbelt Rally'],
  notSupported: { 'Iron Orchard': 'the EA app', 'Northern Static': 'Ubisoft Connect' } as Record<string, string>,
  account: 'SamPlays',
} as const;

/**
 * Brings the app to the Library: connects to the stage's mock RomM server
 * when the connect form shows (the Epic seed may also start the profile
 * without a server, in which case the Library is already there).
 */
export async function startApp(): Promise<void> {
  await browser.waitUntil(
    async () => (await $(testId('connect-server-url')).isExisting()) || (await $(testId('library-section')).isExisting()),
    { timeout: APP_START_TIMEOUT, timeoutMsg: 'neither the connect form nor the Library appeared' },
  );
  if (await $(testId('connect-server-url')).isExisting()) {
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
  }
  await $(testId('library-section')).waitForExist({ timeout: TRANSITION_TIMEOUT });
}

/** Opens Library › Epic Games › <part>. */
export async function openEpic(part: 'installed' | 'available'): Promise<void> {
  const pill = await $('button=Library');
  if (await pill.isExisting()) await pill.click();
  const row = await $(testId(`library-rail-store-epic-${part}`));
  await row.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Library › Epic Games › ${part} never appeared in the rail` });
  await row.click();
  await expect($(testId('library-title'))).toHaveText(expect.stringContaining('Epic Games'));
}

/** The Epic Games covers on screen, in grid order. */
export function epicCards() {
  return $$(testIdPrefix('library-store-card-epic-'));
}

/** Every Epic cover's title, in grid order. */
export async function epicTitles(): Promise<string[]> {
  const out: string[] = [];
  for (const card of await epicCards()) out.push((await card.$('.label').getText()).trim());
  return out;
}

/**
 * The cover with `title`, and its badge (`epic-<app name>`, the suffix of
 * its test ids). Waits for it: the owned list loads after sign-in.
 */
export async function epicCard(title: string): Promise<{ card: WebdriverIO.Element; badge: string }> {
  let found: WebdriverIO.Element | null = null;
  await browser.waitUntil(
    async () => {
      for (const card of await epicCards()) {
        if ((await card.$('.label').getText()).trim() === title) {
          found = card;
          return true;
        }
      }
      return false;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no Epic Games cover titled ${title}` },
  );
  const card = found as unknown as WebdriverIO.Element;
  const badge = ((await card.getAttribute('data-testid')) ?? '').replace('library-store-card-', '');
  return { card, badge };
}

/** The tag texts on a cover (`38%`, `UPDATE`, `SAVE CONFLICT`, `NOT SUPPORTED`). */
export async function coverTags(badge: string): Promise<string[]> {
  const tags = await $$(`${testId(`library-store-card-${badge}`)} ${testId(`card-tag-${badge}`)}`);
  return tags.map((t) => t.getText());
}

/** Opens Details for the cover with `title`. */
export async function openEpicDetails(title: string): Promise<string> {
  const { card, badge } = await epicCard(title);
  await card.click();
  await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  return badge;
}

/** Closes the topmost dialog (Escape) and waits for `id` to go. */
export async function closeWithEscape(id: string): Promise<void> {
  await browser.keys('Escape');
  await $(testId(id)).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
}

/** The lines under the Details buttons, by test id. */
export async function detailsLine(id: string): Promise<string> {
  return (await $(testId(id)).getText()).trim();
}
