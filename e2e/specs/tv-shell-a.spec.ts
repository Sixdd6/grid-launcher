import { readFileSync } from 'node:fs';
import { APP_START_TIMEOUT, configPath, TRANSITION_TIMEOUT } from '../helpers/env.js';
import {
  axis,
  connectPad,
  currentTab,
  disconnectPad,
  focused,
  hints,
  hold,
  invoke,
  key,
  NINTENDO_PAD,
  press,
  release,
  waitFocused,
  waitHintLeft,
  waitPromptSet,
  waitTab,
  XBOX_PAD,
} from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `tv-shell`, part A: GRID started with --tv on a profile used without
 * a server. The shell's bands and sizing (items 2, 4, 5), tabs (98), the
 * rail and pane rules (85, 95, 105), mouse and hint clicks (75–76, 134),
 * prompts and the hint bar's name following the last pad (47, 67–68),
 * Nintendo style swapping the face buttons (50, 54), an assignment kept per
 * controller (53), held-direction repeat (40) and the left stick (35).
 */
describe('tv-shell (a): --tv', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never appeared with --tv' });
    await waitTab('library');
  });

  it('is in TV mode, fullscreen, started with --tv (items 522, 30)', async () => {
    const info = await invoke<{ mode: string; started_with_tv: boolean; fullscreen: boolean }>('get_ui_mode');
    expect(info.mode).toBe('tv');
    expect(info.started_with_tv).toBe(true);
    expect(info.fullscreen).toBe(true);
    expect(await browser.execute(() => document.documentElement.dataset.ui)).toBe('tv');
  });

  it('sizes in rem from the height and lays out three bands (items 2, 5)', async () => {
    await browser.setWindowSize(1920, 1080);
    await browser.waitUntil(async () => (await browser.execute(() => window.innerHeight)) > 0);
    const m = await browser.execute(() => {
      const root = parseFloat(getComputedStyle(document.documentElement).fontSize);
      const h = (id: string) => document.querySelector(`[data-testid="${id}"]`)!.getBoundingClientRect().height;
      return { root, w: innerWidth, vh: innerHeight, top: h('tv-topbar'), hints: h('tv-hints'), stage: h('tv-stage') };
    });
    const want = Math.min(m.vh / 54, m.w / 80);
    expect(Math.abs(m.root - want)).toBeLessThan(0.05);
    if (m.vh === 1080 && m.w === 1920) expect(m.root).toBeCloseTo(20, 2);
    expect(Math.abs(m.top - 4.6 * m.root)).toBeLessThan(1);
    expect(Math.abs(m.hints - 3.6 * m.root)).toBeLessThan(1);
    expect(Math.abs(m.top + m.stage + m.hints - m.vh)).toBeLessThan(1.5);
  });

  it('shows the no-server bars (item 517)', async () => {
    await expect($(testId('tv-connection'))).toHaveText('Offline · no server');
    await waitHintLeft('No server. Your Library and Backup drives work without one.');
  });

  it('opens on the Library rail, and moving through the rail filters at once (items 77, 105)', async () => {
    await waitFocused('r-all');
    // The Library's own hints (P8, item 193).
    expect(await hints()).toEqual(['A Open', 'Y Search', 'LT Sort', 'RT Select', 'MENU Menu']);
    await key('ArrowDown');
    await waitFocused('r-recent');
    await expect($(`${testId('tv-library')} .vh h1`)).toHaveText('Recent');
  });

  it('switches tabs with Q / E and with injected LB / RB, wrapping (item 98)', async () => {
    await connectPad(XBOX_PAD);
    await key('e');
    await waitTab('server');
    await key('q');
    await waitTab('library');
    await press('left_bumper');
    await waitTab('settings');
    await press('right_bumper');
    await waitTab('library');
    await press('right_bumper');
    await waitTab('server');
    await press('right_bumper');
    await press('right_bumper');
    await waitTab('settings');
  });

  it('remembers each tab focus (item 79)', async () => {
    // Library was left on Recent.
    await press('right_bumper');
    await waitTab('library');
    await waitFocused('r-recent');
    await press('left_bumper');
    await waitTab('settings');
  });

  it('a held D-pad direction repeats (item 40); the window never scrolls (item 4)', async () => {
    await waitFocused('r-appearance');
    await hold('dpad_down');
    await browser.pause(800);
    await release('dpad_down');
    const after = await focused();
    const order = ['r-appearance', 'r-backup-drives', 'r-cloud-saves', 'r-connection', 'r-library', 'r-retroachievements', 'r-systems', 'r-tv', 'r-updates'];
    expect(order.indexOf(after ?? '')).toBeGreaterThanOrEqual(3);
    const settled = await focused();
    await browser.pause(400);
    expect(await focused()).toBe(settled);
    const scroll = await browser.execute(() => ({ top: document.scrollingElement!.scrollTop, h: document.scrollingElement!.scrollHeight, c: document.scrollingElement!.clientHeight }));
    expect(scroll.top).toBe(0);
    expect(scroll.h).toBeLessThanOrEqual(scroll.c);
  });

  it('the left stick moves like the D-pad (item 35)', async () => {
    await key('ArrowUp');
    const before = await focused();
    await axis('left_stick_y', 0.9);
    await axis('left_stick_y', 0);
    const order = ['r-appearance', 'r-backup-drives', 'r-cloud-saves', 'r-connection', 'r-library', 'r-retroachievements', 'r-systems', 'r-tv', 'r-updates'];
    await browser.waitUntil(async () => order.indexOf((await focused()) ?? '') === order.indexOf(before ?? '') - 1, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the left stick did not move focus up',
    });
  });

  it('Right from a rail row goes into the pane; B goes back to the selected row (items 85, 95)', async () => {
    // To TV mode's pane with the mouse: pointing focuses, a click is A.
    const tv = await $(testId('tv-set-rail-tv'));
    await tv.moveTo();
    await waitFocused('r-tv');
    await expect($(`${testId('tv-pane')} .pane-h h1`)).toHaveText('TV mode');
    await key('ArrowRight');
    await waitFocused('s-prompts');
    expect(await hints()).toEqual(['A Select', 'MENU Menu']);
    await key('Escape');
    await waitFocused('r-tv');
    // A hint is a button: clicking A presses A (item 134).
    await $(testId('tv-hint-A')).click();
    await waitFocused('s-prompts');
    await press('east');
    await waitFocused('r-tv');
  });

  it('clicking a top-bar tab switches to it (item 76)', async () => {
    await $(testId('tv-tab-downloads')).click();
    await waitTab('downloads');
    await $(testId('tv-tab-settings')).click();
    await waitTab('settings');
    await waitFocused('r-tv');
  });

  it('the hint bar names the controller pressed last, and the prompts follow it (items 47, 67–68)', async () => {
    await waitHintLeft('No server. Your Library and Backup drives work without one.');
    await waitPromptSet('xbox');
    await connectPad(NINTENDO_PAD);
    // Connecting is not pressing.
    await waitPromptSet('xbox');
    await press('dpad_down', NINTENDO_PAD);
    await waitPromptSet('nintendo');
    await press('dpad_up', XBOX_PAD);
    await waitPromptSet('xbox');
  });

  it('a Nintendo pad gets Nintendo style: right selects, bottom goes back (items 50, 54)', async () => {
    await waitFocused('r-tv');
    await press('east', NINTENDO_PAD);
    await waitFocused('s-prompts');
    await waitPromptSet('nintendo');
    await press('south', NINTENDO_PAD);
    await waitFocused('r-tv');
    // The Xbox pad keeps its own: bottom selects.
    await press('south', XBOX_PAD);
    await waitFocused('s-prompts');
    await press('east', XBOX_PAD);
    await waitFocused('r-tv');
  });

  it('an assignment is kept for that controller only (item 53)', async () => {
    const settings = await invoke<Record<string, unknown>>('get_ui_settings');
    await invoke('set_ui_settings', { settings: { ...settings, tv_assignments: { [NINTENDO_PAD.key]: 'xbox' } } });
    await browser.waitUntil(() => readFileSync(configPath(), 'utf-8').includes(NINTENDO_PAD.key), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the assignment never reached config.toml',
    });
    // A fresh TV shell reads the stored choice and the controllers still connected.
    await browser.refresh();
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT });
    await waitTab('library');
    await press('left_bumper', NINTENDO_PAD);
    await waitTab('settings');
    await waitHintLeft('No server. Your Library and Backup drives work without one.');
    // Xbox style on the Nintendo pad: bottom now selects, and match shows Xbox prompts.
    await waitFocused('r-appearance');
    await press('south', NINTENDO_PAD);
    await waitPromptSet('xbox');
    expect(await currentTab()).toBe('settings');
  });

  it('disconnecting hands over to the next controller; none left clears the name (item 39)', async () => {
    await press('dpad_down', NINTENDO_PAD);
    await disconnectPad(NINTENDO_PAD);
    await disconnectPad(XBOX_PAD);
    // Not offline would show the name; no-server keeps its line, so check the
    // TV mode pane's Controller row instead.
    await $(testId('tv-set-rail-tv')).moveTo();
    await waitFocused('r-tv');
    await expect($(testId('tv-row-tv-controller'))).toHaveText(expect.stringContaining('None found · the keyboard works too'));
    await connectPad(XBOX_PAD);
    await expect($(testId('tv-row-tv-controller'))).toHaveText(expect.stringContaining('Xbox Wireless Controller'));
  });
});
