// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { assertNoClipping, connectPad, focused, hold, key, release, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const FIRST_ID = 10001;
const LAST_ID = 12500;
const LIBRARY_GAMES = 600;
/** The most covers a windowed grid may hold in the DOM: a screenful plus
 *  the buffer rows, at any window size the suite runs in. */
const MAX_RENDERED = 150;
/** How long the cover focus stops on may take to show (the mock answers each
 *  cover after 40 ms; the desktop's large-platform spec allows 2.5 s). */
const STOPPED_COVER_TIMEOUT = 2_500;

/** The focused cover's index in its grid, or -1. */
async function focusedIndex(): Promise<number> {
  return (await browser.execute(() => Number(document.querySelector('[data-tv-root] .f')?.closest('[data-i]')?.getAttribute('data-i') ?? -1))) as number;
}

/** Covers in the fullest rendered row: the most cells sharing one top.
 *  Rows differ in length (covers are the shape of their art: the fixtures'
 *  square covers next to portrait ones not loaded yet). */
async function perRow(grid: string): Promise<number> {
  return (await browser.execute((g: string) => {
    const tops = new Map<number, number>();
    for (const c of Array.from(document.querySelectorAll<HTMLElement>(`#${g} [data-i]`))) tops.set(c.offsetTop, (tops.get(c.offsetTop) ?? 0) + 1);
    return Math.max(1, ...tops.values());
  }, grid)) as number;
}

/** The focused cover's cell: its index, its top, and the first index in its row. */
async function focusedCell(): Promise<{ index: number; top: number; rowStart: number }> {
  return (await browser.execute(() => {
    const cell = document.querySelector('[data-tv-root] .f')?.closest<HTMLElement>('[data-i]');
    if (!cell) return { index: -1, top: -1, rowStart: -1 };
    const row = Array.from(cell.parentElement!.querySelectorAll<HTMLElement>('[data-i]')).filter((c) => c.offsetTop === cell.offsetTop);
    return { index: Number(cell.dataset.i), top: cell.offsetTop, rowStart: Math.min(...row.map((c) => Number(c.dataset.i))) };
  })) as { index: number; top: number; rowStart: number };
}

/** Up once, confirmed: focus lands on an earlier cover in a row above. */
async function upOneRow(): Promise<void> {
  const was = await focusedCell();
  await key('ArrowUp');
  await browser.waitUntil(
    async () => {
      const now = await focusedCell();
      return now.index >= 0 && now.index < was.index && now.top < was.top;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `Up did not move a row up from cover ${was.index}` },
  );
}

async function rendered(grid: string): Promise<number> {
  return (await browser.execute((g: string) => document.querySelectorAll(`#${g} [data-cover]`).length, grid)) as number;
}

/**
 * Holds Down for `ms`, sampling the DOM as focus runs through the grid:
 * answers the most covers rendered at once and whether focus was ever lost.
 */
async function holdDown(grid: string, ms: number): Promise<{ most: number; lost: boolean }> {
  let most = 0;
  let lost = false;
  await hold('dpad_down');
  const end = Date.now() + ms;
  while (Date.now() < end) {
    most = Math.max(most, await rendered(grid));
    if ((await focused()) === null) lost = true;
    await browser.pause(150);
  }
  await release('dpad_down');
  return { most, lost };
}

/** Milliseconds until the focused cover's image has decoded, or a little over `limit`. */
async function msUntilFocusedCover(limit: number): Promise<number> {
  return (await browser.execute(
    (max: number) =>
      new Promise<number>((resolve) => {
        const t0 = performance.now();
        const poll = () => {
          const img = document.querySelector('[data-tv-root] .f img') as HTMLImageElement | null;
          const now = performance.now() - t0;
          if ((img?.naturalWidth ?? 0) > 0 || now > max) resolve(Math.round(now));
          else requestAnimationFrame(poll);
        };
        poll();
      }),
    limit,
  )) as number;
}

/** Steps Left one cover at a time, each step confirmed, down to index `to`. */
async function stepLeftTo(to: number): Promise<void> {
  // Let any repeat still in flight from a hold land first.
  let prev = -2;
  await browser.waitUntil(
    async () => {
      const i = await focusedIndex();
      const still = i === prev;
      prev = i;
      return still;
    },
    { timeout: TRANSITION_TIMEOUT, interval: 300 },
  );
  for (let i = await focusedIndex(); i > to; i--) {
    await key('ArrowLeft');
    await browser.waitUntil(async () => (await focusedIndex()) === i - 1, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `Left from cover ${i} did not reach ${i - 1} (focused index ${await focusedIndex()})`,
    });
  }
  expect(await focusedIndex()).toBe(to);
}

/** Waits for the grid's scroll to stop gliding. */
async function settle(grid: string): Promise<void> {
  let last = -1;
  await browser.waitUntil(
    async () => {
      const top = (await browser.execute((g: string) => (document.getElementById(g)!.closest('[data-scroll]') as HTMLElement).scrollTop, grid)) as number;
      const still = top === last;
      last = top;
      return still;
    },
    { timeout: TRANSITION_TIMEOUT, interval: 200 },
  );
}

/**
 * Stage `tv-large-platform` (docs/tv-mode/plan.md risk R8): the
 * large-platform fixtures (one 2500-game Arcade platform, covers 40 ms each)
 * and 600 seeded installed games. TV grids render only the covers near the
 * view and move on their layout inside, so a long grid stays fast, focus moves
 * through it without a gap, and covers load as they scroll in.
 */
describe('tv-large-platform: windowed grids', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the platform rail never appeared after connecting' });
    await browser.execute(() => {
      const w = window as unknown as { __errs: string[] };
      w.__errs = [];
      addEventListener('error', (e) => w.__errs.push(`error: ${e.message} ${e.error?.stack ?? ''}`));
      addEventListener('unhandledrejection', (e) => w.__errs.push(`rejection: ${String(e.reason)} ${e.reason?.stack ?? ''}`));
    });
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await connectPad(XBOX_PAD);
  });

  it('a 2500-game Server platform renders a screenful, at the whole list’s height', async () => {
    // Into the rail, then down to the Arcade platform.
    for (let i = 0; i < 6 && !((await focused()) ?? '').startsWith('r-'); i++) {
      await key('ArrowLeft');
      await browser.pause(100);
    }
    for (let i = 0; i < 6 && (await focused()) !== 'r-1'; i++) {
      await key('ArrowDown');
      await browser.pause(100);
    }
    await waitFocused('r-1');
    await $(testId(`tv-cover-sg${FIRST_ID}`)).waitForExist({ timeout: 15_000, timeoutMsg: 'the platform never drew its grid' });
    const count = await rendered('srv-grid');
    console.log(`${count} of 2500 covers rendered at the top`);
    expect(count).toBeLessThanOrEqual(MAX_RENDERED);
    expect(await $(testId(`tv-cover-sg${LAST_ID}`)).isExisting()).toBe(false);
    // The layout's box makes the scroll area as tall as every row. Rows
    // differ in length, but none holds twice the fullest one rendered.
    const cols = await perRow('srv-grid');
    const tall = await browser.execute((c: number) => {
      const s = document.getElementById('srv-grid')!.closest('[data-scroll]') as HTMLElement;
      const cell = document.querySelector('#srv-grid [data-cover]') as HTMLElement;
      return s.scrollHeight >= Math.ceil(2500 / (2 * c)) * cell.offsetHeight;
    }, cols);
    expect(tall).toBe(true);
    await key('ArrowRight');
    await waitFocused(`sg${FIRST_ID}`);
  });

  it('holding Down runs focus through the platform without a gap; the cover it stops on loads', async () => {
    const { most, lost } = await holdDown('srv-grid', 4000);
    const index = await focusedIndex();
    console.log(`held Down: focus at index ${index}, at most ${most} covers rendered`);
    expect(lost).toBe(false);
    expect(most).toBeLessThanOrEqual(MAX_RENDERED);
    // 400 ms, then a row every 90 ms: well past the first screenful.
    expect(index).toBeGreaterThan(60);
    await settle('srv-grid');
    await assertNoClipping();
    const ms = await msUntilFocusedCover(STOPPED_COVER_TIMEOUT + 1000);
    console.log(`the stopped cover loaded after ${ms} ms`);
    expect(ms).toBeLessThan(STOPPED_COVER_TIMEOUT);
    expect(await $(testId(`tv-cover-sg${FIRST_ID}`)).isExisting()).toBe(false);
  });

  it('moves across rows on the layout; Left from the first column goes to the selected rail row', async () => {
    await upOneRow();
    await stepLeftTo((await focusedCell()).rowStart);
    const k = await focused();
    await key('ArrowLeft');
    await waitFocused('r-1');
    await key('ArrowRight');
    await waitFocused(k!, 'Right did not come back to the cover deep in the grid');
  });

  it('comes back from Details to the same cover deep in the grid', async () => {
    const k = (await focused())!;
    await key('Enter');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never opened Details' });
    await key('Escape');
    await waitFocused(k, 'B did not come back to the cover');
    await assertNoClipping();
  });

  it('the Library windows its 600 games the same way', async () => {
    await key('q');
    await waitTab('library');
    await waitFocused('r-all');
    let n = 0;
    await browser.waitUntil(async () => (n = await rendered('lib-grid')) > 0, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the Library never drew its grid' });
    expect(n).toBeLessThanOrEqual(MAX_RENDERED);
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-lib-count"]')?.textContent)).toBe(`${LIBRARY_GAMES} games`);
    await key('ArrowRight');
    await browser.waitUntil(async () => (await focusedIndex()) === 0, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Right did not reach the first cover' });
    const { most, lost } = await holdDown('lib-grid', 3000);
    expect(lost).toBe(false);
    expect(most).toBeLessThanOrEqual(MAX_RENDERED);
    expect(await focusedIndex()).toBeGreaterThan(40);
    await settle('lib-grid');
    await assertNoClipping();
    // Back to the top: Up runs back, and the first cover scrolls the grid to its very top.
    await hold('dpad_up');
    await browser.waitUntil(async () => (await focusedIndex()) < 12, { timeout: 30_000, timeoutMsg: 'holding Up never reached the top rows' });
    await release('dpad_up');
    await settle('lib-grid');
    for (let i = 0; i < 4 && (await focusedCell()).top > 0; i++) await upOneRow();
    await stepLeftTo(0);
    await browser.waitUntil(
      async () => (await browser.execute(() => (document.getElementById('lib-grid')!.closest('[data-scroll]') as HTMLElement).scrollTop)) === 0,
      { timeout: TRANSITION_TIMEOUT },
    ).catch(async () => {
      const st = await browser.execute(() => {
        const s = document.getElementById('lib-grid')!.closest('[data-scroll]') as HTMLElement;
        const f = document.querySelector('[data-tv-root] .f');
        return { top: s.scrollTop, focused: f?.getAttribute('data-k'), index: f?.closest('[data-i]')?.getAttribute('data-i'), first: document.querySelector('#lib-grid [data-f]')?.getAttribute('data-k') };
      });
      throw new Error(`the first cover did not scroll the grid to its top: ${JSON.stringify(st)}`);
    });
  });

  it('had no page errors', async () => {
    const errs = (await browser.execute(() => (window as unknown as { __errs: string[] }).__errs)) as string[];
    expect(errs).toEqual([]);
  });
});
