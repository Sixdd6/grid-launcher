// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { mountOf, plugIn, readPack } from '../helpers/drives.js';
import { connectPad, focused, hints, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, screenState, dlLines, driveRow, gridCovers, openDrivesPane, pageRow, tap, tapK, waitDriveStatus, waitNotice } from '../helpers/tvBackups.js';

const DRIVE = 'TESTDRIVE';

/**
 * Stage `tv-backups/tv-backups-a` (docs/tv-mode/plan.md P13), on the
 * `backups` fixtures (seed: backups-seed.mjs `tv-backups-a`, PCSX2 from the
 * server as PlayStation 2's emulator). Connects on the desktop, enters TV
 * mode, sets up a drive with Start, picks two games with RT and A, Y opens
 * the Backup page, Start backs them up, and Downloads reads the backup rows.
 */
describe('tv-backups-a: set up a drive and backup two games', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the server view never rendered after connecting' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await connectPad(XBOX_PAD);
  });

  it('Settings › Backup drives lists a plugged-in drive as Not set up (items 495–498, 494)', async () => {
    await openDrivesPane();
    await waitText('tv-drives-none', 'No drives connected. Plug in a USB drive or SSD.');
    expect(await text('tv-drives-firmware')).toContain('The server’s firmware for the systems backed up. The Backup page starts with this.');
    plugIn(DRIVE, { fs: 'exfat' });
    await $(driveRow(DRIVE)).waitForExist({ timeout: 10_000, timeoutMsg: `${DRIVE} never appeared in the pane` });
    await waitDriveStatus(DRIVE, 'Not set up');
    const sub = (await $(`${driveRow(DRIVE)} [data-testid="tv-drive-sub"]`).getText()).trim();
    expect(sub).toMatch(new RegExp(`^${DRIVE} · exFAT · `));
    expect(await $(`${driveRow(DRIVE)} .st`).getAttribute('class')).toContain('warn');
  });

  it('A opens Set up for GRID with this computer’s OS ticked; Start sets it up (items 510–513, ruling 14)', async () => {
    await tap(driveRow(DRIVE));
    await $(testId('tv-drive-setup-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', `Set up ${DRIVE} for GRID`);
    await waitText('tv-rows-sub', /^exFAT · .+ · Not set up$/);
    expect(await pageRow('su-name')).toMatchObject({ label: 'Pack name', value: DRIVE });
    expect((await pageRow('su-linux')).on).toBe(true);
    expect((await pageRow('su-windows')).on).toBe(false);
    expect(await $(testId('tv-pr-su-fat32')).isExisting()).toBe(false);
    await waitText('tv-pr-su-note', 'GRID makes a gridpack folder at the drive’s root and never touches anything outside it.');
    expect(await hints()).toContain('MENU Set up');
    // Turning off the only computer is refused.
    await tapK('su-linux');
    await waitNotice('At least one computer stays ticked');
    expect((await pageRow('su-linux')).on).toBe(true);
    await tapK('su-windows');
    await browser.waitUntil(async () => (await pageRow('su-windows')).on === true, { timeout: TRANSITION_TIMEOUT });
    // Start presses Set up for GRID from anywhere on the page.
    await key('m');
    await waitNotice(`${DRIVE} is ready for backups | ${DRIVE} · exFAT`, 10_000);
    await waitGone('tv-drive-setup-page');
    await waitDriveStatus(DRIVE, 'Ready for backups');
    const pack = readPack(DRIVE);
    expect([...pack.os].sort()).toEqual(['linux', 'windows']);
    expect(pack.games).toEqual([]);
  });

  it('a set-up drive opens its page (items 499–501)', async () => {
    await tap(driveRow(DRIVE));
    await $(testId('tv-drive-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', DRIVE);
    await waitText('tv-rows-sub', /^TESTDRIVE · exFAT · .+ · Ready for backups$/);
    expect(await pageRow('dp-os')).toMatchObject({ label: 'For Linux and Windows', value: 'Change…' });
    expect((await pageRow('dp-space')).sub).toMatch(/^Other files .+ · GRID .+ · .+ free$/);
    expect(await pageRow('dp-games')).toMatchObject({ value: '0' });
    expect(await pageRow('dp-emus')).toMatchObject({ value: 'None' });
    expect(await pageRow('dp-fw')).toMatchObject({ value: '0 systems' });
    expect((await pageRow('dp-folder')).value).toBe(`${mountOf(DRIVE)}/gridpack`);
    expect(await pageRow('dp-open')).toMatchObject({ label: 'Open folder', sub: 'Opens the file manager in desktop mode' });
    expect((await pageRow('dp-remove')).sub).toBe('Deletes only the gridpack folder; games installed on this computer stay');
    await key('Escape');
    await waitGone('tv-drive-page');
  });

  it('RT starts select mode with nothing picked; Y then says how to pick (items 439, 445–447)', async () => {
    await tap(testId('tv-tab-server'));
    await waitTab('server');
    await tapK('r-1');
    await browser.waitUntil(async () => (await gridCovers('#srv-grid')).length === 6, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the PlayStation 2 grid never filled' });
    if ((await focused()) !== 'sg901') await key('ArrowRight');
    await waitFocused('sg901');
    await key('r');
    await $(testId('tv-selbar')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-selbar-summary', '0 selected · 0 B');
    await waitText('tv-selbar-x', 'Select all 6');
    await waitText('tv-selbar-y', 'Backup to drive…');
    await waitText('tv-selbar-rt', 'Done');
    expect(await hints()).toEqual(['A Pick', 'X Select all', 'Y Backup to drive…', 'RT Done']);
    await key('y');
    await waitNotice('Pick games first | A picks the focused game; X picks them all.');
  });

  it('A picks covers, X selects all or clears, and nothing opens Details (items 440–442)', async () => {
    await key('x');
    await waitText('tv-selbar-x', 'All 6 selected · Clear');
    expect(await hints()).toContain('X Clear all');
    await key('x');
    await waitText('tv-selbar-summary', /^0 selected/);
    await key('Enter');
    await waitText('tv-selbar-summary', /^1 selected/);
    expect(await $(byK('sg901')).getAttribute('class')).toContain('pk');
    expect(await $(`${byK('sg901')} .pick`).isExisting()).toBe(true);
    expect(await hints()).toContain('A Unpick');
    await key('ArrowRight');
    await waitFocused('sg902');
    await key('Enter');
    await waitText('tv-selbar-summary', /^2 selected/);
    expect(await $(testId('tv-details')).isExisting()).toBe(false);
  });

  it('Y opens the Backup page on its first row (items 443, 457–460, 473–474, 479–480)', async () => {
    await key('y');
    await $(testId('tv-backup-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', 'Backup 2 games');
    expect(await text('tv-crumb')).toContain('Backup 2 games');
    await waitText('tv-rows-sub', /^GRID downloads each game from .+, with its art and details, to the drive’s gridpack folder\.$/);
    await browser.waitUntil(async () => /to write$/.test(await text('tv-rows-summary')), { timeout: 15_000, timeoutMsg: 'the page never showed what it writes' });
    const drive = await pageRow(`bk-drive-${mountOf(DRIVE)}`);
    expect(drive.radio).toBe(true);
    expect(drive.label).toBe(`${DRIVE} · ${DRIVE} · exFAT`);
    expect(drive.sub).toMatch(/left after this backup$/);
    expect(await focused()).toBe(`bk-drive-${mountOf(DRIVE)}`);
    const emus = await pageRow('bk-emus');
    expect(emus.sub).toContain('PCSX2');
    expect(emus.sub).toContain('Linux and Windows');
    const fw = await pageRow('bk-fw');
    expect(fw.on).toBe(true);
    expect(fw.sub).toMatch(/^1 system · \d+ files?$/);
    // Nothing is installed here: no Source section.
    expect(await $(testId('tv-pr-bk-h-source')).isExisting()).toBe(false);
    await waitText('tv-rows-main', 'Backup 2 games');
    expect(await hints()).toContain('MENU Backup');
    expect(await hints()).not.toContain('MENU Menu');
  });

  it('Start backs them up, goes back and ends select mode (items 480, 482, 452)', async () => {
    await key('m');
    await waitGone('tv-backup-page');
    await waitNotice(`2 games added to the backup queue for ${DRIVE}`, 10_000);
    expect(await $(testId('tv-selbar')).isExisting()).toBe(false);
    if (!(await $(byK('sg901')).isExisting())) throw new Error(`back on the grid? ${await screenState()}`);
    expect(await $(byK('sg901')).getAttribute('class')).not.toContain('pk');
  });

  it('Downloads reads each backup row, with the Backup badge, until every one is written (items 483–489)', async () => {
    await tap(testId('tv-tab-downloads'));
    await waitTab('downloads');
    const seen = new Set<string>();
    let rows: { title: string; kind: string; line: string }[] = [];
    await browser
      .waitUntil(
        async () => {
          rows = (await dlLines()).filter((r) => r.kind === 'Backup');
          for (const r of rows) seen.add(r.line.replace(/ \d+%$/, ''));
          return rows.length >= 4 && rows.every((r) => r.line.endsWith(`Written to ${DRIVE}`));
        },
        { timeout: 60_000, interval: 150 },
      )
      .catch(() => {
        throw new Error(`the Backup rows never all read "Written to ${DRIVE}": ${JSON.stringify(rows)} (seen ${JSON.stringify([...seen])})`);
      });
    const titles = rows.map((r) => r.title);
    expect(titles).toEqual(expect.arrayContaining(['Cinder Peaks', 'Moss and Marrow']));
    expect(titles.some((t) => /^PCSX2 .* for (Linux|Windows)$/.test(t))).toBe(true);
    await waitNotice(/ backed up to TESTDRIVE$/, 15_000);
    const pack = readPack(DRIVE);
    expect(pack.games.map((g) => g.source.rom_id).sort()).toEqual([901, 902]);
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
