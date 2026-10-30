import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const GAME_COUNT = 2500;
const FIRST_ID = 10001;
const LAST_ID = FIRST_ID + GAME_COUNT - 1;
const COVER_TIMEOUT = 15_000;
/** How long the card the user stops on may take to show its cover. The mock
 *  answers each cover after 40ms (`--cover-delay-ms`) and the backend fetches
 *  six at a time; the first bounded queue, still serving the cards scrolled
 *  past, missed even 3s here. About 1.3–1.7s in the build container. */
const STOPPED_COVER_TIMEOUT = 2_500;
/** How long a cover already loaded once may take to show again after its
 *  card was unmounted and mounted again. */
const REMOUNTED_COVER_TIMEOUT = 1_000;

/** Whether the cover `<img>` inside `game-card-<id>` has decoded. */
async function coverLoaded(id: number): Promise<boolean> {
  return browser.execute((sel: string) => {
    const img = document.querySelector(`${sel} img:not(.backdrop)`) as HTMLImageElement | null;
    return (img?.naturalWidth ?? 0) > 0;
  }, testId(`game-card-${id}`));
}

/** The most cards the windowed grid may have in the DOM at once: the
 *  visible rows plus two buffer rows either side, at any window size the
 *  suite runs in. Every card of the platform used to be in it. */
const MAX_RENDERED_CARDS = 200;

/** How many cards `server-grid` has in the DOM. */
async function renderedCards(): Promise<number> {
  return browser.execute(
    (sel: string) => document.querySelector(sel)?.children.length ?? 0,
    testId('server-grid'),
  );
}

/** Scrolls the grid's container to `top` (or by `step` px at a time down to
 *  the end, the way a person browses) and returns the most cards the DOM
 *  held at any point on the way. */
async function scrollGrid(mode: { to: number } | { stepDown: number }): Promise<number> {
  return browser.execute(
    async (sel: string, to: number, step: number) => {
      const grid = document.querySelector(sel) as HTMLElement;
      let scroller: HTMLElement | null = grid.parentElement;
      while (scroller && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) {
        scroller = scroller.parentElement;
      }
      if (!scroller) throw new Error('no scroll container around the grid');
      const frame = () => new Promise((r) => setTimeout(r, 40));
      let most = grid.children.length;
      if (step > 0) {
        while (scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1) {
          scroller.scrollTop += step;
          await frame();
          most = Math.max(most, grid.children.length);
        }
      } else {
        scroller.scrollTop = to;
        await frame();
        most = Math.max(most, grid.children.length);
      }
      return most;
    },
    testId('server-grid'),
    'to' in mode ? mode.to : 0,
    'stepDown' in mode ? mode.stepDown : 0,
  );
}

/** Milliseconds, timed in the page, until `game-card-<id>`'s cover has
 *  decoded, or a little over `limit` if it never does. A WebDriver poll from
 *  outside adds its own interval and round trips. */
async function msUntilCover(id: number, limit: number): Promise<number> {
  return browser.execute(
    (sel: string, max: number) =>
      new Promise<number>((resolve) => {
        const t0 = performance.now();
        const poll = () => {
          const img = document.querySelector(`${sel} img:not(.backdrop)`) as HTMLImageElement | null;
          const now = performance.now() - t0;
          if ((img?.naturalWidth ?? 0) > 0 || now > max) resolve(Math.round(now));
          else requestAnimationFrame(poll);
        };
        poll();
      }),
    testId(`game-card-${id}`),
    limit,
  );
}

/**
 * Stage `large-platform`: a single 2500-game platform, selected by default
 * when the Server view opens.
 *
 * - Before covers were bounded, every card fetched its cover at mount — 2500
 *   IPC calls and asset requests at once — and the web process ran out of
 *   file descriptors and aborted (2026-10-20).
 * - The first bound served cards in the order they came near, and never
 *   dropped one the user had scrolled past, so after browsing, the cards on
 *   screen waited behind every card passed on the way (2026-10-20).
 * - With every card in the DOM, style, layout and paint took over 200 ms a
 *   frame for scroll and hover alike, so the grid renders only the rows near
 *   the visible part, and its padding stands in for the rest (2026-10-20).
 */
describe('large platform: windowed grid, bounded cover loading', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the platform rail never appeared after connecting',
    });
  });

  it('renders only the rows near the screen, at the full list’s height', async () => {
    await $(testId(`game-card-${FIRST_ID}`)).waitForExist({
      timeout: COVER_TIMEOUT,
      timeoutMsg: 'the 2500-game platform never rendered its grid',
    });
    const cards = await renderedCards();
    console.log(`${cards} of ${GAME_COUNT} cards in the DOM at the top`);
    expect(cards).toBeLessThanOrEqual(MAX_RENDERED_CARDS);
    expect(await $(testId(`game-card-${LAST_ID}`)).isExisting()).toBe(false);

    // The grid is as tall as every card laid out (`cards/layout.ts`), so
    // the scrollbar spans the whole list though only a screenful renders:
    // at the very end of the scroll, the last card sits on the grid's
    // bottom edge (its 24px padding below it, label included).
    await scrollGrid({ to: 1e9 });
    await $(testId(`game-card-${LAST_ID}`)).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the last card never rendered at the end of the scroll',
    });
    const gap = await browser.execute(
      (gridSel: string, lastSel: string) => {
        const grid = (document.querySelector(gridSel) as HTMLElement).getBoundingClientRect();
        const last = (document.querySelector(lastSel) as HTMLElement).getBoundingClientRect();
        return grid.bottom - last.bottom;
      },
      testId('server-grid'),
      testId(`game-card-${LAST_ID}`),
    );
    expect(Math.abs(gap - 24)).toBeLessThanOrEqual(2);
    await scrollGrid({ to: 0 });

    await browser.waitUntil(() => coverLoaded(FIRST_ID), {
      timeout: COVER_TIMEOUT,
      timeoutMsg: 'the first card’s cover never loaded',
    });
  });

  it('shows the covers where the user stops, after scrolling through the platform', async () => {
    // Every card passes the near zone on the way to the last one.
    const most = await scrollGrid({ stepDown: 600 });
    console.log(`at most ${most} cards in the DOM while scrolling`);

    const elapsed = await msUntilCover(LAST_ID, STOPPED_COVER_TIMEOUT + 1_000);
    console.log(`last card's cover loaded ${elapsed}ms after the scroll stopped`);
    expect(elapsed).toBeLessThan(STOPPED_COVER_TIMEOUT);

    expect(most).toBeLessThanOrEqual(MAX_RENDERED_CARDS);
    expect(await renderedCards()).toBeLessThanOrEqual(MAX_RENDERED_CARDS);
    expect(await $(testId(`game-card-${FIRST_ID}`)).isExisting()).toBe(false);
  });

  it('shows a cover it has loaded at once when its card comes back', async () => {
    await scrollGrid({ to: 0 });
    // The card mounted again as a new element, but the file is remembered:
    // no slot, no backend call, just the decode of a file on disk.
    const elapsed = await msUntilCover(FIRST_ID, STOPPED_COVER_TIMEOUT);
    console.log(`first card's cover back ${elapsed}ms after scrolling to the top`);
    expect(elapsed).toBeLessThan(REMOUNTED_COVER_TIMEOUT);
  });
});
