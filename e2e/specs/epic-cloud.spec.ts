import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { dataDir, INSTALL_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { closeWithEscape, coverTags, detailsLine, epicCard, openEpic, openEpicDetails, startApp, testId } from '../helpers/epic.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `epic-cloud` (GameStores README, "Cloud saves tab", "Launch";
 * docs/stores-phase2-plan.md "E2E plan"), seeded with `--cloud-conflict`:
 * Starlight Courier's cloud save is newer than its last sync and its local
 * save changed too. The cover says SAVE CONFLICT; the Cloud saves tab shows
 * both sides (Epic's list-saves gives no file list, so the cloud side has no
 * file count); Play asks "Which save should … use?"; Don't launch leaves it;
 * Use the cloud save copies the local save to
 * `<data>/stores/save-backups/epic/<app>/<epoch-ms>/` before the download,
 * then launches; afterwards the game is synced and Sync now works. Needs
 * packages A–C (fake legendary's cloud folder, the seed): it goes green only
 * after they are merged.
 */
const TITLE = 'Starlight Courier';

describe('epic-cloud', () => {
  let badge = '';

  before(async () => {
    await startApp();
    await openEpic('installed');
    badge = (await epicCard(TITLE)).badge;
  });

  it('tags the cover SAVE CONFLICT and shows the cloud button', async () => {
    let tags: string[] = [];
    // The state is read in the background after sign-in, so the tag can come a moment later.
    await browser
      .waitUntil(
        async () => {
          tags = await coverTags(badge);
          return tags.includes('SAVE CONFLICT');
        },
        { timeout: TRANSITION_TIMEOUT },
      )
      .catch(() => undefined);
    expect(tags).toContain('SAVE CONFLICT');
  });

  it('shows both sides on the Cloud saves tab, the cloud side without a file count', async () => {
    await openEpicDetails(TITLE);
    expect(await detailsLine('store-details-cloud')).toBe('Cloud save conflict · Resolve');
    await $(testId('store-details-tab-cloud')).click();
    const tab = $(testId('store-details-cloud-tab'));
    await expect(tab).toHaveText(expect.stringContaining('These saves changed in two places'));
    await expect(tab).toHaveText(expect.stringContaining('Both this computer’s save and the one in your Epic Games account changed since the last sync.'));
    await expect(tab).toHaveText(expect.stringContaining('Epic Games cloud'));
    await expect(tab).toHaveText(expect.stringContaining('Uploads it to Epic Games.'));
    // "Changed … on another computer · 2.0 MB": a size, never a guessed file count.
    await expect(tab).toHaveText(expect.stringContaining('on another computer'));
    await expect(tab).not.toHaveText(expect.stringMatching(/on another computer · \d+ files?/));
    await expect(tab).toHaveText(expect.stringContaining('Epic Games says where the game keeps its saves. RomM cloud saves are off for store games.'));
    await closeWithEscape('store-details');
  });

  it('asks which save to use before launching; Don’t launch leaves the conflict', async () => {
    await openEpic('installed');
    await $(testId(`library-store-card-${badge}`)).moveTo();
    await $(testId(`card-primary-${badge}`)).click();
    await $(testId('store-conflict-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('store-conflict-dialog'))).toHaveText(expect.stringContaining(`Which save should ${TITLE} use?`));
    await expect($(testId('store-conflict-later'))).toHaveText('Don’t launch');
    await $(testId('store-conflict-later')).click();
    await $(testId('store-conflict-dialog')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    expect(await coverTags(badge)).toContain('SAVE CONFLICT');
  });

  it('Use the cloud save keeps a copy of the local save, restores the cloud one and launches', async () => {
    const app = badge.replace(/^epic-/, '');
    const backups = join(dataDir(), 'stores', 'save-backups', 'epic', app);
    const before = existsSync(backups) ? readdirSync(backups).length : 0;
    await $(testId(`library-store-card-${badge}`)).moveTo();
    await $(testId(`card-primary-${badge}`)).click();
    await $(testId('store-conflict-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(`${testId('store-conflict-dialog')} ${testId('store-conflict-cloud')}`).click();
    await waitForToast(`Restored the cloud save for ${TITLE}`, { timeout: INSTALL_TIMEOUT });
    await waitForToast(`Launching ${TITLE}…`, { timeout: INSTALL_TIMEOUT });
    // The local save was copied aside before the download replaced it.
    await browser.waitUntil(async () => existsSync(backups) && readdirSync(backups).length > before, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `no save-backups copy under ${backups}`,
    });
    // After the session, the upload card names the store.
    await waitForToast(`Cloud saves uploaded to Epic Games for ${TITLE}`, { timeout: INSTALL_TIMEOUT * 2 });
    expect(await coverTags(badge)).not.toContain('SAVE CONFLICT');
  });

  it('shows the synced line and syncs on Sync now', async () => {
    await openEpic('installed');
    await openEpicDetails(TITLE);
    expect(await detailsLine('store-details-cloud')).toMatch(/^Cloud saves synced /);
    await $(testId('store-details-tab-cloud')).click();
    await expect($(testId('store-details-cloud-tab'))).toHaveText(expect.stringContaining('GRID syncs with Epic Games before the game starts and after it closes.'));
    await $(testId('store-details-sync')).click();
    await waitForToast(`Cloud saves synced for ${TITLE}`, { timeout: INSTALL_TIMEOUT });
    await closeWithEscape('store-details');
  });

  it('says a game without cloud saves has none', async () => {
    await openEpicDetails('Vector Bloom');
    expect(await detailsLine('store-details-cloud')).toBe('This game has no cloud saves');
    await $(testId('store-details-tab-cloud')).click();
    await expect($(testId('store-details-cloud-tab'))).toHaveText(expect.stringContaining('Epic Games doesn’t keep saves for it.'));
    await closeWithEscape('store-details');
  });
});
