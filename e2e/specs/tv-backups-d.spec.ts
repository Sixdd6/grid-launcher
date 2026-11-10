import { existsSync } from 'node:fs';
import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { packDir, plugIn, readPack, unplug } from '../helpers/drives.js';
import { connectPad, key, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { confirmDialog, driveRow, listOptions, openDrivesPane, pageRow, tap, tapK, waitDriveStatus, waitNotice } from '../helpers/tvBackups.js';

const DRIVE = 'TESTDRIVE';
const PACK = 'Travel kit';
const SPARE = 'SPAREDRIVE';

/**
 * Stage `tv-backups/tv-backups-d` (docs/tv-mode/plan.md P13): on spec c's
 * profile (used without a server) and its TESTDRIVE ("Travel kit", for
 * Linux, no games left). Change… refuses to turn off the last computer, a
 * FAT32 drive is set up and renamed, Remove GRID files… asks twice, and a
 * remembered drive is forgotten.
 */
describe('tv-backups-d: drive settings', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('library');
    await captureErrors();
    await connectPad(XBOX_PAD);
    await openDrivesPane();
    await $(driveRow(DRIVE)).waitForExist({ timeout: 10_000 });
    await waitDriveStatus(DRIVE, 'Ready for backups');
  });

  it('the firmware default switches (item 495)', async () => {
    const on = async () => (await $(`${testId('tv-drives-firmware')} .sw`).getAttribute('class'))?.includes('on') ?? false;
    await browser.waitUntil(on, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the firmware default never read on' });
    await tap(testId('tv-drives-firmware'));
    await browser.waitUntil(async () => !(await on()), { timeout: TRANSITION_TIMEOUT });
    await tap(testId('tv-drives-firmware'));
    await browser.waitUntil(on, { timeout: TRANSITION_TIMEOUT });
  });

  it('Change… refuses to turn off the last computer (items 502–503)', async () => {
    await tap(driveRow(DRIVE));
    await $(testId('tv-drive-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', PACK);
    expect(await pageRow('dp-os')).toMatchObject({ label: 'For Linux', value: 'Change…' });
    await tapK('dp-os');
    await $(testId('tv-drive-os-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', 'Computers this pack is for');
    await waitText('tv-rows-sub', 'Emulators go on the drive built for each one. At least one stays ticked.');
    expect((await pageRow('os-linux')).on).toBe(true);
    expect((await pageRow('os-windows')).on).toBe(false);
    await tapK('os-linux');
    await waitNotice('At least one computer stays ticked');
    expect((await pageRow('os-linux')).on).toBe(true);
    await key('Escape');
    await waitGone('tv-drive-os-page');
    await key('Escape');
    await waitGone('tv-drive-page');
  });

  it('a FAT32 drive is set up with its warning, then renamed (items 507, 510–513)', async () => {
    plugIn(SPARE, { fs: 'vfat' });
    await $(driveRow(SPARE)).waitForExist({ timeout: 10_000, timeoutMsg: `${SPARE} never appeared` });
    await waitDriveStatus(SPARE, 'Not set up');
    await tap(driveRow(SPARE));
    await $(testId('tv-drive-setup-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-pr-su-fat32', 'This drive is FAT32: files over 4 GB can’t go on it. exFAT avoids that. GRID never formats a drive.');
    await key('m');
    await waitNotice(new RegExp(`^${SPARE} is ready for backups \\| ${SPARE} · FAT32$`), 10_000);
    await waitGone('tv-drive-setup-page');
    await waitDriveStatus(SPARE, 'Ready for backups');

    await tap(driveRow(SPARE));
    await $(testId('tv-drive-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await tapK('dp-rename');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    for (let i = 0; i < SPARE.length; i++) await browser.keys('Backspace');
    for (const ch of 'Spare kit') await browser.keys(ch);
    await browser.keys('Enter');
    await waitGone('tv-osk');
    await waitNotice('Renamed to Spare kit', 10_000);
    await waitText('tv-rows-title', 'Spare kit');
    expect(readPack(SPARE).name).toBe('Spare kit');
    await key('Escape');
    await waitGone('tv-drive-page');
  });

  it('Remove GRID files… asks twice, frees the folder and the drive reads Not set up (items 508–509)', async () => {
    await tap(driveRow(DRIVE));
    await $(testId('tv-drive-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await tapK('dp-remove');
    const first = await confirmDialog();
    expect(first.title).toBe(`Remove GRID files from ${PACK}?`);
    expect(first.body).toMatch(new RegExp(`^The gridpack folder \\(.+\\) is deleted from ${DRIVE}\\. Nothing else on the drive is touched, and the games installed on this computer stay\\.$`));
    expect(first).toMatchObject({ ok: 'Continue', danger: true });
    const second = await confirmDialog();
    expect(second.title).toMatch(new RegExp(`^Delete .+ from ${DRIVE}\\?$`));
    expect(second).toMatchObject({ body: 'This can’t be undone.', ok: 'Remove GRID files', danger: true });
    await waitNotice(new RegExp(`^GRID files removed from ${DRIVE} \\| .+ freed$`), 10_000);
    await waitGone('tv-drive-page');
    await waitDriveStatus(DRIVE, 'Not set up');
    expect(existsSync(packDir(DRIVE))).toBe(false);
  });

  it('a remembered drive offers Forget (items 497, 514)', async () => {
    unplug(SPARE);
    await waitDriveStatus(SPARE, 'Not connected');
    expect(await text('tv-pane')).toContain('Not connected');
    const sub = (await $(`${driveRow(SPARE)} [data-testid="tv-drive-sub"]`).getText()).trim();
    expect(sub).toMatch(/^Last seen today · 0 games · /);
    await tap(driveRow(SPARE));
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await listOptions()).toEqual({ title: 'Spare kit', options: ['Forget Spare kit'] });
    await tap(testId('tv-drive-forget'));
    await waitNotice('Spare kit forgotten', 10_000);
    await browser.waitUntil(async () => !(await $(driveRow(SPARE)).isExisting()), { timeout: 10_000, timeoutMsg: 'the forgotten drive stayed in the pane' });
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
