// e2e-app-args:
import { existsSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, confirmDialog, dlLines, listOptions, notices, pageRow, tap, waitNotice } from '../helpers/tvBackups.js';
import { loginWindowOpen, MAIN_WINDOW } from '../helpers/gogLogin.js';

/**
 * Stage `tv-gog/tv-gog-b` (docs/stores-phase1-plan.md package F; TVStores
 * README § Settings › Stores, § Signing in, § Offline GOG installer, § GOG
 * games on backup drives). On package D's GOG harness (mock GOG, fake
 * gogdl, the fake offline installers in the stage's ~/Downloads, a seeded
 * GOG login with one game installed). Green only once the backend
 * packages and D's harness are merged.
 */
const DATA = process.env.E2E_DATA_DIR ?? '';

/** Opens the installer `name` from ~/Downloads through Settings › Stores › GOG's Install from installer…. */
async function openTvInstaller(name: string): Promise<void> {
  await tap(testId('tv-pr-installer'));
  await $(testId('tv-folder-browser')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  await tap(testId(`tv-fs-fs-${name}`));
  await $(testId('tv-store-offline')).waitForExist({ timeout: 10_000 });
}

/** The Downloads line of the row whose title starts with `title`. */
async function tvRowLine(title: string): Promise<string> {
  return (await dlLines()).find((r) => r.title.startsWith(title))?.line ?? '';
}

async function waitTvRowLine(title: string, re: RegExp, timeout = 60_000): Promise<void> {
  await browser.waitUntil(async () => re.test(await tvRowLine(title)), {
    timeout,
    interval: 250,
    timeoutMsg: `${title}'s row never matched ${re} (it reads "${await tvRowLine(title)}")`,
  });
}

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
    // The store's login window opens beside TV mode. WebDriver may follow it
    // as the active window, so every step below runs in the main window.
    await browser.waitUntil(loginWindowOpen, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the login window never opened' });
    await browser.switchToWindow(MAIN_WINDOW);
    await $(testId('tv-store-login')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the sign-in layer never opened' });
    // The layer's hints come a frame after the layer itself.
    await browser
      .waitUntil(async () => JSON.stringify(await hints()) === JSON.stringify(['A Select', 'Y Keyboard', 'B Cancel sign-in']), { timeout: TRANSITION_TIMEOUT })
      .catch(() => undefined);
    expect(await hints()).toEqual(['A Select', 'Y Keyboard', 'B Cancel sign-in']);
    await key('y');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // One B per layer, each after the one before has gone: the keyboard,
    // then the sign-in, which closes the login window too.
    await browser.waitUntil(async () => /^k/.test((await focused()) ?? ''), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the keyboard never took focus' });
    await key('Escape');
    await waitGone('tv-osk');
    await key('Escape');
    await waitGone('tv-store-login');
    await browser.waitUntil(async () => !(await loginWindowOpen()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the login window stayed open' });
    await browser.switchToWindow(MAIN_WINDOW);
    expect(await browser.execute(() => document.querySelectorAll('[data-layer]').length)).toBe(0);
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

  it('a Windows installer whose silent run fails installs on the milder silent run', async () => {
    if (DATA === '') return;
    await openTvInstaller('setup_bramble_1.0_(55002).exe');
    const how = await text('tv-pr-how');
    expect(how).toContain('GRID tries once more with gentler settings');
    expect(how).not.toMatch(/desktop/i);
    await key('m');
    await waitGone('tv-store-offline');
    // The card names the file: the identity is known only after the run.
    await waitNotice(/^setup_bramble_1\.0_\(55002\)\.exe added to download queue$/, 10_000);
  });

  it('when both silent runs fail: the reason, Retry and Delete installer, never desktop mode', async () => {
    if (DATA === '') return;
    await openTvInstaller('setup_foxglove_1.0_(55001).exe');
    await key('m');
    await waitGone('tv-store-offline');
    await waitNotice('Couldn’t install Foxglove silently | There isn’t enough disk space.', 60_000);
    await tap(testId('tv-tab-downloads'));
    // Bramble (the case before) installed on its milder silent run.
    await waitTvRowLine('Bramble', /^Installed from its installer$/);
    await waitTvRowLine('Foxglove', /^Failed · /);
    expect(await tvRowLine('Foxglove')).toBe("Failed · Couldn't run the installer silently: there isn't enough disk space. The installer was kept.");
    expect((await notices()).filter((n) => /desktop/i.test(n))).toEqual([]);
    const row = (await browser.execute(
      () => Array.from(document.querySelectorAll('[data-testid^="tv-dl-row-"]')).find((r) => (r.textContent ?? '').includes('Foxglove'))?.getAttribute('data-k') ?? null,
    )) as string;
    await tap(byK(row));
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect((await listOptions()).options).toEqual(['Retry', 'Delete installer', 'Dismiss']);
    await tap(testId('tv-dl-act-delete_installer'));
    const d = await confirmDialog();
    expect(d).toMatchObject({
      title: 'Delete Foxglove’s installer?',
      body: 'The installer file and its parts are deleted from this computer. This can’t be undone.',
      ok: 'Delete installer',
      danger: true,
    });
    await browser.waitUntil(async () => (await tvRowLine('Foxglove')) === '', { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the row never left' });
    expect(existsSync(path.join(DATA, 'home', 'Downloads', 'setup_foxglove_1.0_(55001).exe'))).toBe(false);
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
