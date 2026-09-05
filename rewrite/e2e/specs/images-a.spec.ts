import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Waits until the `<img>` at `selector` has decoded (naturalWidth > 0), not
 * just that some element exists at that selector — a `src` the asset
 * protocol rejects still gets assigned to the DOM node (see
 * library.spec.ts), and every image here loads asynchronously through
 * `Image.svelte`'s `api.ensureImage()` round trip, so a bare existence check
 * would be flaky. `document.querySelector` returning null (nothing there
 * yet, or a placeholder <div> instead of an <img>) reads as naturalWidth 0,
 * so this alone also covers "never appeared".
 */
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
 * Stage `images`, part A: connect, open rom 101's details (a real large
 * cover plus the merged-screenshots list, one entry of which is a foreign
 * URL the server-resolver filters out), install rom 101, and confirm its
 * cover renders on the Library grid. The seeded row (rom 102 — see
 * e2e/seed/images-seed.mjs) is left alone here; its image columns start
 * empty (a v1-schema db migrated on open) and are not asserted on until part
 * B, after a replenish pass has had a chance to run.
 *
 * Ends by flipping the mock into "offline" mode via `/__e2e__/offline` so
 * part B's app start finds the server unreachable — the pairing works the
 * same way as `connect-restore` and `install`: the embedded WebDriver
 * provider cannot restart the app inside one `wdio run`, so the runner
 * starts a fresh `wdio run` (and app process) for part B against the same
 * data dir and the same (still-running) mock.
 */
describe('images (a): cover, screenshots, install, library grid', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });
  });

  it('renders the large cover and the filtered screenshot list for rom 101', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();

    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for rom 101',
    });

    await waitForLoadedImage(
      testId('details-cover'),
      TRANSITION_TIMEOUT,
      "rom 101's details-cover",
    );
    await waitForLoadedImage(
      testId('details-screenshot-0'),
      TRANSITION_TIMEOUT,
      "rom 101's details-screenshot-0",
    );
    await waitForLoadedImage(
      testId('details-screenshot-1'),
      TRANSITION_TIMEOUT,
      "rom 101's details-screenshot-1",
    );
    // Fixture rom 101's third merged_screenshots entry is a foreign URL
    // (https://img.example/box-front.jpg); server_resolver filters any URL
    // whose host isn't the RomM server's own, so it never becomes a third
    // screenshot_urls entry and no details-screenshot-2 node ever exists.
    await expect($(testId('details-screenshot-2'))).not.toExist();

    await expect($(testId('details-description'))).toHaveText('A classic platformer.');

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('renders the redesigned popup: header, tabs, related, media viewer and file version', async () => {
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    // Design §7's right header: platform · year · developer · genres · rating.
    // The rating's star is an inline SVG (`Icon name="star"`), so it does not
    // appear in the element's text — only the number does.
    await expect($(testId('details-header-line'))).toHaveText(
      'Super Nintendo Entertainment System · 1990 · Nintendo · Platformer · 9.2',
    );
    await expect($(testId('details-verification'))).toHaveText('Identified');

    // All four §11 tab ids exist, and Overview is the one showing.
    for (const name of ['overview', 'media', 'saves', 'files']) {
      await expect($(testId(`details-tab-${name}`))).toExist();
    }
    await expect($(testId('details-description'))).toHaveText('A classic platformer.');
    await expect($(testId('details-meta-players'))).toHaveText('1');

    // Related is filtered to titles the platform actually holds: "Chrono
    // Trigger" is rom 102 (as "Chrono Trigger (USA)"), "A Game Nobody Owns"
    // is on no platform and must not appear.
    await $(testId('details-related-0')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Related row never rendered after the platform list loaded',
    });
    // The chip renders the title and its kind label; `toHaveText`
    // normalizes the whitespace between the two spans to one space.
    await expect($(testId('details-related-0'))).toHaveText('Chrono Trigger Similar');
    await expect($(testId('details-related-1'))).not.toExist();

    // Media: two screenshots plus the YouTube trailer tile.
    await $(testId('details-tab-media')).click();
    await $(testId('details-media-0')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-media-2'))).toExist();

    await $(testId('details-media-0')).click();
    await $(testId('media-viewer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the fullscreen media viewer never opened',
    });
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 1',
    );
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 2',
    );
    // Wrapping past the last item returns to the first (media.ts nextIndex).
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText('Super Mario World — trailer');
    await expect($(testId('media-viewer-youtube'))).toHaveAttribute(
      'src',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ',
    );
    await $(testId('media-viewer-next')).click();
    await expect($(testId('media-viewer-caption'))).toHaveText(
      'Super Mario World — screenshot 1',
    );

    // Esc closes the viewer and leaves the popup open.
    await browser.keys(['Escape']);
    await $(testId('media-viewer')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await expect($(testId('details-panel'))).toExist();

    // Files: D-UI-10 with no version tag in the name falls back to the
    // file's own last_modified date.
    await $(testId('details-tab-files')).click();
    await $(testId('details-file-1001')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-file-version-1001'))).toHaveText('2026-01-02');

    // Leave the session's remembered tab on Overview: it is module state,
    // so it would otherwise decide which tab the next case's popup opens on.
    await $(testId('details-tab-overview')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('installs rom 101', async () => {
    // images-seed.mjs already writes a non-empty library_path into
    // config.toml (it has to — the seeded rom 102 row's game.rom lives
    // under it), so, unlike install-a.spec.ts, the library-path banner is
    // never shown here.
    await expect($(testId('library-path-banner'))).not.toExist();

    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('details-install')).click();

    await $(`${testId('server-view')} ${testId('installed-badge-101')}`).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the installed badge never appeared on rom 101\'s card',
    });

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('renders a loaded cover for the newly installed rom 101 on the Library grid, and the seeded rom 102 row', async () => {
    await $(testId('nav-library')).click();
    await $(testId('library-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'library-card-101 never appeared after installing',
    });

    await waitForLoadedImage(
      `${testId('library-card-101')} img:not(.backdrop)`,
      TRANSITION_TIMEOUT,
      'library-card-101',
    );

    // The seeded row (rom 102): its cover may or may not have been
    // replenished yet by this point — only that it exists at all.
    await expect($(testId('library-card-102'))).toExist();
  });

  it('flips the mock into offline mode for part B', async () => {
    const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
      method: 'POST',
      body: JSON.stringify({ offline: true }),
    });
    expect(res.ok).toBe(true);
    const body = await res.json();
    expect(body).toEqual({ offline: true });
  });
});
