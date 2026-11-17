import { APP_START_TIMEOUT, FIXTURE_USERNAME, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { setServerOffline } from '../helpers/openOn.js';
import { tvRailKeys, tvRailTo } from '../helpers/romm.js';
import { connectPad, focused, press, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { notices, tap, tapK } from '../helpers/tvBackups.js';
import { captureErrors, moveTo, pageErrors, testId, text, waitText } from '../helpers/tvDetails.js';

const host = () => new URL(mockUrl()).host;
const CANT_REACH = "Can't reach RomM";

/** How many notices of the session read `title` (with or without a sub line). */
async function noticeCount(title: string): Promise<number> {
  return (await notices()).filter((n) => n === title || n.startsWith(`${title} | `)).length;
}

/**
 * Stage `tv-romm`, part B (TVRommStorefront README, "Library › RomM group"
 * and "The RomM page"): a second --tv launch on the profile part A left
 * connected to RomM, with the mock RomM dropping every API request (part A's
 * last step). RomM's startup restore finds no answer, so GRID opens its
 * shell (no welcome) with RomM in the Can't reach state: the rail group's
 * flag, no Discover and no platform rows, RomM › Installed's banner whose
 * Retry fails again while the server is down, Settings › Game stores' pill
 * and the RomM page's line and Reconnect. Then the server answers again and
 * Reconnect brings RomM back.
 */
describe('tv-romm (b): RomM can’t be reached after it was connected', () => {
  before(async () => {
    await $(testId('tv-tabs')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed its shell on relaunch' });
    await captureErrors();
    await connectPad(XBOX_PAD);
  });

  it("opens the shell, not the welcome; RomM's group flags Can't reach and has no Discover or platforms", async () => {
    await expect($(testId('tv-welcome'))).not.toExist();
    await waitTab('library');
    await browser.waitUntil(async () => (await $(testId('tv-lib-romm-flag')).getAttribute('aria-label').catch(() => null)) === CANT_REACH, {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: "the RomM group never flagged Can't reach RomM",
    });
    const keys = await tvRailKeys();
    expect(keys).toContain('r-romm:installed');
    expect(keys).not.toContain('r-romm:discover');
    expect(keys.some((k) => k.startsWith('r-platform:'))).toBe(false);
  });

  it("RomM › Installed: the Can't reach banner; Retry while the server is down says so again", async () => {
    await tvRailTo('r-romm:installed');
    await $(testId('tv-romm-banner')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-romm-banner')).toContain(`${CANT_REACH} · ${host()} didn't answer. Installed games still play.`);
    await waitText('tv-romm-banner-act', 'Retry');
    expect(await $(testId('tv-romm-chip')).isExisting()).toBe(false);
    const before = await noticeCount(CANT_REACH);
    await tap(testId('tv-romm-banner-act'));
    await browser.waitUntil(async () => (await noticeCount(CANT_REACH)) > before, {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: `Retry never ended in a "${CANT_REACH}" card (notices: ${JSON.stringify(await notices())})`,
    });
    expect(await notices()).toContain(`Reconnecting to RomM… | ${host()}`);
    await waitText('tv-romm-banner-act', 'Retry');
  });

  it("Settings › Game stores: the RomM row's pill, and the RomM page's line and Reconnect", async () => {
    await press('right_bumper');
    await press('right_bumper');
    await waitTab('settings');
    // From the pane to its rail row, then down the rail.
    if (!(await focused())?.startsWith('r-')) await press('east');
    await moveTo('r-stores', 'ArrowDown');
    await waitText('tv-pane-title', 'Game stores');
    await waitText('tv-stores-row-romm-pill', "Can't reach");
    await tapK('sto-romm');
    await $(testId('tv-romm-page')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await text('tv-rows-sub')).toBe(`Signed in as ${FIXTURE_USERNAME} · can't reach ${host()}`);
    await waitText('tv-rows-main', 'Reconnect');
  });

  it('with the server answering again, Reconnect connects RomM and brings the platforms back', async () => {
    await setServerOffline(false);
    await press('start');
    await waitText('tv-rows-sub', `Signed in as ${FIXTURE_USERNAME} · ${host()}`);
    await browser.waitUntil(async () => (await notices()).includes(`Connected to RomM as ${FIXTURE_USERNAME} | ${host()}`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `no "Connected to RomM" card (notices: ${JSON.stringify(await notices())})`,
    });
    expect(await $(testId('tv-rows-main')).isExisting()).toBe(false);
    await press('east');
    await $(testId('tv-romm-page')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    await tap(testId('tv-tab-library'));
    await waitTab('library');
    await browser.waitUntil(async () => (await tvRailKeys()).some((k) => k.startsWith('r-platform:')), {
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the platforms never came back to the RomM group',
    });
    expect(await tvRailKeys()).not.toContain('r-romm:discover');
    expect(await $(testId('tv-lib-romm-flag')).isExisting()).toBe(false);
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
