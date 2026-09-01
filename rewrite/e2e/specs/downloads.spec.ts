import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  THROTTLED_DOWNLOAD_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `downloads`: this group's mock server is started with
 * `--throttle-ms 100` (see rewrite/scripts/e2e.sh's mock_args_for_group),
 * so content requests stream in ~20KB chunks with a 100ms gap between them.
 * Rom 301 ("Big Arcade Game", ~300KB) is the fixture sized to actually span
 * several of those chunks — a comfortable, real in-flight download window
 * to cancel. Rom 201 (Pac-Man) is small and used only to prove queuing: its
 * own download slot never opens until 301's does, regardless of its size.
 *
 * The queue hands out entry ids in strict admission order and never reuses
 * one (see grid-core/src/library/queue.rs's `alloc_id`), so across this
 * spec's one fresh app instance the ids are deterministic: 1 = rom 301's
 * first install, 2 = rom 201's install, 3 = rom 301's retried install.
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

    // Open the downloads drawer once, up front, and leave it open — every
    // row assertion below reads through it.
    await $(testId('downloads-footer')).click();
    await $(testId('downloads-drawer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads drawer never opened',
    });
  });

  async function install(cardTestId: string) {
    await $(testId(cardTestId)).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('details-install')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  }

  it('starts the first install downloading and queues the second behind it', async () => {
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

    await install('game-card-201');
    await $(testId('download-row-2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared for the second install',
    });
    await expect($(testId('download-detail-2'))).toHaveText('Queued');
  });

  it('cancels the active throttled download', async () => {
    await $(testId('download-action-cancel-1')).click();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()) === 'Cancelled',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cancelled download never showed the Cancelled status',
      },
    );
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
  });

  it('dismiss removes the completed row', async () => {
    await $(testId('download-action-dismiss-3')).click();
    await $(testId('download-row-3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the completed row was still there after dismissing it',
    });
  });
});
