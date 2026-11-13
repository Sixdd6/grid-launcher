// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, hints, key, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { confirmDialog, pageRow, tap, waitNotice } from '../helpers/tvBackups.js';

/**
 * Stage `tv-gog/tv-gog-b` (docs/stores-phase1-plan.md package F; TVStores
 * README § Settings › Stores, § Signing in, § Offline GOG installer, § GOG
 * games on backup drives). On package D's GOG harness (mock GOG, fake
 * gogdl, the fake offline installers in the stage's ~/Downloads, a seeded
 * GOG login with one game installed). Green only once the backend
 * packages and D's harness are merged.
 */
describe('tv-gog-b: Settings › Stores, signing in, offline installers, backups', () => {
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

  after(async () => {
    expect(await pageErrors()).toEqual([]);
  });

  it('Settings › Stores sits between RetroAchievements and Systems with a GOG row', async () => {
    await key('e');
    await key('e');
    await tap(testId('tv-set-rail-stores'));
    await waitText('tv-pane-title', 'Stores');
    const rail = await $$('[data-testid^="tv-set-rail-"]').map((r) => r.getAttribute('data-testid'));
    const at = rail.indexOf('tv-set-rail-stores');
    expect(rail[at - 1]).toBe('tv-set-rail-retroachievements');
    expect(rail[at + 1]).toBe('tv-set-rail-systems');
    expect(await text('tv-stores-row-gog')).toMatch(/GOG[\s\S]*Signed in as [\s\S]*Connected/);
  });

  it('GOG’s page lists the rows; Disconnect… asks with the desktop’s words', async () => {
    await tap(testId('tv-stores-row-gog'));
    await $(testId('tv-store-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await pageRow('owned')).toMatchObject({ label: 'Games you own' });
    expect((await pageRow('tool')).value).toMatch(/^gogdl .+ · installed$/);
    expect((await pageRow('license')).value).toBe('heroic-gogdl · GPL-3.0');
    expect(await pageRow('cloud')).toMatchObject({ label: 'Cloud saves', on: true });
    await tap(testId('tv-pr-disconnect'));
    const d = await confirmDialog();
    expect(d).toMatchObject({ title: 'Disconnect GOG?', ok: 'Disconnect', danger: true });
    expect(d.body).toMatch(/^GRID deletes your GOG login and gogdl from this computer\./);
  });

  it('Disconnect leaves GOG not connected; Start opens the sign-in with its hints, B cancels silently', async () => {
    // `confirmDialog` read the question and pressed Disconnect: the page closes.
    await waitGone('tv-store-page');
    await tap(testId('tv-stores-row-gog'));
    await waitText('tv-rows-sub', 'Not signed in');
    await waitText('tv-rows-main', 'Connect GOG…');
    await key('m');
    await $(testId('tv-store-login')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in layer never opened' });
    expect(await hints()).toEqual(['A Select', 'Y Keyboard', 'B Cancel sign-in']);
    await key('y');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await key('Escape');
    await waitGone('tv-store-login');
  });

  it('Install from installer… opens the folder browser in Downloads, files only; a .pkg is refused', async () => {
    await tap(testId('tv-pr-installer'));
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-fs-path', /\/Downloads$/);
    expect(await hints()).not.toEqual(expect.arrayContaining(['Y Use this folder']));
    const pkg = await $('[data-testid^="tv-fs-fs-"][data-testid$=".pkg"]');
    await tap(`[data-testid="${await pkg.getAttribute('data-testid')}"]`);
    await $(testId('tv-store-refusal')).waitForExist({ timeout: 10_000 });
    await waitText('tv-store-refusal-title', 'This is a macOS installer.');
    await tap(testId('tv-store-refusal-ok'));
    await waitGone('tv-store-refusal');
  });

  it('a Linux installer opens its page; Start installs it', async () => {
    await tap(testId('tv-pr-installer'));
    await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const sh = await $('[data-testid^="tv-fs-fs-"][data-testid$=".sh"]');
    await tap(`[data-testid="${await sh.getAttribute('data-testid')}"]`);
    await $(testId('tv-store-offline')).waitForExist({ timeout: 10_000 });
    await waitText('tv-rows-sub', /^From a GOG offline installer · .+\.sh$/);
    expect(await text('tv-pr-how')).toContain('The installer script isn’t run.');
    await key('m');
    await waitGone('tv-store-offline');
    await waitNotice(/added to download queue$/, 10_000);
  });

  it('GOG › Installed: RT picks, Y opens the Backup page', async () => {
    // The tabs wrap (LB/RB), so go to the tab itself.
    await tap(testId('tv-tab-library'));
    await tap(testId('tv-lib-rail-store-gog-installed'));
    await key('ArrowRight');
    await key('r');
    await $(testId('tv-selbar')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Enter');
    await waitText('tv-selbar-summary', /^1 selected · .+ to write$/);
    expect(await hints()).toEqual(['A Unpick', 'X Select all', 'Y Backup to drive…', 'RT Done']);
    await key('y');
    await $(testId('tv-store-backup')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-rows-title', 'Backup 1 game');
  });
});
