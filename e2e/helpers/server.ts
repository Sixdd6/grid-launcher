/**
 * The Server view (app/src/lib/Server.svelte). It opens on Server › Discover,
 * the first rail row, so a spec that wants a platform's grid selects the
 * platform first.
 */
import { TRANSITION_TIMEOUT } from './env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Selects the platform `platform-btn-<id>` in the Server rail and waits for
 * its header. The Server view must be the one on screen.
 */
export async function openPlatform(id: number | string): Promise<void> {
  const button = $(testId(`platform-btn-${id}`));
  await button.waitForClickable({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the Server rail never offered platform ${id}`,
  });
  await button.click();
  await $(testId('server-platform-header')).waitForExist({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `platform ${id}'s header never appeared`,
  });
}

/** Selects Server › Discover and waits for it. */
export async function openDiscover(): Promise<void> {
  await $(testId('server-rail-discover')).click();
  await $(testId('discover')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Server › Discover never appeared',
  });
}
