import { APP_START_TIMEOUT, INSTALL_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** See images-a.spec.ts — identical helper, kept local to avoid a shared
 * import between two files the runner treats as independent `wdio run`s. */
async function waitForLoadedImage(selector: string, timeout: number, label: string) {
  await browser.waitUntil(
    async () => {
      const width = await browser.execute((sel: string) => {
        const el = document.querySelector(sel) as HTMLImageElement | null;
        return el?.naturalWidth ?? 0;
      }, selector);
      return width > 0;
    },
    { timeout, timeoutMsg: `${label} never rendered a loaded <img> (naturalWidth stayed 0)` },
  );
}

/**
 * Stage `images`, part B: a second launch of the same binary against the
 * data dir part A left behind, with the mock still in "offline" mode (part
 * A's last step). `restore_session` finds a stored session but an
 * unreachable server (`RestoreOutcome::Unreachable`) — R2's startup routing
 * puts the shell on the Library section with the chip reading "Not
 * connected", and rom 101's already-cached cover (fetched during part A)
 * still renders with no server round trip needed.
 *
 * Flipping the mock back online and clicking Retry is what finally lets the
 * seeded rom 102 row (migrated v1 -> v2 on this same open, with empty image
 * columns until now) get replenished: `retry_connect` only spawns the
 * replenish job on a SUCCESSFUL retry, which fetches rom 102's detail and
 * its small cover.
 */
describe('images (b): offline startup, cached cover, retry, replenish', () => {
  before(async () => {
    await $(testId('session-chip')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared — the app did not reach a usable state',
    });
  });

  it('starts on the Library section, disconnected, with rom 101\'s cover already cached', async () => {
    await expect($(testId('session-chip'))).toHaveText('Not connected');
    await expect($(testId('library-section'))).toBeDisplayed();

    await $(testId('library-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'library-card-101 never appeared — the installed store never loaded',
    });
    await waitForLoadedImage(
      `${testId('library-card-101')} img`,
      TRANSITION_TIMEOUT,
      'library-card-101 (offline, from the disk cache)',
    );
  });

  it('shows Play (already installed) with no Install button for rom 101, offline', async () => {
    await $(testId('library-card-101')).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for the installed rom 101',
    });

    await expect($(testId('details-play'))).toBeEnabled();
    await expect($(testId('details-install'))).not.toExist();

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('shows the offline state on the Server section', async () => {
    await $(testId('nav-server')).click();
    await $(testId('server-offline')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'server-offline never appeared on the Server section',
    });
  });

  it('reconnects via Retry once the mock comes back, and replenishes the seeded rom 102 cover', async () => {
    const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
      method: 'POST',
      body: JSON.stringify({ offline: false }),
    });
    expect(res.ok).toBe(true);

    await $(testId('session-retry')).click();

    await browser.waitUntil(
      async () => (await $(testId('session-chip')).getText()).startsWith('e2euser @'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the session chip never showed the connected e2euser@host label after Retry',
      },
    );
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after retrying',
    });

    await $(testId('nav-library')).click();
    // The migrated-then-replenished seeded row (rom 102): its cover only
    // appears once the post-retry replenish job has fetched rom 102's
    // detail (finding empty image columns from the v1->v2 migration) and
    // then its small cover file — a detail fetch plus a cover fetch, so
    // this gets the more generous install-scale timeout.
    await waitForLoadedImage(
      `${testId('library-card-102')} img`,
      INSTALL_TIMEOUT,
      'library-card-102 (post-replenish)',
    );
  });
});
