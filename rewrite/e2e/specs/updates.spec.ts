import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** The self-update tag `mock-romm/mock-forge.mjs` serves for this repo. */
const SELF_UPDATE_TAG = 'v9.9.9-e2e';

/**
 * Stage `updates` (doc 10): the two things the launcher calls an "update" —
 * a newer copy of an installed GAME on the RomM server, and a newer release
 * of the LAUNCHER itself on GitHub — from the badge on the Library card to
 * the bytes on disk.
 *
 * Everything here starts from a pre-seeded registry
 * (`e2e/seed/updates-seed.mjs`) rather than a UI install: four installed
 * games whose stored `rom_file_name`/`server_updated_at` pair against
 * `e2e/fixtures-updates/rom-details.json` to hit every branch of
 * `game_has_server_update` (grid-core/src/library/update_detection.rs).
 *
 *   801 "Old Rom"     update by the TIMESTAMP rule; button "Update"
 *   802 "My Game"     update by the Windows FILE-NAME rule; "Update to v1.1.0"
 *   803 "Current Rom" identical to the server: no update
 *   804 "Ghost Rom"   no rom detail at all (the mock 404s): no update
 *
 * The two update paths are genuinely different code:
 * 801 re-downloads and re-extracts (`install_update`), 802 MERGES the new
 * archive over the installed tree (`install_native_update`) — which is why
 * the seeded `saves/slot1.sav` still has to be there afterwards.
 *
 * The launcher's own check runs once at startup and only in this group:
 * `GRID_LAUNCHER_E2E_UPDATE_CHECK=1` lifts the dev-build gate
 * (app_update.rs `should_check`) and `GRID_LAUNCHER_E2E_FORGE_BASE` points
 * the request at the mock forge, which answers with tag `v9.9.9-e2e` —
 * newer than any version the app can report.
 *
 * Queue ids across this spec's single app instance: 1 = rom 801's update,
 * 2 = rom 802's native update. The spec still resolves the newest drawer row
 * by id rather than hardcoding those, so an extra background job admitted
 * before an update could never make the assertion silently read the wrong row.
 */
describe('updates', () => {
  const library = () => path.join(dataDir(), 'library');
  const nativeGameDir = () => path.join(library(), 'Windows', 'My Game');

  async function openDetails(romId: number) {
    await $(testId(`library-card-${romId}`)).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId(`library-card-${romId}`)).click();
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the details overlay never opened for rom ${romId}`,
    });
  }

  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return readFileSync(configPath(), 'utf-8').includes(line);
        } catch {
          return false;
        }
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `config.toml never got: ${line}` },
    );
  }

  async function closeDetails() {
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  }

  /** The highest download-row id currently rendered, or 0 when there is none. */
  async function newestRowId(): Promise<number> {
    const rows = await $$('[data-testid^="download-row-"]');
    const ids: number[] = [];
    for (const row of rows) {
      const attribute = (await row.getAttribute('data-testid')) ?? '';
      ids.push(Number(attribute.split('-').pop()));
    }
    return ids.length === 0 ? 0 : Math.max(...ids);
  }

  /**
   * Waits for a download row NEWER than `previous` and returns its id. The
   * queue never reuses an id, so "newer than the last one I saw" is what
   * makes this safe to call a second time with rom 801's row still on
   * screen.
   */
  async function waitForNewRow(previous: number, what: string): Promise<number> {
    let id = previous;
    await browser.waitUntil(
      async () => {
        id = await newestRowId();
        return id > previous;
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no download row appeared for ${what}` },
    );
    return id;
  }

  // The five views no longer stack (design §3): one pill click swaps which
  // root is displayed, so a spec has to be on the right view before it reads
  // text from it or clicks anything inside it.
  async function showDownloads() {
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  }

  async function showLibrary() {
    await $(testId('nav-library')).click();
    await $(testId('library-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library view never came back',
    });
  }

  /** Waits for one download row to reach `Completed`, on the Downloads view. */
  async function waitCompleted(id: number, what: string) {
    await showDownloads();
    await browser.waitUntil(
      async () => (await $(testId(`download-detail-${id}`)).getText()).startsWith('Completed'),
      { timeout: INSTALL_TIMEOUT, timeoutMsg: `${what} never reached Completed` },
    );
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

    // Prove the Downloads pill reaches its view once, up front; both update
    // jobs are observed as rows on it.
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });

    await $(testId('nav-library')).click();
    await $(testId('library-card-801')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Library grid never rendered the seeded installs',
    });
  });

  it('badges only the two games the server really has a newer copy of', async () => {
    // The update set is recomputed on connect and announced by
    // `updates-changed`, so the positives are what to WAIT for; only once
    // both are on screen is the absence of the other two meaningful.
    await $(testId('library-update-badge-801')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "rom 801's update badge never appeared",
    });
    await $(testId('library-update-badge-802')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "rom 802's update badge never appeared",
    });
    await expect($(testId('library-update-badge-801'))).toHaveText('UPDATE');

    // 803 matches the server exactly; 804 has no rom detail (404).
    await $(testId('library-update-badge-803')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'rom 803 was badged, but it is identical to the server copy',
    });
    await $(testId('library-update-badge-804')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'rom 804 was badged, but the server has no detail for it at all',
    });
  });

  it('badges a newer launcher release and lets it be dismissed from Settings', async () => {
    // The banner strip is gone (design §3): the notice is a badge on the
    // server menu plus an entry under Settings › Updates.
    await $(testId('app-update-badge')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the self-update badge never appeared for the mock forge release',
    });
    // The badge is the whole route: it opens Settings *on* the Updates
    // page, with no second click on the rail (design §3).
    await $(testId('app-update-badge')).click();
    const notice = $(testId('app-update-notice'));
    await notice.waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Settings › Updates never showed the stored notice',
    });
    expect(await notice.getText()).toContain(SELF_UPDATE_TAG);

    // `app-update-open` is deliberately NOT clicked: it hands the URL to the
    // OS opener, which would spawn a real browser out of the headless run.
    await $(testId('app-update-dismiss')).click();
    await $(testId('app-update-badge')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the self-update badge survived Dismiss',
    });
  });

  it('updates a non-native game by re-installing the newer server copy', async () => {
    // The case above ends on Settings; the Library root is hidden until the
    // pill is clicked, and a hidden card cannot be clicked.
    await showLibrary();
    await openDetails(801);
    // No version row: SNES is not a Windows/PC platform and the fixture
    // carries no revision, so `versionLabel` yields nothing to show.
    await expect($(testId('details-update'))).toHaveText('Update');
    expect(await $(testId('details-version')).isExisting()).toBe(false);

    const before = await newestRowId();
    await $(testId('details-update')).click();
    const rowId = await waitForNewRow(before, "rom 801's update job");
    await waitCompleted(rowId, "rom 801's update");
    await expect($(testId(`download-kind-${rowId}`))).toHaveText('Update');

    await showLibrary();
    await expect($(testId('details-update-toast'))).toHaveText("Updated 'Old Rom' successfully.");

    // The server copy is `newrom.zip`, so it extracts beside the seeded
    // `oldrom/` into its own `extraction_dir` (library/paths.rs).
    const extracted = path.join(library(), 'SNES', 'newrom');
    expect(existsSync(extracted)).toBe(true);
    expect(readdirSync(extracted).length > 0).toBe(true);

    // The post-finalize recompute finds the row now carrying the server's
    // own timestamp, so the badge has to go.
    await closeDetails();
    await $(testId('library-update-badge-801')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      reverse: true,
      timeoutMsg: 'rom 801 is still badged after its update completed',
    });
  });

  it('merges a native update over the install, preserving saves', async () => {
    await openDetails(802);
    // The INSTALLED version, not the server's: a Library-opened subject
    // reads its own `rom_file_name` first (`romFileNamesFor`), so the row
    // names what is on disk while the button names what it can become.
    await expect($(testId('details-version'))).toHaveText('Version: v1.0.0');
    await expect($(testId('details-update'))).toHaveText('Update to v1.1.0');

    // D-UI-10 on the Files tab: the tag parsed out of each file name, and
    // the installed-vs-server line above them.
    await $(testId('details-tab-files')).click();
    await expect($(testId('details-files-version'))).toHaveText(
      'Installed v1.0.0 · Server v1.1.0',
    );
    await expect($(testId('details-file-version-4802'))).toHaveText('v1.1.0');
    // `game.json` carries no tag, so it falls back to its last_modified —
    // which this fixture file does not have, leaving the cell blank.
    expect(await $(testId('details-file-version-4803')).getText()).toBe('');
    await $(testId('details-tab-overview')).click();

    // Native updates confirm first (doc 10): the first click only states
    // what survives, the second one starts the job.
    await $(testId('details-update')).click();
    await expect($(testId('details-update'))).toHaveText(
      'Saves and configuration will be preserved — confirm update',
    );

    const before = await newestRowId();
    await $(testId('details-update')).click();
    const rowId = await waitForNewRow(before, "rom 802's native update job");
    await waitCompleted(rowId, "rom 802's native update");
    await expect($(testId(`download-kind-${rowId}`))).toHaveText('Update');
    await showLibrary();

    // The whole point of the native path: the archive holds
    // `MyGame/mygame.exe` and nothing else the seed wrote, so a merge keeps
    // the save while a replace would have deleted it.
    const gameDir = path.join(nativeGameDir(), 'game');
    expect(readFileSync(path.join(gameDir, 'saves', 'slot1.sav'), 'utf-8')).toBe('SAVE1');
    expect(existsSync(path.join(gameDir, 'MyGame', 'mygame.exe'))).toBe(true);

    // Now the INSTALLED row carries v1.1.0 too, so the row is honest again.
    await expect($(testId('details-version'))).toHaveText('Version: v1.1.0');

    await closeDetails();
    await $(testId('library-update-badge-802')).waitForExist({
      timeout: INSTALL_TIMEOUT,
      reverse: true,
      timeoutMsg: 'rom 802 is still badged after its native update completed',
    });
  });

  it('offers no Update button for a game the server no longer knows', async () => {
    await openDetails(804);
    expect(await $(testId('details-update')).isExisting()).toBe(false);
  });

  // Design §10. The pure halves live in `theme.test.ts` / `uiSettings.test.ts`;
  // this is the one end-to-end pass over the surface: the select writes the
  // `<html>` attribute the whole token set keys off, and reaches config.toml.
  it('applies and persists the Appearance theme choice', async () => {
    await closeDetails();
    await $(testId('nav-settings')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the settings view never rendered',
    });
    await $(testId('settings-nav-appearance')).click();

    await $(testId('theme-select')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // A WebDriver click on the `<option>` (what `selectByAttribute` does)
    // does not reliably fire `change` on this embedded WebKitGTK driver —
    // the same limitation `emulators.spec.ts` documents, confirmed here by
    // config.toml keeping `theme = "system"`. Set the value and dispatch.
    await browser.execute(
      (selector: string) => {
        const el = document.querySelector(selector) as HTMLSelectElement | null;
        if (!el) throw new Error(`no element matched ${selector}`);
        el.value = 'dark';
        el.dispatchEvent(new Event('change', { bubbles: true }));
      },
      testId('theme-select'),
    );

    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.documentElement.dataset.theme ?? null)) === 'dark',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the dark theme never reached the <html> data-theme attribute',
      },
    );
    await waitForConfigLine('theme = "dark"');
  });

  // Design §3/§10: the blur sigma is baked into the cached variant's file
  // name, so the slider commits on release only. This checks the surface and
  // the round trip; `theme.test.ts` / `uiSettings.test.ts` own the clamping.
  it('persists the Appearance background blur slider', async () => {
    const blur = $(testId('background-blur'));
    await blur.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect(blur).toHaveAttribute('max', '40');
    await expect(blur).toHaveValue('2');

    // `setValue` does not move a range input under this driver: set the
    // value and dispatch the two events the component listens for.
    const setBlur = async (value: string) => {
      await browser.execute(
        (selector: string, next: string) => {
          const el = document.querySelector(selector) as HTMLInputElement | null;
          if (!el) throw new Error(`no element matched ${selector}`);
          el.value = next;
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
        },
        testId('background-blur'),
        value,
      );
      await waitForConfigLine(`background_blur = ${value}`);
    };

    await setBlur('0');
    // `waitForConfigLine('background_blur = 0')` above is what proves the
    // round trip: `Settings.svelte` renders every page in one `{#each}` with
    // `hidden`, so the Appearance pane never unmounts and the assertion
    // below reads the same DOM node, reflecting the in-memory store.
    await $(testId('settings-nav-updates')).click();
    await $(testId('settings-page-updates')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('settings-nav-appearance')).click();
    await $(testId('background-blur')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('background-blur'))).toHaveValue('0');

    // Restore the default so the cases after this one see the shipped value.
    await setBlur('2');
    await expect($(testId('background-blur'))).toHaveValue('2');
  });

  // Design §10: the four pages plan 1 left as placeholders. One pass over
  // the rail, reading one line each page owns; the pure rules live in
  // `settings/*.test.ts`.
  it('walks the Settings rail: Connection, Updates, Cloud saves, RetroAchievements', async () => {
    await $(testId('settings-nav-connection')).click();
    await $(testId('settings-page-connection')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('settings-connection-url'))).toHaveText(mockUrl());
    await expect($(testId('settings-connection-credential'))).toHaveText(
      'Stored in the OS keyring · session verified',
    );
    // Connected: Reconnect is not on offer, Disconnect is.
    await expect($(testId('settings-connection-reconnect'))).toBeDisabled();
    await expect($(testId('settings-connection-disconnect'))).toBeEnabled();

    await $(testId('settings-nav-updates')).click();
    await $(testId('settings-page-updates')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('settings-updates-version')).getText()).toMatch(/^GRID Launcher \d/);
    // The notice dismissed earlier is still listed here — Dismiss hid the
    // badge, not the entry (design §3).
    expect(await $(testId('app-update-notice')).getText()).toContain(SELF_UPDATE_TAG);
    expect(await $(testId('app-update-dismiss')).isExisting()).toBe(false);
    await expect($(testId('settings-updates-note'))).toHaveText(
      'GRID Launcher checks GitHub for a newer release once at startup. It never downloads or installs an update — open the release page to get it.',
    );

    await $(testId('settings-nav-cloud-saves')).click();
    await $(testId('settings-page-cloud-saves')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('cloud-settings-save')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });

    await $(testId('settings-nav-retroachievements')).click();
    await $(testId('settings-page-retroachievements')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('ra-status'))).toHaveText('Not set');
    await expect($(testId('ra-save'))).toBeDisabled();

    await $(testId('settings-nav-appearance')).click();
    await $(testId('settings-page-appearance')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('background-art-toggle'))).toBeSelected();
    await expect($(testId('card-size-library'))).toHaveValue('medium');
  });
});
