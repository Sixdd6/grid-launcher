import { existsSync, readdirSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `content`: the two "extra content" console flows, which share one
 * server endpoint and are told apart only by `file_ids`
 * (`content_job`, grid-core/src/library/mod.rs) — the mock resolves that
 * pair back to a fixture file (`resolveContentKey`, mock-romm/server.mjs).
 *
 * PlayStation 4 (rom 501) is USER-driven: the base install completes, the
 * `Install Update` button appears because the rom detail lists an
 * update-category file, and clicking it merges that archive's title-id tree
 * into the installed one.
 *
 * Xbox 360 (rom 601) is AUTOMATIC: `queue_xbox360_content` admits the
 * update job itself as the base install finalizes, so the second row appears
 * with no click at all. Its STFS package is copied to Xenia's content root,
 * which the seeded portable stub puts at `<stubs>/xenia-edge/content`.
 *
 * The queue hands out ids in strict admission order and never reuses one, so
 * across this spec's single app instance: 1 = 501 base, 2 = 501 update,
 * 3 = 601 base, 4 = 601 update.
 */
describe('content', () => {
  const library = () => path.join(dataDir(), 'library');

  /** Waits for a drawer row to reach `Completed`. */
  async function waitCompleted(id: number, what: string) {
    await browser.waitUntil(
      async () => (await $(testId(`download-detail-${id}`)).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: `${what} never reached Completed` },
    );
  }

  /** The title text of one drawer row. */
  async function rowTitle(id: number): Promise<string> {
    return $(`${testId(`download-row-${id}`)} .title`).getText();
  }

  async function openDetails(romId: number) {
    await $(testId(`game-card-${romId}`)).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId(`game-card-${romId}`)).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the details overlay never opened for rom ${romId}`,
    });
  }

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

    // The seed already wrote library_path, so no library-path banner step.
    await $(testId('downloads-footer')).click();
    await $(testId('downloads-drawer')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads drawer never opened',
    });
  });

  it('installs the PS4 base game and then offers Install Update', async () => {
    await $(testId('platform-btn-1')).click();
    await openDetails(501);
    await $(testId('details-install')).click();
    await waitCompleted(1, 'the PS4 base install');

    await $(testId('details-install-update')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'Install Update never appeared for the PS4 game',
    });
  });

  it('applies the PS4 update into the installed title-id tree', async () => {
    await $(testId('details-install-update')).click();

    await $(testId('download-row-2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no drawer row appeared for the PS4 update job',
    });
    expect(await rowTitle(2)).toBe('PS4 Game (update)');
    await waitCompleted(2, 'the PS4 update');

    // The update merges into the BASE install's extraction directory, whose
    // name follows `extraction_dir(archive)` — the archive stem, i.e. the
    // fs_name `ps4-base.zip` without its extension. Globbing the platform
    // directory rather than hardcoding that keeps the assertion honest if
    // the naming rule ever changes.
    const platformDir = path.join(library(), 'PlayStation 4');
    const merged = readdirSync(platformDir, { withFileTypes: true })
      .filter((e) => e.isDirectory())
      .map((e) => path.join(platformDir, e.name, 'CUSA12345', 'patch.txt'))
      .filter((p) => existsSync(p));
    expect(merged.length).toBe(1);
  });

  it('queues the Xbox 360 update by itself once the base install finalizes', async () => {
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });

    await $(testId('platform-btn-2')).click();
    await openDetails(601);
    await $(testId('details-install')).click();
    await waitCompleted(3, 'the Xbox 360 base install');

    // No click: grid-core's `queue_xbox360_content` admitted this row from
    // inside the base install's finalize step.
    await $(testId('download-row-4')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the Xbox 360 update was never queued automatically',
    });
    expect(await rowTitle(4)).toBe('Xbox Game (update)');
    await waitCompleted(4, 'the Xbox 360 update');
  });

  it("copies the STFS package into Xenia's content root", () => {
    const contentRoot = path.join(dataDir(), 'stubs', 'xenia-edge', 'content');
    expect(
      existsSync(
        path.join(contentRoot, '0000000000000000', '415608C3', '000B0000', 'tu00000001'),
      ),
    ).toBe(true);
  });
});
