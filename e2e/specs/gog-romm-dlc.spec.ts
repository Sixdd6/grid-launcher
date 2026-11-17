import { existsSync } from 'node:fs';
import path from 'node:path';
import { dataDir, INSTALL_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openRommPlatform, platformSlug } from '../helpers/romm.js';
import { connectRomm, openDownloads, testId } from '../helpers/gog.js';

/**
 * Stage `gog-romm-installer`, the game's own DLC files: a RomM game whose
 * archive holds a GOG installer, with DLC files on the server (RomM category
 * `dlc`, each a zipped GOG Linux DLC installer, fixtures-gog).
 *
 * - Kestrel Reach (903): a fresh install runs the game's installer, then each
 *   DLC file's installer into the game, one Downloads row per file
 *   ("Kestrel Reach (Ember Pass)"), and Details' Content line counts them
 *   installed.
 * - Heron Gate (904): its DLC files are hidden on the mock while the game
 *   installs (`POST /__e2e__/hide-files`), then shown, as for a game
 *   installed before the server held its DLC: Details offers Install DLC,
 *   which installs both, and then no longer offers it.
 */
const PLATFORM = Number(process.env.E2E_GOG_ROMM_PLATFORM ?? '1');
const games = () => path.join(dataDir(), 'library', 'games', 'Linux');

async function hideDlc(romId: number, hidden: boolean): Promise<void> {
  const res = await fetch(`${mockUrl()}/__e2e__/hide-files`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ rom_id: romId, category: 'dlc', hidden }),
  });
  if (!res.ok) throw new Error(`POST /__e2e__/hide-files answered ${res.status}`);
}

async function openDetails(romId: number): Promise<void> {
  await openRommPlatform(await platformSlug(PLATFORM));
  const card = $(testId(`game-card-${romId}`));
  await card.waitForExist({ timeout: TRANSITION_TIMEOUT });
  await card.click();
  await $(testId('details-panel')).waitForExist({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the details overlay never opened for rom ${romId}`,
  });
}

async function closeDetails(): Promise<void> {
  await $(testId('details-close')).click();
  await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
}

/** Every Downloads row as `{ title, line }` (Downloads must be on screen). */
async function rows(): Promise<{ title: string; line: string }[]> {
  const out: { title: string; line: string }[] = [];
  for (const row of await $$('[data-testid^="download-row-"]')) {
    try {
      const id = (await row.getAttribute('data-testid')).replace('download-row-', '');
      const title = (await row.$('.title').getText()).trim();
      const line = (await $(testId(`download-detail-${id}`)).getText()).trim();
      out.push({ title, line });
    } catch {
      // The row went away mid-read.
    }
  }
  return out;
}

/** Waits until each of `titles` has a row that ran its GOG installer. */
async function waitInstalled(titles: string[]): Promise<void> {
  await openDownloads();
  let seen: { title: string; line: string }[] = [];
  await browser.waitUntil(
    async () => {
      seen = await rows();
      return titles.every((title) =>
        seen.some((row) => row.title === title && row.line === 'Linux · Installed · ran its GOG installer'),
      );
    },
    {
      timeout: INSTALL_TIMEOUT * 2,
      timeoutMsg: `not every row installed: ${titles.join(', ')}`,
    },
  );
  expect(seen.some((row) => row.line.includes('Failed'))).toBe(false);
}

async function contentFact(): Promise<string> {
  const fact = $(testId('details-meta-content'));
  await fact.waitForExist({ timeout: TRANSITION_TIMEOUT });
  return (await fact.getText()).trim();
}

describe('gog-romm-dlc', () => {
  before(async () => {
    await connectRomm();
  });

  it('a fresh install runs each DLC file’s installer into the game after it', async () => {
    await openDetails(903);
    await $(testId('details-install')).click();
    await closeDetails();
    await waitInstalled(['Kestrel Reach', 'Kestrel Reach (Ember Pass)', 'Kestrel Reach (Tide Lines)']);

    const game = path.join(games(), 'Kestrel Reach', 'game', 'game');
    expect(existsSync(path.join(game, 'goggame-1305779630.info'))).toBe(true);
    expect(existsSync(path.join(game, 'goggame-1305779631.info'))).toBe(true);
    expect(existsSync(path.join(game, 'dlc-1305779632.pak'))).toBe(true);
    expect(existsSync(path.join(games(), 'Kestrel Reach', 'dlc'))).toBe(false);

    await openDetails(903);
    await browser.waitUntil(async () => (await contentFact()) === '2 DLC (all installed)', {
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the Content line never counted both DLC installed',
    });
    expect(await $(testId('details-install-dlc')).isExisting()).toBe(false);
    await closeDetails();
  });

  it('Install DLC installs the DLC files an installed game lacks', async () => {
    await hideDlc(904, true);
    await openDetails(904);
    await $(testId('details-install')).click();
    await closeDetails();
    await waitInstalled(['Heron Gate']);
    await hideDlc(904, false);

    // Reopened: Details asks the server again.
    await openDetails(904);
    const install = $(testId('details-install-dlc'));
    await install.waitForExist({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'Install DLC never appeared for the installed game',
    });
    expect(await install.getText()).toBe('Install DLC');
    expect(await contentFact()).toBe('2 DLC (none installed)');
    await install.click();
    await closeDetails();
    await waitInstalled(['Heron Gate (Night Market)', 'Heron Gate (Lantern Row)']);

    const game = path.join(games(), 'Heron Gate', 'game', 'game');
    expect(existsSync(path.join(game, 'goggame-1305779641.info'))).toBe(true);
    expect(existsSync(path.join(game, 'goggame-1305779642.info'))).toBe(true);

    await openDetails(904);
    await browser.waitUntil(async () => (await contentFact()) === '2 DLC (all installed)', {
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the Content line never counted both DLC installed',
    });
    expect(await $(testId('details-install-dlc')).isExisting()).toBe(false);
  });
});
