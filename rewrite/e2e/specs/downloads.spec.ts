import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  THROTTLED_DOWNLOAD_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Verbatim from design §8; `segments.ts` carries the same string. */
const LEGEND =
  'Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled';

/**
 * Stage `downloads`: this group's mock server is started with
 * `--throttle-ms 100` (see rewrite/scripts/e2e.sh's mock_args_for_group),
 * so content requests stream in ~20KB chunks with a 100ms gap between them.
 * Rom 301 ("Big Arcade Game", ~2MB — see mock-romm/server.mjs's
 * `BIG_CONTENT_BYTES`) is the fixture sized to actually span several of
 * those chunks — a comfortable, real in-flight download window to cancel,
 * long enough to outlast a full second `install()` round-trip through the
 * five-view shell. Rom 201 (Pac-Man) is small and used only to prove
 * queuing: its own download slot never opens until 301's does, regardless
 * of its size.
 *
 * The queue hands out entry ids in strict admission order and never reuses
 * one (see grid-core/src/library/queue.rs's `alloc_id`), so across this
 * spec's one fresh app instance the ids are deterministic: 1 = rom 301's
 * first install, 2 = rom 201's install, 3 = rom 301's retried install.
 *
 * The redesign (design §8) splits the view into three stacked segments and
 * gives every row a sparkline panel. Sampling is once per wall-clock second
 * and WebDriver round trips are hundreds of milliseconds, so nothing here
 * asserts a sample count or a drawn path — only that the graph element with
 * its two series exists and that rows sit in the right segment.
 */
describe('downloads', () => {
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

    // Install needs a library path before it will admit anything.
    await $(testId('library-path-input')).setValue(path.join(dataDir(), 'library'));
    await $(testId('library-path-save')).click();
    await $(testId('library-path-banner')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the library-path banner never hid after saving a path',
    });

    await $(testId('platform-btn-2')).click();
    await $(testId('game-card-301')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'platform 2 never rendered its game cards',
    });

    await showDownloads();
  });

  // The five views no longer stack (design §3): one pill click swaps which
  // root is displayed, so a spec has to be on the right view before it reads
  // text from it or clicks anything inside it. Every row assertion below
  // reads through the Downloads view, so `install` leaves it displayed.
  async function showDownloads() {
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  }

  async function install(cardTestId: string) {
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back',
    });
    await $(testId(cardTestId)).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('details-install')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await showDownloads();
  }

  /** The row with `id`, scoped to the segment it must live in right now. */
  function rowIn(segment: 'active' | 'queued' | 'completed', id: number) {
    return $(testId(`downloads-seg-${segment}`)).$(testId(`download-row-${id}`));
  }

  it('renders the three segments, their counts and the legend before anything runs', async () => {
    await expect($(testId('downloads-legend'))).toHaveText(LEGEND);
    for (const seg of ['active', 'queued', 'completed'] as const) {
      await expect($(testId(`downloads-seg-${seg}`))).toExist();
      await expect($(testId(`downloads-seg-count-${seg}`))).toHaveText('0');
    }
    await expect($(testId('downloads-graph-key'))).toExist();
  });

  it('starts the first install downloading in Active and queues the second in Queued', async () => {
    await install('game-card-301');
    await $(testId('download-row-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared for the first install',
    });
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Downloading'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the throttled download for rom 301 never entered the downloading state',
      },
    );
    await expect(rowIn('active', 1)).toExist();
    await expect($(testId('downloads-seg-count-active'))).toHaveText('1');
    // A base game carries no kind badge (design §8: "base none").
    expect(await $(testId('download-kind-1')).isExisting()).toBe(false);

    await install('game-card-201');
    await $(testId('download-row-2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared for the second install',
    });
    await expect($(testId('download-detail-2'))).toHaveText('Queued');
    await expect(rowIn('queued', 2)).toExist();
    await expect($(testId('downloads-seg-count-queued'))).toHaveText('1');
  });

  it('gives every row a sparkline panel with a network and a disk series', async () => {
    for (const id of [1, 2]) {
      const graph = $(testId(`download-graph-${id}`));
      await expect(graph).toExist();
      expect(await graph.getTagName()).toBe('svg');
      const paths = await graph.$$('path');
      expect(paths.length).toBe(2);
      expect(await paths[0].getAttribute('class')).toContain('net');
      expect(await paths[1].getAttribute('class')).toContain('disk');
      await expect($(testId(`download-graph-caption-${id}`))).toExist();
    }
  });

  it('shows the live transfer on the footer strip with its sparkline and opens the view from it', async () => {
    const strip = $(testId('downloads-footer'));
    await strip.waitForDisplayed({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the downloads strip never appeared for a live transfer',
    });
    // `⬇ <title> · <percent> · <speed>` (design §3). The percent is a
    // number while the total is known and an em dash otherwise; the speed
    // slot is a byte rate while downloading.
    expect(await $(testId('downloads-aggregate')).getText()).toMatch(
      /^⬇ Big Arcade Game · (\d{1,3}%|—) · [\d.]+ [KMGT]?B\/s$/,
    );
    await expect($(testId('downloads-footer-graph'))).toExist();
    expect(await strip.getText()).toContain('Open Downloads');

    await $(testId('nav-library')).click();
    await strip.click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking the strip did not open the Downloads view',
    });
  });

  it('cancels the active throttled download and moves it to Completed', async () => {
    await $(testId('download-action-cancel-1')).click();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()) === 'Cancelled',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cancelled download never showed the Cancelled status',
      },
    );
    // Only the row's segment is asserted here, not the Completed count:
    // cancelling frees the download slot, and entry 2 (small, unthrottled
    // past the chunk gap) can reach Completed within a WebDriver round trip.
    await expect(rowIn('completed', 1)).toExist();
  });

  it('retries the cancelled download and lets it complete', async () => {
    await $(testId('download-action-retry-1')).click();
    // Retry dismisses the old entry (id 1) and creates a fresh one (id 3 —
    // see the queue id-allocation note above).
    await $(testId('download-row-1')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await $(testId('download-row-3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the retried download never created a new row',
    });

    await browser.waitUntil(
      async () => (await $(testId('download-detail-3')).getText()).startsWith('Completed'),
      {
        // Entry 2 (rom 201) may still be occupying the download slot ahead
        // of this retry, plus the throttle itself, so this gets the
        // generous throttled-download budget.
        timeout: THROTTLED_DOWNLOAD_TIMEOUT,
        timeoutMsg: 'the retried download never reached Completed',
      },
    );
    await expect(rowIn('completed', 3)).toExist();
  });

  it('dismiss removes the completed row', async () => {
    await $(testId('download-action-dismiss-3')).click();
    await $(testId('download-row-3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the completed row was still there after dismissing it',
    });
  });

  // The other half of the strip's contract (design §3): it is always
  // mounted, and hides itself once no entry is in a live state. Entry 2
  // (rom 201) is the last one that can still be running by now, so this
  // gets the install budget rather than a transition one.
  it('hides the footer strip once nothing is live', async () => {
    await $(testId('downloads-footer')).waitForDisplayed({
      timeout: INSTALL_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the downloads strip stayed visible with no live transfer left',
    });
    await expect($(testId('downloads-seg-count-active'))).toHaveText('0');
    await expect($(testId('downloads-seg-count-queued'))).toHaveText('0');
  });
});
