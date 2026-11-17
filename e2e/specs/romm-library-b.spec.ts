import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectRomm } from '../helpers/romm.js';
import { setServerOffline } from '../helpers/openOn.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const OUT = path.resolve(process.cwd(), '..', 'ctr-out', 'romm-library');

/** A screenshot for the design parity check (ctr-out/ is git-ignored). */
async function shot(name: string): Promise<void> {
  mkdirSync(OUT, { recursive: true });
  await browser.saveScreenshot(path.join(OUT, `${name}.png`));
}

/** The rail's row test ids, top to bottom. */
async function railOrder(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[data-testid="library-rail"] button')].map((b) => b.getAttribute('data-testid') ?? ''),
  );
}

/**
 * Stage `romm-library`, part B (romm-storefront-plan Step 3): RomM
 * connected. The Library opens on All games (Discover and Open GRID on are
 * removed for now, 2026-11-17); the RomM group has Installed and one row per
 * platform with RomM's game count; a platform's grid merges the server's
 * games with the installed ones (NOT LINKED, NOT ON ROMM). The seed installed
 * three Super Nintendo games: Super Mario World (rom 101), Hidden Kiln (no
 * rom id) and Sky Lantern (rom 999, gone from the server).
 */
describe('romm-library (b): connected', () => {
  before(async () => {
    await connectRomm();
    await $(testId('library-rail-romm-installed')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the RomM group never showed after connecting',
    });
  });

  it('opens on Library › All games, with the Library pill selected and no Discover', async () => {
    await expect($(testId('nav-library'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('library-rail-romm-discover'))).not.toExist();
    await expect($(testId('discover'))).not.toExist();
    await shot('b-all-games');
  });

  it('lists Installed and the platforms with RomM\'s game counts', async () => {
    await $(testId('library-rail-platform-snes')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the platform rows never appeared',
    });
    const order = await railOrder();
    const romm = order.filter((id) => id.startsWith('library-rail-romm-') || id.startsWith('library-rail-platform-'));
    // Platforms by label: Arcade, Super Nintendo Entertainment System, Windows 9x.
    expect(romm).toEqual([
      'library-rail-romm-installed',
      'library-rail-platform-arcade',
      'library-rail-platform-snes',
      'library-rail-platform-win',
    ]);
    await expect($(testId('library-rail-count-romm-installed'))).toHaveText('3');
    // RomM's game count only (decision 14): the two games RomM doesn't have aren't counted.
    await expect($(testId('library-rail-count-platform-snes'))).toHaveText('3');
    await expect($(testId('library-rail-count-platform-arcade'))).toHaveText('2');
    // The heading: the RomM mark, ROMM, and no flag while connected.
    await expect($(testId('library-rail-romm-installed-flag'))).not.toExist();
    // RomM sits after the fixed rows and among the stores by name (between GOG and Steam).
    expect(order.indexOf('library-rail-romm-installed')).toBeGreaterThan(order.indexOf('library-rail-updates'));
    const gog = order.indexOf('library-rail-store-gog-installed');
    const steam = order.indexOf('library-rail-store-steam-installed');
    if (gog !== -1) expect(order.indexOf('library-rail-romm-installed')).toBeGreaterThan(gog);
    if (steam !== -1) expect(order.indexOf('library-rail-romm-installed')).toBeLessThan(steam);
  });

  it('merges a platform\'s server games with the installed ones', async () => {
    await $(testId('library-rail-platform-snes')).click();
    await $(testId('server-platform-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('game-card-101')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('server-platform-counts'))).toHaveText(
      '3 games on RomM · 1 installed · plus 2 only on this computer',
    );
    // Super Mario World is RomM's rom 101, installed here: Play, with the dot.
    await expect($(testId('installed-badge-101'))).toExist();
    await expect($(testId('installed-badge-102'))).not.toExist();
    // The installed games RomM doesn't have, tagged on the cover.
    await expect($(testId('card-tag-local-hidden-kiln'))).toHaveText('NOT LINKED');
    await expect($(testId('card-tag-local-sky-lantern'))).toHaveText('NOT ON ROMM');
    // The Server header's chips and actions, now in the Library.
    await expect($(testId('server-emulator-chip'))).toExist();
    await expect($(testId('server-backup-platform'))).toHaveText('Backup all 3…');
    await expect($(testId('library-rail-platform-snes'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('nav-library'))).toHaveAttribute('aria-current', 'page');
    await shot('b-platform-snes');
  });

  it('words Details for an unlinked row', async () => {
    await $(testId('game-card-local-hidden-kiln')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-unlinked-note'))).toHaveText('Not linked to a RomM game');
    await expect($(testId('details-link-server'))).toHaveText('Link…');
    await $(testId('details-tab-saves')).click();
    await expect($(testId('details-link-to-sync'))).toHaveText(expect.stringContaining('Link this game to sync its saves'));
    await shot('b-details-unlinked-saves');
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('searches the merged grid', async () => {
    await $(testId('server-search')).setValue('Kiln');
    await browser.waitUntil(async () => !(await $(testId('game-card-101')).isExisting()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the search never narrowed the platform grid',
    });
    await expect($(testId('game-card-local-hidden-kiln'))).toExist();
    await $(testId('server-search')).setValue('');
  });

  it('shows RomM › Installed with its header and no banner', async () => {
    await $(testId('library-rail-romm-installed')).click();
    await $(testId('library-header')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('library-title'))).toHaveText('RomM · Installed');
    const host = new URL(mockUrl()).host;
    await expect($(testId('library-count'))).toHaveText(expect.stringMatching(new RegExp(`^3 games · .+ · from ${host.replace(/\./g, '\\.')}$`)));
    await expect($(testId('romm-installed-chip'))).toHaveText(`Signed in as ${FIXTURE_USERNAME}`);
    await expect($(testId('library-romm-backup-all'))).toHaveText('Backup all 3…');
    await expect($(testId('romm-banner'))).not.toExist();
    await expect($(testId('library-search'))).toHaveAttribute('placeholder', 'Search by title');
    await expect($(testId('library-card-101'))).toExist();
    await shot('b-installed');
  });

  it('keeps installed RomM games in All games', async () => {
    await $(testId('library-rail-all')).click();
    await expect($(testId('library-card-101'))).toExist();
    await expect($(testId('library-rail-count-all'))).toHaveText('3');
  });

  it('switches the three views with Ctrl+1..3', async () => {
    await browser.keys(['Control', '2']);
    await $(testId('downloads-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.keys(['Control', '3']);
    await $(testId('settings-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await browser.keys(['Control', '1']);
    await $(testId('library-view')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('hides Open GRID on (2026-11-17) and drops the Server card size', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-appearance')).click();
    await $(testId('theme-select')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('open-on-select'))).not.toExist();
    await expect($(testId('open-on-romm-off'))).not.toExist();
    await expect($(testId('card-size-server'))).not.toExist();
    await expect($(testId('card-size-library'))).toExist();
    await $(testId('nav-library')).click();
  });

  // Last: part C starts against an unreachable server.
  it('takes the mock offline for part C', async () => {
    await setServerOffline(true);
  });
});
