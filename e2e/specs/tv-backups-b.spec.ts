import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { mountOf, plugIn, unplug } from '../helpers/drives.js';
import { connectPad, focused, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, dlLines, entries, gridCovers, notices, pageRow, tap, tapK, waitNotice } from '../helpers/tvBackups.js';

const DRIVE = 'TESTDRIVE';
const GAMES = [903, 904, 905, 906];

/**
 * Stage `tv-backups/tv-backups-b` (docs/tv-mode/plan.md P13): the drive is
 * pulled out mid-backup. Runs on spec a's profile (the session restores from
 * the keyring; --tv opens Server › Discover) and drive, which already holds
 * Cinder Peaks, Moss and Marrow, PCSX2 and the PS2 firmware, so this backup
 * is four throttled game rows ("Big Arcade Game" first, about ten seconds).
 */
describe('tv-backups-b: the drive is removed mid-backup', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it('queues four games picked in select mode', async () => {
    await tapK('r-1');
    await browser.waitUntil(async () => (await gridCovers('#srv-grid')).length === 6, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the PlayStation 2 grid never filled' });
    if ((await focused()) !== 'sg901') await key('ArrowRight');
    await waitFocused('sg901');
    await key('r');
    await $(testId('tv-selbar')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // A click on a cover in select mode picks it, as A does.
    for (const id of GAMES) await tapK(`sg${id}`);
    await waitText('tv-selbar-summary', /^4 selected/);
    await key('y');
    await $(testId('tv-backup-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', 'Backup 4 games');
    await browser.waitUntil(async () => (await text('tv-rows-main')) === 'Backup 4 games', { timeout: 15_000, timeoutMsg: 'the page never offered "Backup 4 games"' });
    // Everything else is on the drive already.
    expect((await pageRow('bk-emus')).sub).toBe('Already on the drive');
    await key('m');
    await waitGone('tv-backup-page');
    await waitNotice(`4 games added to the backup queue for ${DRIVE}`, 10_000);
  });

  it('pulling the drive fails the row being written, folds the rest into one row, says so once and leaves the rail (items 490–494, 452)', async () => {
    // The Library on the drive, so the rail's fallback shows.
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    const row = `r-drive:${mountOf(DRIVE)}`;
    await tapK(row);
    await waitText('tv-lib-name', DRIVE);
    await browser
      .waitUntil(async () => (await entries()).some((e) => e.title === 'Big Arcade Game' && e.status === 'downloading'), { timeout: 20_000, interval: 100 })
      .catch(async () => {
        throw new Error(`"Big Arcade Game" never started downloading: ${JSON.stringify(await entries())}`);
      });
    unplug(DRIVE);

    // The drive leaves the rail, which falls back to All games.
    await browser.waitUntil(async () => !(await $(byK(row)).isExisting()), { timeout: 10_000, timeoutMsg: 'the drive never left the Library rail' });
    await browser.waitUntil(async () => (await $(byK('r-all')).getAttribute('class'))?.includes('sel') ?? false, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the rail never fell back to All games',
    });
    await waitText('tv-lib-name', 'All games');
    await waitText('tv-lib-no-drives', 'No Backup drives connected');

    const removed = `${DRIVE} was removed`;
    await tap(testId('tv-tab-downloads'));
    await waitTab('downloads');
    let rows: { title: string; kind: string; line: string }[] = [];
    await browser
      .waitUntil(
        async () => {
          rows = (await dlLines()).filter((r) => r.kind === 'Backup');
          return rows.some((r) => r.title === 'Big Arcade Game' && r.line.endsWith(`Failed — ${removed}`)) && rows.some((r) => r.title === '3 more items');
        },
        { timeout: 15_000, interval: 200 },
      )
      .catch(() => {
        throw new Error(`the removal never failed and folded the rows: ${JSON.stringify(rows)}`);
      });
    const folded = rows.find((r) => r.title === '3 more items')!;
    expect(folded.line).toMatch(new RegExp(`Cancelled — ${removed} · backup again to finish$`));
    // Both sit in Completed; nothing of this backup waits or runs.
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-dl-seg-completed"]')?.textContent?.includes('3 more items') ?? false)).toBe(true);
    expect(rows.filter((r) => /(Waiting|Downloading|Writing)/.test(r.line))).toEqual([]);

    const card = await waitNotice(new RegExp(`^${removed}\\.`), 10_000);
    expect(card).toBe(`${removed}. 0 of 4 items were written to ${DRIVE}.`);
    await browser.pause(1500);
    expect((await notices()).filter((n) => n.includes(removed))).toHaveLength(1);
  });

  it('a drive plugged back in returns to the rail within a few seconds (item 494)', async () => {
    plugIn(DRIVE);
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    await $(byK(`r-drive:${mountOf(DRIVE)}`)).waitForExist({ timeout: 10_000, timeoutMsg: 'the drive never came back to the rail' });
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
