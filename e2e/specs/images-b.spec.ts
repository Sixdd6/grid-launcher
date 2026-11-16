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
 * unreachable server — the shell opens with RomM's Library group flagged
 * Can't reach and the selection on RomM › Installed (romm-storefront-plan decision 6: no chip), and rom 101's already-cached cover (fetched during part A)
 * still renders with no server round trip needed.
 *
 * Flipping the mock back online and clicking Retry is what finally lets the
 * seeded rom 102 row (migrated v1 -> v2 on this same open, with empty image
 * columns until now) get replenished: RomM › Installed's Retry only
 * spawns the replenish job on a SUCCESSFUL reconnect, which fetches rom 102's detail and
 * its grid cover (the large one, else — as for rom 102 — the small one).
 */
describe('images (b): offline startup, cached cover, retry, replenish', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the shell never appeared — the app did not reach a usable state',
    });
  });

  it('starts on the Library section, disconnected, with rom 101\'s cover already cached', async () => {
    await browser.waitUntil(
      async () => (await $(testId('library-rail-romm-installed-flag')).getAttribute('title').catch(() => null)) === "Can't reach RomM",
      { timeout: APP_START_TIMEOUT, timeoutMsg: "the RomM group never flagged Can't reach" },
    );
    await expect($(testId('library-section'))).toBeDisplayed();
    // Connecting at startup opened RomM › Discover; the server didn't answer,
    // so the selection fell back to RomM › Installed. Rom 101 is in All games.
    await expect($(testId('library-rail-romm-installed'))).toHaveAttribute('aria-current', 'page');
    await $(testId('library-rail-all')).click();

    await $(testId('library-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'library-card-101 never appeared — the installed store never loaded',
    });
    await waitForLoadedImage(
      `${testId('library-card-101')} img:not(.backdrop)`,
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

  it("shows Can't reach RomM on RomM › Installed", async () => {
    await $(testId('library-rail-romm-installed')).click();
    await $(testId('romm-banner')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "RomM › Installed never showed the Can't reach banner",
    });
    await expect($(testId('romm-banner'))).toHaveAttribute('data-kind', 'unreachable');
    await expect($(testId('romm-banner-action'))).toHaveText('Retry');
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
  });

  it('reconnects via Retry once the mock comes back, and replenishes the seeded rom 102 cover', async () => {
    const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
      method: 'POST',
      body: JSON.stringify({ offline: false }),
    });
    expect(res.ok).toBe(true);

    // Retry lives on RomM › Installed's banner (and the Settings card).
    await $(testId('romm-banner-action')).click();
    await $(testId('library-rail-romm-discover')).waitForExist({
      timeout: TRANSITION_TIMEOUT * 2,
      timeoutMsg: 'Retry never reconnected RomM',
    });
    await $(testId('library-rail-platform-snes')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the RomM group never listed a platform after retrying',
    });
    await expect($(testId('romm-banner'))).not.toExist();

    await $(testId('library-rail-all')).click();
    // The migrated-then-replenished seeded row (rom 102): its cover only
    // appears once the post-retry replenish job has fetched rom 102's
    // detail (finding empty image columns from the v1->v2 migration) and
    // then its cover file (rom 102 has only a small one) — a detail fetch plus a cover fetch, so
    // this gets the more generous install-scale timeout.
    await waitForLoadedImage(
      `${testId('library-card-102')} img:not(.backdrop)`,
      INSTALL_TIMEOUT,
      'library-card-102 (post-replenish)',
    );
  });
});
