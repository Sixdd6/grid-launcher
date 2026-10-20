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

/** Whether the cover `<img>` inside `game-card-<id>` has decoded. */
async function coverLoaded(id: number): Promise<boolean> {
  return browser.execute((sel: string) => {
    const img = document.querySelector(`${sel} img:not(.backdrop)`) as HTMLImageElement | null;
    return (img?.naturalWidth ?? 0) > 0;
  }, testId(`game-card-${id}`));
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
 */
describe('large platform: bounded cover loading', () => {
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

  it('renders every card and loads the covers at the top first', async () => {
    await $(testId(`game-card-${LAST_ID}`)).waitForExist({
      timeout: COVER_TIMEOUT,
      timeoutMsg: 'the 2500-game platform never finished rendering its grid',
    });
    const cards = await browser.execute(
      (sel: string) => document.querySelector(sel)?.children.length ?? 0,
      testId('server-grid'),
    );
    expect(cards).toBe(GAME_COUNT);

    await browser.waitUntil(() => coverLoaded(FIRST_ID), {
      timeout: COVER_TIMEOUT,
      timeoutMsg: 'the first card’s cover never loaded',
    });
    expect(await coverLoaded(LAST_ID)).toBe(false);
  });

  it('shows the covers where the user stops, after scrolling through the platform', async () => {
    // Scroll the grid's container down in steps, the way a person browses,
    // until the last card is on screen. Every card passes the near zone.
    await browser.execute(async (sel: string) => {
      const grid = document.querySelector(sel) as HTMLElement;
      let scroller: HTMLElement | null = grid.parentElement;
      while (scroller && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) {
        scroller = scroller.parentElement;
      }
      if (!scroller) throw new Error('no scroll container around the grid');
      while (scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1) {
        scroller.scrollTop += 600;
        await new Promise((r) => setTimeout(r, 40));
      }
    }, testId('server-grid'));

    // Timed in the page, from the end of the scroll to the cover's `load`:
    // a WebDriver poll from outside adds its own interval and round trips.
    const elapsed = await browser.execute(
      (sel: string, limit: number) =>
        new Promise<number>((resolve) => {
          const t0 = performance.now();
          const poll = () => {
            const img = document.querySelector(`${sel} img:not(.backdrop)`) as HTMLImageElement | null;
            const now = performance.now() - t0;
            if ((img?.naturalWidth ?? 0) > 0 || now > limit) resolve(Math.round(now));
            else requestAnimationFrame(poll);
          };
          poll();
        }),
      testId(`game-card-${LAST_ID}`),
      STOPPED_COVER_TIMEOUT + 1_000,
    );
    console.log(`last card's cover loaded ${elapsed}ms after the scroll stopped`);
    expect(elapsed).toBeLessThan(STOPPED_COVER_TIMEOUT);
  });
});
