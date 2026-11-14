// e2e-app-args:
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, FIXTURE_TOKEN, INSTALL_TIMEOUT, mockUrl, THROTTLED_DOWNLOAD_TIMEOUT, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { assertNoClipping, axis, connectPad, focused, hints, invoke, key, NINTENDO_PAD, promptSet, waitFocused, waitPromptSet, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, text, typeKeys, waitGone, waitOskClosed, waitText } from '../helpers/tvDetails.js';

/** A controller input that moves nothing (a right-stick flick scrolls only):
 *  the controller leads the prompts again after keys. */
async function nudgePad(): Promise<void> {
  await axis('right_stick_y', 0.6);
  await axis('right_stick_y', 0);
}

const SNES = 'Super Nintendo Entertainment System';
/** With E2E_TV_SCREENS=1, screens are saved under ctr-out/tv-screens/settings/. */
const SHOTS = process.env.E2E_TV_SCREENS === '1' ? path.resolve(process.cwd(), '..', 'ctr-out', 'tv-screens', 'settings') : '';
async function shot(name: string): Promise<void> {
  if (!SHOTS) return;
  // Let the focused cover finish growing (tv.css: 150ms).
  await browser.pause(400);
  mkdirSync(SHOTS, { recursive: true });
  await browser.saveScreenshot(path.join(SHOTS, `${name}.png`));
}
const oldRoot = () => path.join(dataDir(), 'library');
const newRoot = () => path.join(dataDir(), 'library-new');

/** The open list's title and its options' texts, whitespace collapsed. */
async function list(): Promise<{ title: string; options: string[] }> {
  return (await browser.execute(() => {
    const sheet = document.querySelector('[data-testid="tv-list"]');
    const clean = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
    return {
      title: clean(sheet?.querySelector('h2')?.textContent),
      options: Array.from(sheet?.querySelectorAll('.opt') ?? []).map((o) => {
        const all = o.textContent ?? '';
        const sub = o.querySelector('.s')?.textContent ?? '';
        return clean(sub ? `${all.slice(0, all.length - sub.length)} ${sub}` : all);
      }),
    };
  })) as { title: string; options: string[] };
}

async function waitList(title: string | RegExp): Promise<void> {
  await browser.waitUntil(async () => {
    const t = (await list()).title;
    return typeof title === 'string' ? t === title : title.test(t);
  }, { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no list titled ${String(title)} (${JSON.stringify(await list())})` });
}

/** The row's segment, from its panel. */
async function segmentOf(id: number): Promise<string | null> {
  return (await browser.execute(
    (s: string) => document.querySelector(s)?.closest('[data-testid^="tv-dl-seg-"]')?.getAttribute('data-testid')?.replace('tv-dl-seg-', '') ?? null,
    testId(`tv-dl-row-${id}`),
  )) as string | null;
}

async function waitSegment(id: number, seg: string, timeout = TRANSITION_TIMEOUT): Promise<void> {
  await browser.waitUntil(async () => (await segmentOf(id)) === seg, { timeout, timeoutMsg: `row ${id} never reached ${seg} (in ${await segmentOf(id)})` });
}

/** Moves focus along a list to `k`, down first, then up. */
async function seek(k: string): Promise<void> {
  for (const dir of ['ArrowDown', 'ArrowUp']) {
    for (let i = 0; i < 12 && (await focused()) !== k; i++) {
      const was = await focused();
      await key(dir);
      await browser.pause(80);
      if ((await focused()) === was) break;
    }
  }
  await waitFocused(k);
}

/** Confirms the open dialog: Cancel has focus, Right reaches the action.
 *  `words` checks the body and the action button's label too. */
async function confirm(title: string | RegExp, words?: { body: string; ok: string; danger?: boolean }): Promise<void> {
  await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  const t = (await $(`${testId('tv-confirm')} h2`).getText()).trim();
  if (typeof title === 'string') expect(t).toBe(title);
  else expect(t).toMatch(title);
  if (words) {
    expect((await $(`${testId('tv-confirm')} p`).getText()).trim()).toBe(words.body);
    expect((await $(testId('tv-confirm-ok')).getText()).trim()).toBe(words.ok);
    if (words.danger) expect(await $(testId('tv-confirm-ok')).getAttribute('class')).toContain('dng');
  }
  await waitFocused('dlg-cancel');
  await key('ArrowRight');
  await waitFocused('dlg-ok');
  await key('Enter');
  await waitGone('tv-confirm');
}

/**
 * Stage `tv-settings` (docs/tv-mode/plan.md P10): connects on the desktop
 * (seed: a library folder and an empty `library-new` beside it), enters TV
 * mode and works through Downloads on throttled installs (rom 301 is the
 * big one; 201 and 101 are small), then Settings: the rail, a slider, Theme,
 * Button prompts and assignments with injected controllers, Change library
 * folder › Start fresh › Leave, and Disconnect.
 */
describe('tv-settings: Downloads and Settings', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('tv-mode-button')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the desktop shell never loaded' });
    await captureErrors();
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await connectPad(XBOX_PAD);
    await key('e');
    await waitTab('downloads');
  });

  it('Downloads shows Active, Queued and Completed with counts and empty lines (items 209–210)', async () => {
    for (const [seg, word] of [['active', 'active'], ['queued', 'queued'], ['completed', 'completed']]) {
      await waitText(`tv-dl-count-${seg}`, '0');
      await waitText(`tv-dl-empty-${seg}`, `Nothing ${word}.`);
    }
    expect(await hints()).not.toContain('Y Clear completed');
  });

  it('rows arrive in their panels with their lines, and A reads Cancel (items 212–213, 216)', async () => {
    await invoke('install_game', { romId: 301 });
    await invoke('install_game', { romId: 201 });
    await invoke('install_game', { romId: 101 });
    await waitSegment(1, 'active');
    await waitText('tv-dl-line-1', /^Arcade · Downloading/);
    await waitSegment(2, 'queued');
    await waitText('tv-dl-line-2', 'Arcade · Queued');
    await waitText('tv-dl-count-queued', '2');
    await seek('dl-1');
    expect(await hints()).toContain('A Cancel');
    expect(await $(`${testId('tv-dl-row-1')} .spark svg`).isExisting()).toBe(true);
  });

  it('Cancel asks first, then stops the row; focus stays on it and the queue moves on (items 217–218, 223, ruling 8)', async () => {
    await key('Enter');
    await waitList('Big Arcade Game');
    expect((await list()).options).toEqual(['Cancel', 'Open Big Arcade Game']);
    await waitFocused('o0');
    await key('Enter');
    // The ruled words (ruling 8): an active install's files are removed.
    await confirm('Cancel Big Arcade Game?', {
      body: 'The download stops and its files are removed. You can install it again later.',
      ok: 'Cancel install',
      danger: true,
    });
    await waitSegment(1, 'completed');
    await waitText('tv-dl-line-1', 'Arcade · Cancelled');
    await waitToast('Cancelled Big Arcade Game');
    expect(await focused()).toBe('dl-1');
    // The small installs run and finish.
    await waitSegment(2, 'completed', INSTALL_TIMEOUT);
    await waitSegment(3, 'completed', INSTALL_TIMEOUT);
    expect(await focused()).toBe('dl-1');
    expect(await hints()).toContain('Y Clear completed');
  });

  it('Dismiss removes a row and focus moves to the row in its place (item 220)', async () => {
    await seek('dl-2');
    expect(await hints()).toContain('A Dismiss');
    await key('Enter');
    await waitList('Pac-Man');
    expect((await list()).options).toEqual(['Dismiss', 'Open Pac-Man']);
    await key('Enter');
    await waitGone('tv-dl-row-2');
    await browser.waitUntil(async () => /^dl-[13]$/.test((await focused()) ?? ''), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'focus did not move to the row in its place' });
  });

  it('Retry queues a cancelled row again (item 219)', async () => {
    await seek('dl-1');
    await key('Enter');
    await waitList('Big Arcade Game');
    expect((await list()).options).toEqual(['Retry', 'Dismiss', 'Open Big Arcade Game']);
    await key('Enter');
    // The retry runs on a new row; the old one makes way.
    await waitGone('tv-dl-row-1');
    await waitSegment(4, 'active');
  });

  it('Y clears the completed rows, keeping focus on the active one (item 222)', async () => {
    await seek('dl-4');
    await key('y');
    await waitText('tv-dl-empty-completed', 'Nothing completed.');
    await waitGone('tv-dl-row-3');
    expect(await focused()).toBe('dl-4');
    expect(await hints()).not.toContain('Y Clear completed');
    // Stop it again (asked first), and clear it too.
    await key('Enter');
    await waitList('Big Arcade Game');
    await key('Enter');
    await confirm('Cancel Big Arcade Game?');
    await waitSegment(4, 'completed', THROTTLED_DOWNLOAD_TIMEOUT);
    await key('y');
    await waitText('tv-dl-empty-completed', 'Nothing completed.');
  });

  it('B on Downloads does nothing (item 224)', async () => {
    await key('Escape');
    await browser.pause(200);
    await waitTab('downloads');
  });

  it('the Settings rail shows each pane at once as focus moves (items 225–227)', async () => {
    await key('e');
    await waitTab('settings');
    await waitFocused('r-appearance');
    const panes: [string, string][] = [
      ['backup-drives', 'Backup drives'],
      ['cloud-saves', 'Cloud saves'],
      ['connection', 'Connection'],
      ['library', 'Library'],
      ['retroachievements', 'RetroAchievements'],
      ['stores', 'Stores'],
      ['systems', 'Systems'],
      ['tv', 'TV mode'],
      ['updates', 'Updates'],
    ];
    for (const [k, title] of panes) {
      await key('ArrowDown');
      await waitFocused(`r-${k}`);
      await waitText('tv-pane-title', title);
    }
    await waitText('tv-row-version', /^Version\s*GRID /);
    await moveTo('r-appearance', 'ArrowUp', 10);
    await waitText('tv-pane-title', 'Appearance');
    // TV flows never send the player to desktop mode (2026-11-14): TV mode
    // has its own Card size and Game labels instead.
    expect(await $(testId('tv-row-cards')).isExisting()).toBe(false);
    await waitText('tv-row-tv-card-size', /Every grid in TV mode.*Medium/s);
    await waitText('tv-row-tv-card-labels', /The title under each cover.*Always visible/s);
  });

  it('a slider adjusts with Left / Right while focused, with the D-pad hint (item 230)', async () => {
    await key('ArrowRight');
    await waitFocused('s-open-on');
    await moveTo('s-fade', 'ArrowDown');
    expect(await hints()).toContain('DPAD Adjust');
    await waitText('tv-row-fade', /50%/);
    expect(await $(`${testId('tv-row-fade')} .adj-arrows`).isDisplayed()).toBe(true);
    await key('ArrowRight');
    await waitText('tv-row-fade', /55%/);
    expect(await focused()).toBe('s-fade');
    await key('ArrowLeft');
    await waitText('tv-row-fade', /50%/);
    await browser.waitUntil(() => /background_fade = 50\b/.test(readFileSync(configPath(), 'utf-8')), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the fade never reached config.toml',
    });
  });

  it('Theme applies at once; Dark in TV mode (items 13, 229, 235, ruling 22)', async () => {
    await moveTo('s-theme', 'ArrowUp');
    await waitText('tv-row-theme', /Dark in TV mode/);
    expect(await browser.execute(() => document.documentElement.dataset.theme)).toBe('dark');
    await key('Enter');
    await waitList('Theme');
    expect((await list()).options).toEqual(['Follow system', 'Dark', 'Light']);
    await waitFocused('o0');
    await moveTo('o2', 'ArrowDown');
    await key('Enter');
    await waitFocused('s-theme');
    await browser.waitUntil(async () => (await browser.execute(() => document.documentElement.dataset.theme)) === 'light', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Light never applied',
    });
    await waitText('tv-row-theme', /Light/);
    await key('Enter');
    await waitFocused('o2');
    await moveTo('o0', 'ArrowUp');
    await key('Enter');
    await browser.waitUntil(async () => (await browser.execute(() => document.documentElement.dataset.theme)) === 'dark', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Follow system did not show dark',
    });
  });

  it("Card size and Game labels are TV mode's own: the grids follow, the desktop's keys stay (2026-11-14)", async () => {
    const config = () => readFileSync(configPath(), 'utf-8');
    const desktopKeys = () => {
      const c = config();
      return ['card_size_library', 'card_size_server', 'card_labels'].map((k) => c.match(new RegExp(`^${k} = .*$`, 'm'))?.[0] ?? `${k} (unset)`);
    };
    const before = desktopKeys();
    await moveTo('s-tv-card-size', 'ArrowDown');
    await key('Enter');
    await waitList('Card size');
    expect((await list()).options).toEqual(['Small', 'Medium', 'Large']);
    await waitFocused('o1');
    await moveTo('o2', 'ArrowDown');
    await key('Enter');
    await waitFocused('s-tv-card-size');
    await waitText('tv-row-tv-card-size', /Large/);
    await moveTo('s-tv-card-labels', 'ArrowDown');
    await key('Enter');
    await waitList('Game labels');
    expect((await list()).options).toEqual(['Always visible', 'Visible on focus']);
    await waitFocused('o0');
    await moveTo('o1', 'ArrowDown');
    await key('Enter');
    await waitFocused('s-tv-card-labels');
    await waitText('tv-row-tv-card-labels', /Visible on focus/);
    await browser.waitUntil(() => /^tv_card_size = "large"$/m.test(config()) && /^tv_card_labels = "focus"$/m.test(config()), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the TV grid settings never reached config.toml',
    });
    expect(desktopKeys()).toEqual(before);
    const root = (await browser.execute(() => {
      const t = document.querySelector('[data-tv-root]') as HTMLElement | null;
      return { size: t?.dataset.size, labels: t?.dataset.labels, gcw: t ? getComputedStyle(t).getPropertyValue('--gcw').trim() : '' };
    })) as { size?: string; labels?: string; gcw: string };
    expect(root).toEqual({ size: 'large', labels: 'focus', gcw: '12.5rem' });
    await shot('appearance');

    // The Library's grid: Large covers (a 12.5rem portrait cover's height),
    // no title line, and the focused cover's title as a pill.
    for (let i = 0; i < 3; i++) {
      await key('q');
      await browser.pause(150);
    }
    await waitTab('library');
    await $(`${testId('tv-lib-grid')} .cv`).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Library grid has no cover' });
    await key('ArrowRight');
    await browser.waitUntil(async () => (await browser.execute(() => !!document.querySelector('[data-testid="tv-lib-grid"] .cv.f'))) as boolean, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'focus never entered the Library grid',
    });
    const grid = (await browser.execute(() => {
      const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      const cells = Array.from(document.querySelectorAll<HTMLElement>('[data-testid="tv-lib-grid"] .gcell'));
      const last = cells.sort((a, b) => Number(a.dataset.i) - Number(b.dataset.i)).at(-1)!;
      const lastArt = last.querySelector<HTMLElement>('.cv-art')!;
      const box = document.querySelector<HTMLElement>('[data-testid="tv-lib-grid"] .gbox')!;
      const gridEl = document.querySelector<HTMLElement>('[data-testid="tv-lib-grid"]')!;
      const f = document.querySelector<HTMLElement>('[data-testid="tv-lib-grid"] .cv.f')!;
      const fl = f.querySelector<HTMLElement>('.cv-l');
      const other = document.querySelector<HTMLElement>('[data-testid="tv-lib-grid"] .cv:not(.f) .cv-l');
      const art = f.querySelector<HTMLElement>('.cv-art')!.getBoundingClientRect();
      const pill = fl?.getBoundingClientRect();
      return {
        lastRem: lastArt.offsetHeight / rem,
        lastBottom: last.offsetTop + lastArt.offsetHeight,
        boxHeight: box.offsetHeight,
        padBottom: parseFloat(getComputedStyle(gridEl).paddingBottom) / rem,
        pill: fl ? { position: getComputedStyle(fl).position, opacity: getComputedStyle(fl).opacity, gap: pill ? (pill.top - art.bottom) / rem : null } : null,
        otherOpacity: other ? getComputedStyle(other).opacity : '0',
      };
    })) as {
      lastRem: number;
      lastBottom: number;
      boxHeight: number;
      padBottom: number;
      pill: { position: string; opacity: string; gap: number | null } | null;
      otherOpacity: string;
    };
    // The last row keeps the target height: 12.5rem ÷ ¾.
    expect(Math.abs(grid.lastRem - 12.5 / 0.75)).toBeLessThan(0.1);
    // No label row under the last cover.
    expect(Math.abs(grid.boxHeight - grid.lastBottom)).toBeLessThanOrEqual(1);
    expect(grid.padBottom).toBeCloseTo(3.6, 1);
    expect(grid.pill?.position).toBe('absolute');
    expect(grid.pill?.opacity).toBe('1');
    // Clear of the grown cover's ring (.42rem) with a .2rem gap.
    expect(grid.pill?.gap ?? 0).toBeGreaterThan(0.6);
    expect(grid.otherOpacity).toBe('0');
    await assertNoClipping();
    await shot('library-large-focus');
    expect(desktopKeys()).toEqual(before);

    // Back to Settings › Appearance, focus in the pane.
    for (let i = 0; i < 3; i++) {
      await key('e');
      await browser.pause(150);
    }
    await waitTab('settings');
    await browser.waitUntil(async () => /^(r|s)-/.test((await focused()) ?? ''), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'focus never came back to Settings' });
    if ((await focused())?.startsWith('r-')) {
      await waitFocused('r-appearance');
      await key('ArrowRight');
    }
    await browser.waitUntil(async () => ((await focused()) ?? '').startsWith('s-'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'focus never entered the Appearance pane' });
  });

  it('Button prompts redraws the hints at once (items 70, 261)', async () => {
    await key('Escape');
    await waitFocused('r-appearance');
    await moveTo('r-tv', 'ArrowDown');
    await key('ArrowRight');
    await waitFocused('s-prompts');
    // Keys lead: the keyboard's prompts, whatever Button prompts says.
    await waitPromptSet('keyboard');
    await nudgePad();
    await waitPromptSet('xbox');
    await key('Enter');
    await waitList('Button prompts');
    expect((await list()).options).toEqual(['Match my controller', 'Xbox', 'PlayStation', 'Nintendo', '8BitDo (Pro series)']);
    await moveTo('o2', 'ArrowDown');
    await key('Enter');
    await waitText('tv-row-prompts', /PlayStation/);
    await waitPromptSet('keyboard');
    await nudgePad();
    await waitPromptSet('ps');
    await key('Enter');
    await moveTo('o0', 'ArrowUp');
    await key('Enter');
    await nudgePad();
    await waitPromptSet('xbox');
  });

  it('Button assignments is kept per controller and the row follows the controller in use (items 48, 53, 262–265)', async () => {
    await moveTo('s-assign', 'ArrowDown');
    await waitText('tv-row-assign', /For Xbox Wireless Controller.*Xbox style/);
    await waitText('tv-row-tv-controller', /Xbox Wireless Controller/);
    await key('Enter');
    await waitList('Button assignments');
    expect((await list()).options).toEqual(['Xbox style Bottom selects, right goes back', 'Nintendo style Right selects, bottom goes back']);
    expect(await $(`${testId('tv-list')} .sub`).getText()).toBe('Xbox Wireless Controller');
    await moveTo('o1', 'ArrowDown');
    await key('Enter');
    await waitText('tv-row-assign', /Nintendo style/);
    await browser.waitUntil(() => new RegExp(`"?${XBOX_PAD.key}"? = "nintendo"`).test(readFileSync(configPath(), 'utf-8')), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "the Xbox pad's assignment never reached config.toml",
    });
    // The assignment changes what the buttons do, not how they look: the
    // Xbox set stays, and Select is now drawn on the right button (B).
    await nudgePad();
    await waitPromptSet('xbox');
    expect(
      await browser.execute(() => document.querySelector('[data-testid="tv-hints"] [data-glyph="A"]')?.getAttribute('data-face') ?? null),
    ).toBe('B');
    // Another controller takes over: the row renames and shows its own style.
    await connectPad(NINTENDO_PAD);
    await axis('right_stick_y', 0.9, NINTENDO_PAD);
    await axis('right_stick_y', 0, NINTENDO_PAD);
    await waitText('tv-row-assign', /For Pro Controller.*Nintendo style/);
    await waitText('tv-row-tv-controller', /Pro Controller/);
    await waitPromptSet('nintendo');
    expect(readFileSync(configPath(), 'utf-8')).not.toContain(NINTENDO_PAD.key);
    expect(await promptSet()).toBe('nintendo');
  });

  it('Change library folder refuses the current one, then Start fresh › Leave switches (items 245–249)', async () => {
    await key('Escape');
    await waitFocused('r-tv');
    await moveTo('r-library', 'ArrowUp');
    await waitText('tv-row-lib-path', new RegExp(oldRoot().replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    await key('ArrowRight');
    await waitFocused('s-lib-change');
    // The current library: refused.
    await key('Enter');
    await waitText('tv-crumb', /Choose the new library folder/);
    await moveTo('fs-library', 'ArrowDown');
    await key('Enter');
    await waitText('tv-fs-path', /\/library$/);
    await key('y');
    await waitToast("Can't use that folder");
    // Another folder: the move list, Start fresh, Leave them on disk.
    await waitFocused('s-lib-change');
    await key('Enter');
    await waitText('tv-crumb', /Choose the new library folder/);
    await moveTo('fs-library-new', 'ArrowDown');
    await key('Enter');
    await waitText('tv-fs-path', /\/library-new$/);
    await key('y');
    await waitList(`Move to ${newRoot()}?`);
    expect((await list()).options).toEqual(['Move existing files Shows progress in Downloads', 'Start fresh Leave or delete the old files']);
    await moveTo('o1', 'ArrowDown');
    await key('Enter');
    await waitList('Delete the old game files, or leave them on disk?');
    expect((await list()).options[0]).toBe('Leave them on disk');
    expect((await list()).options[1]).toMatch(/^Delete them \(.+\)$/);
    await waitFocused('o0');
    await key('Enter');
    await waitToast(`Library is now ${newRoot()}`);
    await waitText('tv-row-lib-path', new RegExp(`${newRoot().replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`));
    await browser.waitUntil(() => readFileSync(configPath(), 'utf-8').includes(`library_path = ${JSON.stringify(newRoot())}`), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the new library folder never reached config.toml',
    });
    // Leave: the old game's files stay on disk.
    expect(existsSync(path.join(oldRoot(), 'games', SNES))).toBe(true);
  });

  it('Edit connection… opens Connect as a page: B keeps the session, Connect reconnects and comes back (ruling E1–3)', async () => {
    await key('Escape');
    await moveTo('r-connection', 'ArrowUp');
    await waitText('tv-row-credential', /Saved · verified/);
    await key('ArrowRight');
    await waitFocused('s-reconnect');
    await seek('s-server-page');
    await seek('s-config-folder');
    await seek('s-edit');
    await key('Enter');
    await $(testId('tv-edit-connection')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Edit connection never opened' });
    await waitText('tv-crumb', /Connect/);
    await waitText('tv-cf-url-value', /^http/);
    expect(await $(testId('tv-cf-lib')).isExisting()).toBe(false);
    expect(await $(testId('tv-connect-offline')).isExisting()).toBe(false);
    // B: back with nothing changed.
    await key('Escape');
    await waitGone('tv-edit-connection');
    await waitFocused('s-edit');
    await waitText('tv-row-credential', /Saved · verified/);
    // Again, with the token typed: Connect reconnects and comes back.
    await key('Enter');
    await $(testId('tv-edit-connection')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await seek('cf-token');
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await moveTo('cf-go', 'ArrowDown');
    await key('Enter');
    await waitToast(/^Connected to http/);
    await waitGone('tv-edit-connection');
    await waitText('tv-row-credential', /Saved · verified/);
  });

  it('Disconnect asks first, then returns to Connect (items 243–244)', async () => {
    await waitFocused('s-edit');
    await seek('s-disconnect');
    await key('Enter');
    await confirm(/^Disconnect from .+\?$/);
    await browser.waitUntil(async () => /Connect to your RomM server/.test(await text('tv-stage')), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Disconnect did not return to Connect',
    });
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
