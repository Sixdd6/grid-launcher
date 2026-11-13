// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, INSTALL_TIMEOUT, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, key, waitFocused, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, pageErrors, testId, text, waitGone, waitText } from '../helpers/tvDetails.js';
import { byK, dlLines, gridCovers, pageRow, tap, tapK, waitNotice } from '../helpers/tvBackups.js';

/**
 * Stage `tv-gog/tv-gog-a` (docs/stores-phase1-plan.md package F; TVStores
 * README § Library, § The game page). Runs on package D's GOG harness: the
 * mock RomM server with `mock-gog.mjs` on the same host, the fake gogdl,
 * and a seeded GOG login (fake token files only). Connects on the desktop,
 * enters TV mode, opens Library › GOG, installs a game from Available
 * through the install page (Start), follows it in Downloads, then plays it
 * from its page. Green only once the backend packages and D's harness are
 * merged.
 */
describe('tv-gog-a: GOG in the Library, the game page and the install page', () => {
  let title = '';

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

  it('the rail has a GOG folder with Installed and Available, open by default', async () => {
    await key('q');
    await key('q');
    await key('q');
    await $(testId('tv-lib-fold-store-gog')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no GOG folder in the Library rail' });
    expect(await $(testId('tv-lib-fold-store-gog')).getAttribute('aria-expanded')).toBe('true');
    await $(testId('tv-lib-rail-store-gog-installed')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-lib-rail-store-gog-available')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('moving onto Available shows the two-row header and the owned games', async () => {
    await tap(testId('tv-lib-rail-store-gog-available'));
    await waitText('tv-lib-name', 'GOG · Available');
    await waitText('tv-lib-count', /^\d+ games? you own and haven’t installed · updated /);
    expect(await text('tv-store-chip-account')).toMatch(/^Signed in as /);
    expect(await $(testId('tv-store-installer')).isExisting()).toBe(true);
    expect(await $(testId('tv-store-refresh')).isExisting()).toBe(true);
    expect(await hints()).toEqual(expect.arrayContaining(['Y Search']));
    expect(await hints()).not.toEqual(expect.arrayContaining(['RT Select']));
    const covers = await gridCovers();
    expect(covers.length).toBeGreaterThan(0);
  });

  it('X on an Available cover says Install…; A opens its page with Install…', async () => {
    await key('ArrowRight');
    const k = await focused();
    expect(k).toMatch(/^sgog:/);
    expect(await hints()).toEqual(expect.arrayContaining(['X Install…']));
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the game page never opened' });
    title = await text('tv-det-title');
    expect(await text('tv-det-header')).toMatch(/^GOG/);
    await waitFocused('sd-install');
    expect(await $(testId('tv-store-tab-Cloud saves')).isExisting()).toBe(false);
  });

  it('Install… opens the install page; Start installs and the queued card offers Open Downloads', async () => {
    await key('Enter');
    await $(testId('tv-store-install')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the install page never opened' });
    await waitText('tv-rows-title', `Install ${title}`);
    await waitText('tv-rows-summary', /^.+ to download · .+ free on .+$/);
    expect(await pageRow('folder')).toMatchObject({ label: 'Install folder' });
    expect(await hints()).toEqual(expect.arrayContaining(['MENU Install']));
    await key('m');
    await waitGone('tv-store-install');
    await waitNotice(`${title} added to download queue`, 10_000);
  });

  it('Downloads shows the store row with the GOG badge; A lists Cancel and Open', async () => {
    await key('e');
    await key('e');
    const rows = await dlLines();
    const mine = rows.find((r) => r.title.startsWith(title));
    expect(mine).toBeDefined();
    expect(mine!.title).toContain('GOG');
    await browser.waitUntil(async () => (await dlLines()).some((r) => r.title.startsWith(title) && /^Installed/.test(r.line)), {
      timeout: INSTALL_TIMEOUT * 3,
      timeoutMsg: `${title} never finished installing`,
    });
    await waitNotice(new RegExp(`^${title} installed`), 10_000);
  });

  it('GOG › Installed lists it with Play; its page has the installed buttons and Cloud saves', async () => {
    await key('q');
    await key('q');
    await key('q');
    await tap(testId('tv-lib-rail-store-gog-installed'));
    await waitText('tv-lib-name', 'GOG · Installed');
    expect(await hints()).toEqual(expect.arrayContaining(['RT Select']));
    expect(await $(testId('tv-store-backup-all')).isExisting()).toBe(true);
    await key('ArrowRight');
    await browser.waitUntil(async () => (await hints()).includes('X Play'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X Play never showed' });
    await key('Enter');
    await $(testId('tv-store-details')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const buttons = await $$('[data-testid="tv-store-details"] .btns .btn').map((b) => b.getText());
    expect(buttons.map((b) => b.trim())).toEqual(expect.arrayContaining(['Play', 'Game settings', 'Verify and repair', 'Uninstall', 'Backup to drive…']));
    await $(testId('tv-store-tab-Cloud saves')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('e');
    await key('e');
    await $(testId('tv-store-cloud')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('Game settings is a page with Save on Start', async () => {
    await key('ArrowUp');
    await tapK('sd-settings');
    await $(testId('tv-store-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await pageRow('exe')).toMatchObject({ label: 'Launch file', sub: 'Set by GOG' });
    await key('m');
    await waitNotice('Game settings saved');
  });

  it('Play launches it: the Launching card, then the Playing page with GOG first', async () => {
    await tapK('sd-play');
    await waitNotice(new RegExp(`^Launching ${title}…`), 10_000);
    await $(testId('tv-store-playing')).waitForExist({ timeout: 10_000, timeoutMsg: 'the Playing page never opened' });
    await waitText('tv-playing-line', /^GOG · /);
    // Stop game asks first; the page closes when the game has stopped.
    await tapK('pl-stop');
    await tap(testId('tv-confirm-ok'));
    await waitGone('tv-store-playing');
    await $(byK('sd-play')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });
});
