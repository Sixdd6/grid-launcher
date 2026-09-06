import { createHash } from 'node:crypto';
import {
  APP_START_TIMEOUT,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `library`: connect once (this group has no pre-connected fixture
 * data of its own — it starts from a fresh data dir like `connect` does),
 * then exercise the library grid: platforms, game cards, cover rendering,
 * and keyboard focus movement.
 *
 * Fixture ids relied on here (see rewrite/e2e/fixtures/): platform 1 (SNES)
 * has roms 101 (has a cover), 102 (server `name: null`, falls back to
 * `fs_name_no_ext`), 103 (no cover); platform 2 (Arcade) has at least 201.
 */
describe('library', () => {
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

  it('renders both platforms from the fixtures', async () => {
    await expect($(testId('platform-btn-1'))).toExist();
    await expect($(testId('platform-btn-2'))).toExist();
  });

  it('selecting platform 1 shows its games, including the null-name game rendering its fs_name_no_ext', async () => {
    await $(testId('platform-btn-1')).click();

    await $(testId('game-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'platform 1 never rendered its game cards',
    });
    await expect($(testId('game-card-102'))).toExist();
    await expect($(testId('game-card-103'))).toExist();

    // rom 102 has a null `name` server-side; the app must fall back to
    // fs_name_no_ext ("Chrono Trigger (USA)") rather than showing a blank
    // title. Opening the details overlay is what renders the resolved name.
    await $(testId('game-card-102')).click();
    const panel = $(testId('details-panel'));
    await panel.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the details overlay never opened for the null-name game',
    });
    await expect(panel.$('h2')).toHaveText('Chrono Trigger (USA)');
    await $(testId('details-close')).click();
    await panel.waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  // This is the regression test for the asset-protocol scope fix (Task 6's
  // ruled-in addition to lib.rs): before it, every cover request failed with
  // "asset protocol not configured to allow the path" because the static
  // scope in tauri.conf.json only ever covered the default ProjectDirs cache
  // dir, never GRID_LAUNCHER_DATA_DIR's <data dir>/covers. naturalWidth > 0
  // is checked, not just a non-empty `src`, because a src the asset
  // protocol rejects still gets *assigned* to the <img> — only a real,
  // successfully decoded image has a nonzero natural size.
  //
  // The mock's per-run request log (rewrite/e2e/last-run-requests.log) is
  // NOT asserted on here: the mock only flushes it from close(), which the
  // runner calls after every spec in this group has finished, so it is not
  // reachable mid-run. naturalWidth is strictly the stronger check anyway —
  // it proves the bytes were actually decoded by the webview through the
  // asset protocol, not merely that a request reached the mock.
  it('renders a real, loaded cover image for a game that has one', async () => {
    await $(testId('platform-btn-1')).click();
    const img = $(`${testId('game-card-101')} img:not(.backdrop)`);
    await img.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the cover <img> never appeared for rom 101',
    });

    const src = await img.getAttribute('src');
    expect(src).toBeTruthy();

    const naturalWidth = await browser.execute((testIdAttr: string) => {
      const el = document.querySelector(`${testIdAttr} img:not(.backdrop)`) as HTMLImageElement | null;
      return el?.naturalWidth ?? 0;
    }, testId('game-card-101'));
    expect(naturalWidth).toBeGreaterThan(0);
  });

  it('ArrowRight moves the focused card', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    // The grid focuses index 0 (rom 101) whenever a platform is (re)selected.
    await browser.waitUntil(
      async () => ((await $(testId('game-card-101')).getAttribute('class')) ?? '').includes('focused'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'game-card-101 was never the initially focused card',
      },
    );

    await browser.keys(['ArrowRight']);

    await browser.waitUntil(
      async () => ((await $(testId('game-card-102')).getAttribute('class')) ?? '').includes('focused'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'ArrowRight never moved focus onto game-card-102',
      },
    );
    const card101Class = await $(testId('game-card-101')).getAttribute('class');
    expect(card101Class).not.toContain('focused');
  });

  // The hover overlay (D-UI-9) raises a centred Install button and a bottom
  // action row over the cover. WebdriverIO clicks an element's CENTRE, so
  // if either sat under that point every `game-card-<id>` click in the
  // suite would install instead of opening Details. `cards/size.ts` keeps
  // the band around the card root's centre free; this is that contract's
  // end-to-end check.
  it('opens Details, not the hover action, when the card itself is clicked', async () => {
    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-103')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-103')).click();

    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking the card centre did not open the details popup',
    });
    // Nothing was queued: the click never reached the Install action.
    expect(await $(testId('details-install')).isExisting()).toBe(true);
    await $(testId('details-close')).click();
  });

  // Round 4: the shell background is no longer the raw cover blurred by the
  // compositor — the backend builds one 960px, pre-blurred `<key>.bg<sigma>.jpg`
  // (`ensure_background_variant`) and the layer composites that. Opening rom
  // 101's details reports its subject synchronously (`noteViewed`), so no
  // hover dwell has to be simulated here.
  it('paints the pre-blurred background variant after a game is viewed', async () => {
    // The previous case closed the popup without waiting; the overlay would
    // otherwise swallow the platform-button click below.
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });

    await $(testId('platform-btn-1')).click();
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });

    // A loose `.bg` check would pass even if the wrong source image (say,
    // the cover) got blurred: `subjectFromDetails` prefers fanart over
    // screenshots and the cover (background.ts), and rom 101's fanart_path
    // ("roms/1/101/fanart/fanart.png") only resolves to a real, fetchable
    // URL because of resolve_image_url's relative-path fix (Task 1) — before
    // that fix it joined onto the SPA root and 404'd. `ensure_background_variant`
    // names the variant `<sha256 of the resolved URL>.bg<sigma>.jpg`
    // (images/background.rs, images/cache.rs's `image_key`), so hashing the
    // exact resolved URL and matching that prefix proves the fanart's URL
    // specifically made it all the way through, not just any image.
    const fanartKey = createHash('sha256')
      .update(`${mockUrl()}/assets/romm/resources/roms/1/101/fanart/fanart.png`)
      .digest('hex');
    await browser.waitUntil(
      async () => {
        const images = await browser.execute(() =>
          Array.from(document.querySelectorAll('[data-testid="background-art"] .layer'))
            .filter((el) => el.classList.contains('visible'))
            .map((el) => (el as HTMLElement).style.backgroundImage)
            .join(' '),
        );
        return images.includes(`${fanartKey}.bg`);
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: "the visible background layer never showed rom 101's fanart variant",
      },
    );

    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
});
