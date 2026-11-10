import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, hints, hold, invoke, key, press, release, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, openLibraryDetails, pageErrors, testId, waitGone, waitText } from '../helpers/tvDetails.js';

type Rect = { left: number; right: number; top: number; bottom: number; width: number; height: number };

async function rect(id: string): Promise<Rect | null> {
  return (await browser.execute((t: string) => {
    const r = document.querySelector(`[data-testid="${t}"]`)?.getBoundingClientRect();
    return r ? { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height } : null;
  }, id)) as Rect | null;
}

async function attr(id: string, name: string): Promise<string | null> {
  return (await browser.execute((t: string, n: string) => document.querySelector(`[data-testid="${t}"]`)?.getAttribute(n) ?? null, id, name)) as
    | string
    | null;
}

/** Waits for the trailer's `data-phase` to read `phase`; the error lists
 *  what it read on the way (phase / reason / linked). */
async function waitPhase(phase: string, what: string): Promise<void> {
  const seen: string[] = [];
  await browser
    .waitUntil(
      async () => {
        const now = `${await attr('tv-viewer-trailer', 'data-phase')}/${await attr('tv-viewer-trailer', 'data-reason')}/${await attr('tv-viewer-trailer', 'data-linked')}`;
        if (seen[seen.length - 1] !== now) seen.push(now);
        return now.startsWith(`${phase}/`);
      },
      { timeout: 12_000, interval: 200 },
    )
    .catch(() => {
      throw new Error(`${what} (phase/reason/linked seen: ${seen.join(' → ')})`);
    });
}

async function storedVolume(): Promise<number> {
  return (await invoke<{ video_volume: number }>('get_ui_settings')).video_volume;
}

/**
 * Stage `tv-details`, spec d: the media viewer's videos (user decisions
 * 2026-11-10). SaveSyncManual has a YouTube trailer and a server video
 * (fixtures-tv-details). The trailer plays through GRID's loopback relay
 * page; e2e runs point its player script at a dead loopback port
 * (wdio.conf.ts), so it can't load, the relay says so, and the viewer falls back to the poster and "Watch on
 * YouTube" before the 10 s ready timeout. The server video takes A, LB / RB
 * (seek), LT / RT (its own volume, held to repeat, stored as
 * `ui.video_volume`) and Left / Right (page), and nothing covers it.
 */
describe('tv-details (d): the media viewer plays videos', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
    await captureErrors();
    await waitTab('server');
    await connectPad(XBOX_PAD);
    await key('q');
    await waitTab('library');
    await waitFocused('r-all');
  });

  it('a trailer that cannot play falls back to its poster, with Watch on YouTube under it', async () => {
    await openLibraryDetails('SaveSyncManual');
    await press('right_bumper');
    await waitFocused('m0', 'RB from Overview did not reach Media and its trailer tile');
    await press('south');
    await $(testId('tv-viewer')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the media viewer never opened' });
    await waitText('tv-viewer-caption', 'Trailer · SaveSyncManual');
    await waitPhase('fallback', 'the trailer never fell back to its poster');
    // The relay reported the failure (not the timeout), after the hello
    // handshake: messages passed both ways, origin and window checked.
    expect(await attr('tv-viewer-trailer', 'data-reason')).toBe('error');
    expect(await attr('tv-viewer-trailer', 'data-linked')).toBe('true');
    await expect($(testId('tv-viewer-embed'))).not.toExist();
    await waitText('tv-viewer-trailer-note', "This trailer can't play in GRID.");
    const poster = (await rect('tv-viewer-trailer'))!;
    const watch = (await rect('tv-viewer-watch'))!;
    expect(watch.top).toBeGreaterThanOrEqual(poster.bottom);
    // A would open the browser here, so it is not pressed.
    expect(await hints()).toEqual(['A Watch on YouTube', 'LB Previous', 'RB Next', 'B Close']);
  });

  it('a server video: LB / RB seek, LT / RT set its own volume (held, they repeat), Left / Right page', async () => {
    await press('right_bumper');
    await waitText('tv-viewer-caption', 'Trailer · SaveSyncManual');
    await $(testId('tv-viewer-video')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'RB did not page to the server video' });
    await $(testId('tv-viewer-videoel')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the server video never resolved' });
    await browser.waitUntil(async () => (await hints()).includes('LT Volume −'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the video hints never showed',
    });
    const h = await hints();
    expect(h.slice(1)).toEqual(['LB Back 10 s', 'RB Forward 10 s', 'LT Volume −', 'RT Volume +', 'B Close']);
    expect(h[0]).toMatch(/^A (Play|Pause)$/);

    // The player opens at the stored level (default 50).
    const start = await storedVolume();
    expect(start).toBe(50);
    expect(await browser.execute(() => (document.querySelector('[data-testid="tv-viewer-videoel"]') as HTMLVideoElement).volume)).toBeCloseTo(0.5);

    // RT: one step, the readout beside the player, and it is stored.
    await press('right_trigger');
    await waitText('tv-viewer-volume', 'Volume 60%');
    expect(await browser.execute(() => (document.querySelector('[data-testid="tv-viewer-videoel"]') as HTMLVideoElement).volume)).toBeCloseTo(0.6);
    await browser.waitUntil(async () => (await storedVolume()) === 60, { timeout: 3_000, timeoutMsg: 'the volume was never stored' });

    // Held LT repeats like a direction: 400 ms, then every 90 ms.
    await hold('left_trigger');
    await browser.pause(700);
    await release('left_trigger');
    const held = Number(((await $(testId('tv-viewer-volume')).getText()).match(/(\d+)%/) ?? [])[1]);
    expect(held).toBeLessThanOrEqual(30);
    await browser.waitUntil(async () => (await storedVolume()) === held, { timeout: 3_000, timeoutMsg: 'the held volume was never stored' });

    // Nothing covers the player: the readout is beside it, the
    // notification column and the hint bar clear of it.
    const player = (await rect('tv-viewer-video'))!;
    const vol = (await rect('tv-viewer-volume'))!;
    const toasts = (await rect('tv-toasts'))!;
    const bar = (await rect('tv-hints'))!;
    expect(player.width).toBeGreaterThanOrEqual(480);
    expect(player.height).toBeGreaterThanOrEqual(270);
    expect(vol.left).toBeGreaterThanOrEqual(player.right);
    expect(toasts.left).toBeGreaterThanOrEqual(player.right);
    expect(bar.top).toBeGreaterThanOrEqual(player.bottom);

    // LB / RB seek inside the video rather than paging.
    await press('left_bumper');
    await browser.pause(200);
    expect(await $(testId('tv-viewer-video')).isExisting()).toBe(true);

    // Left pages back to the trailer, which gets its own try.
    await press('dpad_left');
    await $(testId('tv-viewer-trailer')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Left did not page back to the trailer' });
    await waitPhase('fallback', 'the trailer never fell back the second time');

    // B closes the viewer; focus is back on the tile.
    await press('east');
    await waitGone('tv-viewer');
    await waitFocused('m0');
    // Put the volume back for later runs on this profile.
    await invoke('set_ui_settings', { settings: { ...(await invoke<Record<string, unknown>>('get_ui_settings')), video_volume: 50 } });
    expect(await focused()).toBe('m0');
    expect(await pageErrors()).toEqual([]);
  });
});
