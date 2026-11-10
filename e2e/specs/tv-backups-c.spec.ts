import { readFileSync } from 'node:fs';
import { APP_START_TIMEOUT, configPath, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { mountOf } from '../helpers/drives.js';
import { connectPad, focused, hints, invoke, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitButtons, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, notices, screenState, confirmDialog, dlLines, gridCovers, listOptions, tap, waitNotice } from '../helpers/tvBackups.js';

const DRIVE = 'TESTDRIVE';
const PACK = 'Travel kit';
const ROM = 777;

const configText = () => {
  try {
    return readFileSync(configPath(), 'utf-8');
  } catch {
    return '';
  }
};

/** The drive cover's `data-k` ("dg777-<server>"). */
async function driveCover(): Promise<string> {
  let k = '';
  await browser.waitUntil(
    async () => {
      k = (await gridCovers()).find((c) => c.startsWith(`dg${ROM}-`)) ?? '';
      return k !== '';
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the drive's game never showed in the Library" },
  );
  return k;
}

/**
 * Stage `tv-backups/tv-backups-c` (docs/tv-mode/plan.md P13): a fresh
 * profile used without a server (seed `tv-backups-c`: a library folder,
 * `offline = true`) and TESTDRIVE holding the "Travel kit" pack: Travel Game
 * (PlayStation 2), PCSX2 for Linux and a PS2 BIOS. --tv opens the Library on
 * the drive. Select mode on a drive, Details from a drive, Install from
 * drive (with the drive's emulator and firmware), then Remove from drive….
 */
describe('tv-backups-c: Backup drives without a server', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('library');
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('opens on the drive: its header, chips and grid (items 420–422, 516)', async () => {
    await waitText('tv-lib-name', PACK);
    await waitText('tv-lib-count', new RegExp(`^1 game · .+ · on ${DRIVE}$`));
    await waitText('tv-lib-emus', 'Emulators: PCSX2 (Playstation 2)');
    await waitText('tv-lib-fw', 'Firmware: 1 system');
    expect(await text('tv-lib-header')).toContain('Select');
    await driveCover();
  });

  it('LT on a drive sorts by the drive sorts, remembered apart from the Library sort (ruling E5)', async () => {
    await waitText('tv-lib-sort', 'Sort: Title');
    await key('z');
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect((await listOptions()).title).toBe('Sort by');
    const labels = await browser.execute(() => Array.from(document.querySelectorAll('[data-testid^="tv-drive-sort-"]')).map((e) => e.textContent?.trim()));
    expect(labels).toEqual(['Title', 'Platform', 'Recently backed up', 'Largest first']);
    await tap(testId('tv-drive-sort-size'));
    await waitGone('tv-list');
    await waitText('tv-lib-sort', 'Sort: Largest first');
    await driveCover();
    // Back to Title for the tests that follow.
    await key('z');
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await tap(testId('tv-drive-sort-title'));
    await waitGone('tv-list');
    await waitText('tv-lib-sort', 'Sort: Title');
  });

  it('select mode on a drive: Y lists Install and Remove; B leaves it (items 94, 439, 444–446, 449)', async () => {
    const k = await driveCover();
    if ((await focused()) !== k) await key('ArrowRight');
    await waitFocused(k);
    await key('r');
    await $(testId('tv-selbar')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-selbar-y', 'Install or remove');
    expect(await hints()).toEqual(['A Pick', 'X Select all', 'Y Install or remove', 'RT Done']);
    await key('Enter');
    await waitText('tv-selbar-summary', /^1 selected · /);
    await key('y');
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await listOptions()).toEqual({ title: `1 game on ${PACK}`, options: ['Install 1 game', 'Remove 1 from drive…'] });
    await key('Escape');
    await waitGone('tv-list');
    // Still selecting; B leaves select mode before anything else (item 94).
    await $(testId('tv-selbar')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await waitGone('tv-selbar');
    expect(await focused()).toBe(k);
  });

  it('Details from a drive leads with the pack and offers Install from drive (items 430–432)', async () => {
    await key('Enter');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-det-title', 'Travel Game');
    await waitText('tv-det-header', new RegExp(`^From ${PACK}`));
    await waitButtons(['Install from drive', 'Remove from drive…']);
    expect(await text('tv-det-left')).toContain(`From ${PACK}`);
    expect(await text('tv-det-left')).toContain('Installing copies it from the drive, no network needed.');
    await key('Escape');
    await waitGone('tv-details');
  });

  it('X installs from the drive with its emulator and firmware, with the Game backups notices (items 424–429, 426, ruling 13)', async () => {
    const k = await driveCover();
    await waitFocused(k);
    expect(await hints()).toContain('X Install from drive');
    await key('x');
    await waitNotice(`Travel Game added to download queue | Copying from ${DRIVE}`, 10_000);
    await waitNotice(new RegExp(`^PCSX2 \\(Playstation 2\\) from ${PACK} added to download queue$`), 15_000);
    await tap(testId('tv-tab-downloads'));
    await waitTab('downloads');
    let rows: { title: string; kind: string; line: string }[] = [];
    await browser
      .waitUntil(
        async () => {
          rows = await dlLines();
          return rows.some((r) => r.title === 'Travel Game' && r.line.endsWith(`Installed from ${DRIVE}`));
        },
        { timeout: 60_000, interval: 200 },
      )
      .catch(() => {
        throw new Error(`Travel Game never read "Installed from ${DRIVE}": ${JSON.stringify(rows)}`);
      });
    await waitNotice('Travel Game installed', 15_000);
    await waitNotice('PCSX2 (Playstation 2) installed | Used for PlayStation 2', 30_000);
    await waitNotice(new RegExp(`firmware installed from ${PACK}$`), 30_000);
    await browser.waitUntil(() => /"PlayStation 2"\s*=\s*"PCSX2 \(Playstation 2\)"/.test(configText()), {
      timeout: 30_000,
      timeoutMsg: `PCSX2 from the drive never became PlayStation 2's default:\n${configText()}`,
    });
  });

  it('the installed copy is Play, and backing up says it needs a server (items 424, 455–456, 520)', async () => {
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    await waitText('tv-lib-name', PACK);
    const k = await driveCover();
    await tap(byK(k));
    // A click focuses and opens Details: Play now.
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitButtons(['Play', 'Remove from drive…']);
    expect(await text('tv-det-left')).toContain('Installed on this computer.');
    await key('Escape');
    await waitGone('tv-details');
    // X on the Library's platform row: Backup all needs a server.
    const plat = 'r-platform:playstation-2';
    await $(byK(plat)).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the PlayStation 2 row never showed' });
    await key('Escape');
    for (let i = 0; i < 12 && (await focused()) !== plat; i++) {
      await key('ArrowUp');
      await browser.pause(80);
    }
    await waitFocused(plat);
    expect(await hints()).toContain('X Backup all 1…');
    await key('x');
    await waitNotice('Backing up needs a server.');
  });

  it('Remove from drive… asks, removes the game and goes back (item 434)', async () => {
    await tapK(`r-drive:${mountOf(DRIVE)}`);
    await waitText('tv-lib-name', PACK);
    const k = await driveCover();
    await tap(byK(k));
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitButtons(['Play', 'Remove from drive…']);
    await tap(byK('db-remove-drive'));
    const words = await confirmDialog();
    expect(words).toEqual({
      title: `Remove Travel Game from ${PACK}?`,
      body: 'Its files leave the drive. The copy installed on this computer stays.',
      ok: 'Remove from drive',
      danger: true,
    });
    await waitNotice(new RegExp(`Travel Game.*${PACK}`), 10_000);
    await waitGone('tv-details');
    await browser.waitUntil(async () => (await gridCovers()).length === 0, { timeout: 10_000 }).catch(async () => {
      const games = await invoke<{ name: string }[]>('drive_games', { key: mountOf(DRIVE) }).catch((e) => String(e));
      throw new Error(`the game never left the drive grid: covers ${JSON.stringify(await gridCovers())}, drive_games ${JSON.stringify(games)}, notices ${JSON.stringify(await notices())}, ${await screenState()}`);
    });
    await waitText('tv-lib-empty', /^Nothing on this drive yet/);
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});

async function tapK(k: string): Promise<void> {
  await tap(byK(k));
}
