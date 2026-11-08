/**
 * The notification stack (Toast.svelte) keeps up to four cards, each for ten
 * seconds or more, so `$(testId('toast'))` (the FIRST card) can be an older,
 * unrelated message. These read every card instead.
 */
import { TRANSITION_TIMEOUT } from './env.js';

const TOAST = '[data-testid="toast"]';

/** Every card's message text, oldest first. A card closing mid-read is skipped. */
export async function toastTexts(): Promise<string[]> {
  const texts: string[] = [];
  for (const el of await $$(TOAST)) {
    try {
      // An error card's screen-reader prefix ("Error: ") is part of its
      // text to WebDriver; the message is what follows it.
      texts.push((await el.getText()).trim().replace(/^Error:\s*/, ''));
    } catch {
      // The card closed between the lookup and the read.
    }
  }
  return texts;
}

/**
 * Waits until some card's message is exactly `expected` (a string) or matches
 * it (a RegExp), and returns that text. Fails with `timeoutMsg` plus the
 * texts that were on screen.
 */
export async function waitForToast(
  expected: string | RegExp,
  { timeout = TRANSITION_TIMEOUT, timeoutMsg = 'the expected notification never appeared' } = {},
): Promise<string> {
  const matches = (text: string) =>
    typeof expected === 'string' ? text === expected : expected.test(text);
  let seen: string[] = [];
  let found: string | undefined;
  await browser
    .waitUntil(
      async () => {
        seen = await toastTexts();
        found = seen.find(matches);
        return found !== undefined;
      },
      { timeout, interval: 100 },
    )
    .catch(() => undefined);
  if (found === undefined) {
    throw new Error(`${timeoutMsg} (wanted ${String(expected)}; cards: ${JSON.stringify(seen)})`);
  }
  return found;
}

/**
 * Closes every card on screen, for a spec about to click something the
 * bottom-right stack may sit over. A card closing on its own mid-loop is
 * skipped.
 */
export async function dismissToasts(): Promise<void> {
  for (const button of await $$('[data-testid="toast-dismiss"]')) {
    try {
      await button.click();
    } catch {
      // Already gone.
    }
  }
  await browser
    .waitUntil(async () => (await $$(TOAST)).length === 0, { timeout: TRANSITION_TIMEOUT, interval: 100 })
    .catch(() => undefined);
}
