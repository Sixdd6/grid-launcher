import { APP_START_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, currentTab, focused, hints, hold, key, press, release, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Calls one of TvShell's e2e hooks (`window.__tvE2E`), with plain data:
 *  callbacks record into `window.__tvLog`. */
async function hook(name: 'openList' | 'openKeyboard' | 'confirmBox' | 'notify', spec: Record<string, unknown>): Promise<void> {
  await browser.execute(
    (n: string, s: Record<string, unknown>) => {
      const w = window as unknown as { __tvE2E: Record<string, (x: unknown) => unknown>; __tvLog: string[] };
      w.__tvLog = w.__tvLog ?? [];
      const log = (tag: string) => (v?: unknown) => w.__tvLog.push(v === undefined ? tag : `${tag}:${String(v)}`);
      const x: Record<string, unknown> = { ...s };
      if (n === 'openList') {
        x.items = (s.items as Record<string, unknown>[]).map((it) => ('group' in it ? it : { ...it, onPick: log(`pick:${String(it.label)}`) }));
      }
      if (n === 'openKeyboard') {
        x.onInput = log('input');
        x.onDone = log('done');
        x.onCancel = log('cancel');
      }
      if (n === 'confirmBox') x.onOk = log('ok');
      void w.__tvE2E[n](x);
    },
    name,
    spec,
  );
}

async function tvLog(): Promise<string[]> {
  return (await browser.execute(() => (window as unknown as { __tvLog?: string[] }).__tvLog ?? [])) as string[];
}

async function clearLog(): Promise<void> {
  await browser.execute(() => {
    (window as unknown as { __tvLog: string[] }).__tvLog = [];
  });
}

async function layers(): Promise<number> {
  return (await browser.execute(() => document.querySelectorAll('[data-testid="tv-layer"]').length)) as number;
}

async function waitLayers(n: number): Promise<void> {
  await browser.waitUntil(async () => (await layers()) === n, { timeout: TRANSITION_TIMEOUT, timeoutMsg: `expected ${n} overlays` });
}

async function oskValue(): Promise<string> {
  return (await browser.execute(() => document.querySelector('[data-testid="tv-osk-value"]')?.textContent ?? '')) as string;
}

async function cardTitles(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-toast-title"]')).map((e) => (e.textContent ?? '').trim()),
  )) as string[];
}

async function menuLabels(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-menu"] .opt')).map((e) => (e.textContent ?? '').trim()),
  )) as string[];
}

/**
 * Stage `tv-shell`, part D (P6): overlays and notifications, with --tv on
 * the profile used without a server. The welcome card (ruling 15); the Menu
 * with its items and counts (items 362–366); overlay suppression (103) and
 * click outside (76); Quit's confirmation with Cancel focused (373–376); a
 * list on its checked option, headings skipped (368–372); the on-screen
 * keyboard: pad shortcuts, held-X repeat, physical typing and Enter, masked
 * fields, its hints and the hint bar (42–43, 74, 377–388; ruling 20); the
 * card stack's limit and View on the newest card with an action (140–149);
 * the Notifications page (356–359).
 */
describe('tv-shell (d): overlays and notifications', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never appeared with --tv' });
    await waitTab('library');
    await connectPad(XBOX_PAD);
  });

  // A failed step must not leave an overlay open under the next one.
  afterEach(async () => {
    for (let i = 0; i < 4 && (await layers()) > 0; i++) {
      await key('Escape');
      await browser.pause(150);
    }
  });

  it('shows the welcome card once the main screen is up (ruling 15, items 140, 144)', async () => {
    await browser.waitUntil(async () => (await cardTitles()).includes('Welcome to TV mode'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no welcome card',
    });
    const card = await browser.execute(() => {
      const el = Array.from(document.querySelectorAll('[data-testid="tv-toast"]')).find((c) => c.textContent?.includes('Welcome to TV mode'))!;
      const r = el.getBoundingClientRect();
      const root = parseFloat(getComputedStyle(document.documentElement).fontSize);
      return { sub: el.querySelector('.ts')?.textContent ?? '', glyph: el.querySelector('.ts .gli')?.getAttribute('data-glyph'), top: r.top / root, right: (innerWidth - r.right) / root, width: r.width / root, role: el.getAttribute('role'), pe: getComputedStyle(el.parentElement!).pointerEvents };
    });
    expect(card.sub).toBe('Press  for the menu. Desktop mode is one press away there.');
    expect(card.glyph).toBe('MENU');
    expect(card.role).toBe('status');
    expect(card.pe).toBe('none');
    expect(Math.abs(card.top - 5.4)).toBeLessThan(0.1);
    expect(Math.abs(card.right - 2.4)).toBeLessThan(0.1);
    expect(Math.abs(card.width - 26)).toBeLessThan(0.1);
  });

  it('opens the Menu with Start, with its items and counts, and B closes it back on the same item (items 362–366, 92)', async () => {
    const before = await focused();
    await press('start');
    await $(testId('tv-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('gm0');
    const labels = await menuLabels();
    expect(labels[0]).toBe('Discover');
    expect(labels[1]).toBe('Downloads');
    expect(labels[2]).toMatch(/^Notifications · [1-9]\d*$/);
    expect(labels.slice(3)).toEqual(['Settings', 'Reconnect', 'Switch to desktop mode', 'Quit GRID']);
    expect((await $(testId('tv-menu-head')).getText()).trim()).toBe('Offline');
    expect(await $(testId('tv-menu-clock')).getText()).toMatch(/^\d\d:\d\d$/);
    expect(await hints()).toEqual(['A Select', 'B Close']);
    const w = await browser.execute(() => document.querySelector('[data-testid="tv-menu"]')!.getBoundingClientRect().width / parseFloat(getComputedStyle(document.documentElement).fontSize));
    expect(Math.abs(w - 30)).toBeLessThan(0.1);
    // Focus stays inside the Menu (item 361).
    for (let i = 0; i < 10; i++) await press('dpad_down');
    await waitFocused('gm6');
    await press('east');
    await waitLayers(0);
    await waitFocused(before!);
  });

  it('ignores X, Y, LT, RT, LB, RB, Menu and View under an overlay (item 103)', async () => {
    await press('start');
    await waitFocused('gm0');
    for (const b of ['west', 'north', 'left_trigger', 'right_trigger', 'left_bumper', 'right_bumper', 'start', 'select'] as const) await press(b);
    expect(await currentTab()).toBe('library');
    expect(await layers()).toBe(1);
    expect(await focused()).toBe('gm0');
  });

  it('closes on a click outside it (item 76)', async () => {
    await press('start');
    await waitFocused('gm0');
    await browser.execute(() => {
      const layer = document.querySelector<HTMLElement>('[data-testid="tv-layer"]')!;
      const r = layer.getBoundingClientRect();
      layer.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: r.left + 20, clientY: r.top + r.height / 2 }));
    });
    await waitLayers(0);
  });

  it('asks before Quit GRID, with Cancel focused; B cancels (items 373–376)', async () => {
    const before = await focused();
    await press('start');
    await waitFocused('gm0');
    for (let i = 0; i < 6; i++) await press('dpad_down');
    await waitFocused('gm6');
    await press('south');
    await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('dlg-cancel');
    expect(await $(testId('tv-confirm')).$('h2').getText()).toBe('Quit GRID?');
    expect(await $(testId('tv-confirm')).$('p').getText()).toBe('Downloads in progress pause and resume the next time GRID starts.');
    expect(await $(testId('tv-confirm-ok')).getText()).toBe('Quit');
    expect(await $(testId('tv-confirm-ok')).getAttribute('class')).toContain('pri');
    expect(await hints()).toEqual(['A Select', 'B Back']);
    await press('east');
    await waitLayers(0);
    await waitFocused(before!);
  });

  it('opens a list on its checked option, skips its headings, and A picks (items 368–372)', async () => {
    await clearLog();
    await hook('openList', {
      title: 'Emulator',
      sub: 'For PlayStation',
      items: [{ group: 'Installed' }, { label: 'DuckStation', checked: false }, { label: 'Beetle PSX', checked: true, sub: 'RetroArch' }, { group: 'Download' }, { label: 'Mednafen', checked: false, danger: true }],
    });
    await $(testId('tv-list')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('o2');
    expect(await $(testId('tv-list')).$('.sub').getText()).toBe('For PlayStation');
    expect(await browser.execute(() => document.querySelectorAll('[data-testid="tv-list"] .sgroup[data-f]').length)).toBe(0);
    expect(await browser.execute(() => !!document.querySelector('[data-k="o2"] .ck svg'))).toBe(true);
    expect(await $('[data-k="o4"]').getAttribute('class')).toContain('dng');
    expect(await hints()).toEqual(['A Select', 'B Back']);
    await press('dpad_down');
    await waitFocused('o4');
    await press('dpad_up');
    await press('dpad_up');
    await waitFocused('o1');
    await press('south');
    await waitLayers(0);
    await browser.waitUntil(async () => (await tvLog()).includes('pick:DuckStation'), { timeout: TRANSITION_TIMEOUT });
  });

  it('types on the on-screen keyboard with the pad, held X repeats, and Done keeps the value (items 42, 377–385, 388; ruling 20)', async () => {
    await clearLog();
    await hook('openKeyboard', { title: 'Search', value: '' });
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('k1');
    expect(await hints()).toEqual(['A Type', 'X Delete', 'Y Space', 'LB ◀', 'RB ▶', 'MENU Done', 'B Cancel']);
    // It stops at the hint bar, which stays in view (ruling 20).
    // (Measured once its slide-in has ended.)
    const gap = async () =>
      (await browser.execute(() => {
        const osk = document.querySelector('[data-testid="tv-osk"]')!.getBoundingClientRect();
        const bar = document.querySelector('[data-testid="tv-hints"]')!.getBoundingClientRect();
        return Math.abs(osk.bottom - bar.top);
      })) as number;
    await browser.waitUntil(async () => (await gap()) < 1.5, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the keyboard does not stop at the hint bar' });
    await press('dpad_down');
    await waitFocused('kq');
    await press('south');
    // LT: the next letter only is a capital (item 381).
    await press('left_trigger');
    await browser.waitUntil(async () => (await $(testId('tv-key-q')).getText()) === 'Q', { timeout: TRANSITION_TIMEOUT });
    expect(await focused()).toBe('kq');
    await press('south');
    await press('south');
    await press('north');
    await browser.waitUntil(async () => (await oskValue()) === 'qQq ', { timeout: TRANSITION_TIMEOUT, timeoutMsg: `value was "${await oskValue()}"` });
    // LB moves the caret; typing goes in at the caret.
    await press('left_bumper');
    await press('left_bumper');
    await press('south');
    await browser.waitUntil(async () => (await oskValue()) === 'qQqq ', { timeout: TRANSITION_TIMEOUT });
    // Held X deletes, repeating after 500 ms (item 42).
    await press('right_bumper');
    await press('right_bumper');
    await hold('west');
    await browser.pause(700);
    await release('west');
    const left = await oskValue();
    expect(left.length).toBeLessThanOrEqual(2);
    await browser.pause(300);
    expect(await oskValue()).toBe(left);
    // Menu is Done.
    await press('start');
    await waitLayers(0);
    const log = await tvLog();
    expect(log[log.length - 1]).toBe(`done:${left}`);
    expect(log).toContain('input:q');
  });

  it('A held on Shift does not repeat (item 43); B cancels and keeps nothing', async () => {
    await clearLog();
    await hook('openKeyboard', { title: 'Search', value: 'abc' });
    await waitFocused('k1');
    for (let i = 0; i < 4; i++) await press('dpad_down');
    await waitFocused('kshift');
    await hold('south');
    await browser.pause(800);
    await release('south');
    expect(await $(testId('tv-key-shift')).getAttribute('class')).toContain('on');
    await press('east');
    await waitLayers(0);
    expect(await tvLog()).toEqual(['cancel']);
  });

  it('takes a physical keyboard: keys type, Backspace deletes, Enter is Done after typing (item 74)', async () => {
    await clearLog();
    await hook('openKeyboard', { title: 'Search', value: '' });
    await waitFocused('k1');
    await key('zelda x');
    await key('Backspace');
    await browser.waitUntil(async () => (await oskValue()) === 'zelda ', { timeout: TRANSITION_TIMEOUT, timeoutMsg: `value was "${await oskValue()}"` });
    expect(await layers()).toBe(1);
    await key('Enter');
    await waitLayers(0);
    const log = await tvLog();
    expect(log[log.length - 1]).toBe('done:zelda ');
  });

  it('shows dots in a masked field, which opens empty (item 386)', async () => {
    await clearLog();
    await hook('openKeyboard', { title: 'API token', value: 'not-shown', mask: true });
    await waitFocused('k1');
    expect(await oskValue()).toBe('');
    await key('ab');
    await browser.waitUntil(async () => (await oskValue()) === '••', { timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await waitLayers(0);
  });

  it('keeps at most three cards, and View acts on the newest card with an action (items 141, 148–149)', async () => {
    await hook('notify', { group: 'game-queued', text: 'Zelda added to download queue', level: 'success', action: 'downloads' });
    await browser.waitUntil(async () => (await cardTitles())[0] === 'Zelda added to download queue', { timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('tv-toast-action')).getText()).toBe('Open Downloads');
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-toast-action"] .gli')?.getAttribute('data-glyph'))).toBe('VIEW');
    for (const t of ['Mario installed', 'Metroid installed', 'Kirby installed']) {
      await hook('notify', { group: 'removed', text: t, level: 'success' });
      await browser.pause(500);
    }
    await browser.waitUntil(async () => (await cardTitles())[0] === 'Kirby installed', { timeout: TRANSITION_TIMEOUT });
    expect(await cardTitles()).toEqual(['Kirby installed', 'Metroid installed', 'Mario installed']);
    await hook('notify', { group: 'game-queued', text: 'Sonic added to download queue', level: 'success', action: 'downloads' });
    await browser.waitUntil(async () => (await cardTitles())[0] === 'Sonic added to download queue', { timeout: TRANSITION_TIMEOUT });
    await press('select');
    await waitTab('downloads');
    expect(await cardTitles()).not.toContain('Sonic added to download queue');
  });

  it('lists the session in Menu › Notifications, newest first, and Y clears it (items 356–359)', async () => {
    await press('start');
    await waitFocused('gm0');
    const label = (await menuLabels())[2];
    await press('dpad_down');
    await press('dpad_down');
    await waitFocused('gm2');
    await press('south');
    await $(testId('tv-notifications')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const count = Number(await $(testId('tv-notifications-count')).getText());
    expect(label).toBe(`Notifications · ${count}`);
    const rows = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="tv-notification"]')).map((r) => ({
        t: r.querySelector('.t')?.textContent ?? '',
        s: r.querySelector('.s')?.textContent ?? '',
      })),
    );
    expect(rows.length).toBe(count);
    expect(rows[0].t).toBe('Sonic added to download queue');
    expect(rows[0].s).toMatch(/^\d\d:\d\d$/);
    expect(rows.map((r) => r.t)).toContain('Welcome to TV mode');
    expect(await hints()).toContain('Y Clear all');
    await press('north');
    await $(testId('tv-notifications-empty')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('tv-notifications-empty')).getText()).toBe('Nothing yet. Every notice GRID shows lands here too.');
    expect(await $(testId('tv-notifications-count')).getText()).toBe('0');
    await press('east');
    await waitTab('downloads');
  });
});
