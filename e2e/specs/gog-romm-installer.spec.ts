import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openPlatform } from '../helpers/server.js';
import { connectRomm, openDownloads, testId, waitForDownloadLine } from '../helpers/gog.js';
import { waitForToast } from '../helpers/toasts.js';

/**
 * Stage `gog-romm-installer` (GameStores README, "RomM games with a GOG
 * installer in their archive"): a RomM game whose archive holds a GOG Linux
 * installer (`.sh`) installs as a RomM game; its Downloads row reads
 * "Linux · Unpacking the GOG installer N%" then "Linux · Installed · ran its
 * GOG installer". One whose archive holds an unrecognised installer fails
 * with its own card. The rom ids come from package D's fixtures
 * (`E2E_GOG_ROMM_SH_ROM`, `E2E_GOG_ROMM_BAD_ROM`, on `E2E_GOG_ROMM_PLATFORM`):
 * it goes green only once packages B and D are merged.
 */
const PLATFORM = process.env.E2E_GOG_ROMM_PLATFORM ?? '';
const SH_ROM = process.env.E2E_GOG_ROMM_SH_ROM ?? '';
const BAD_ROM = process.env.E2E_GOG_ROMM_BAD_ROM ?? '';

async function install(rom: string): Promise<void> {
  await openPlatform(PLATFORM);
  const card = $(testId(`game-card-${rom}`));
  await card.waitForExist({ timeout: TRANSITION_TIMEOUT });
  await card.moveTo();
  await $(testId(`card-primary-${rom}`)).click();
}

describe('gog-romm-installer', () => {
  before(async () => {
    await connectRomm();
  });

  it('runs the archive’s Linux installer and stays a RomM game', async () => {
    if (PLATFORM === '' || SH_ROM === '') return;
    await install(SH_ROM);
    await openDownloads();
    await waitForDownloadLine(/^Linux · (Unpacking the GOG installer \d+%|Installed · ran its GOG installer)$/);
  });

  it('fails an unrecognised installer with its own card', async () => {
    if (PLATFORM === '' || BAD_ROM === '') return;
    await install(BAD_ROM);
    await waitForToast(/^Couldn’t install .+: its archive holds an installer GRID doesn’t recognise$/);
    await openDownloads();
    await waitForDownloadLine(/ · Failed · Its archive holds an installer GRID doesn’t recognise/);
  });
});
