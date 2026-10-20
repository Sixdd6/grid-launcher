import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const GAME_COUNT = 2500;
const FIRST_ID = 10001;
const LAST_ID = FIRST_ID + GAME_COUNT - 1;
/** Covers near the visible area only. Generous on purpose: what matters is
 *  that it is nowhere near GAME_COUNT. */
const MAX_COVERS_AT_OPEN = 300;
const COVER_TIMEOUT = 15_000;

/** The mock's live request log (`GET /__e2e__/requests`, no auth). */
async function smallCoverRequests(): Promise<number> {
  const res = await fetch(`${mockUrl()}/__e2e__/requests`);
  const log = (await res.json()) as { path: string }[];
  return log.filter((entry) => /\/cover\/small\.png$/.test(entry.path)).length;
}

/** Whether the cover `<img>` inside `game-card-<id>` has decoded. */
async function coverLoaded(id: number): Promise<boolean> {
  return browser.execute((sel: string) => {
    const img = document.querySelector(`${sel} img:not(.backdrop)`) as HTMLImageElement | null;
    return (img?.naturalWidth ?? 0) > 0;
  }, testId(`game-card-${id}`));
}

/**
 * Stage `large-platform`: a single 2500-game platform, selected by default
 * when the Server view opens. Before covers were bounded, every card fetched
 * its cover at mount — 2500 IPC calls and asset requests at once — and the
 * web process ran out of file descriptors and aborted (2026-10-20). Now only
 * cards near the visible area load, so opening the platform fetches a few
 * dozen covers, and scrolling to the end loads the last card's.
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

  it('renders every card but loads only the covers near the visible area', async () => {
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
    // Let anything still queued near the top finish before counting.
    await browser.pause(2_000);
    const atOpen = await smallCoverRequests();
    expect(atOpen).toBeGreaterThan(0);
    expect(atOpen).toBeLessThan(MAX_COVERS_AT_OPEN);
    expect(await coverLoaded(LAST_ID)).toBe(false);
  });

  it('loads the last card’s cover once it is scrolled into view', async () => {
    await browser.execute((sel: string) => {
      document.querySelector(sel)?.scrollIntoView({ block: 'center' });
    }, testId(`game-card-${LAST_ID}`));
    await browser.waitUntil(() => coverLoaded(LAST_ID), {
      timeout: COVER_TIMEOUT,
      timeoutMsg: 'the last card’s cover never loaded after scrolling to it',
    });
    // Still bounded: scrolling straight to the end skips the middle.
    expect(await smallCoverRequests()).toBeLessThan(2 * MAX_COVERS_AT_OPEN);
  });
});
