import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl } from '../helpers/env.js';

/**
 * Stage `large-platform-perf` (opt-in, measurement only): how a large
 * platform loads, scrolls and hovers in the real webview. Nothing here
 * asserts a number — every result is a `PERF …` log line, compared run to run
 * (BUILD.md "Performance probe"). Run it on a release build
 * (`E2E_RELEASE=1`): a debug build parses JSON several times slower.
 *
 * Against the mock it uses the `large-platform` fixture (2500 generated
 * games). With E2E_LIVE_URL, E2E_LIVE_TOKEN and E2E_LIVE_PLATFORM it connects
 * to a real RomM server instead, network included; the token is read from
 * the environment and typed into the Connect form by the test, never logged.
 *
 * Frame times are requestAnimationFrame intervals: ~16ms is 60fps, and a
 * longer one means the main thread was busy that long. The container renders
 * in software under Xvfb, so compare runs with each other, not with a desktop.
 */

const testId = (id: string) => `[data-testid="${id}"]`;

const LIVE_URL = process.env.E2E_LIVE_URL ?? '';
const PLATFORM_ID = Number(process.env.E2E_LIVE_PLATFORM ?? '1');
const PLATFORM_BTN = testId(`platform-btn-${PLATFORM_ID}`);
const GRID = testId('server-grid');
/** The window size, `WIDTHxHEIGHT` (E2E_WINDOW); 1600x1000 by default. The
 *  test display is 4K, so up to 3840x2160 works. */
const [WINDOW_W, WINDOW_H] = (process.env.E2E_WINDOW ?? '1600x1000').split('x').map(Number);

/** Starts recording frame intervals in the page. */
async function startFrames(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as { __frames: number[]; __recording: boolean };
    w.__frames = [];
    w.__recording = true;
    let last = performance.now();
    const tick = (now: number) => {
      if (!w.__recording) return;
      w.__frames.push(now - last);
      last = now;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}

/** Stops recording and logs the frame-time distribution under `label`. */
async function stopFrames(label: string): Promise<void> {
  const frames = await browser.execute(() => {
    const w = window as unknown as { __frames: number[]; __recording: boolean };
    w.__recording = false;
    return w.__frames.slice(1);
  });
  const sorted = [...frames].sort((a, b) => a - b);
  const pct = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))] ?? 0;
  const total = frames.reduce((a, b) => a + b, 0);
  console.log(
    `PERF ${label}: ${frames.length} frames over ${Math.round(total)}ms · ` +
      `p50 ${pct(0.5).toFixed(1)} · p95 ${pct(0.95).toFixed(1)} · max ${pct(1).toFixed(1)}ms · ` +
      `>33ms: ${frames.filter((f) => f > 33).length} · >100ms: ${frames.filter((f) => f > 100).length}`,
  );
}

/** What the grid is painting: cards, covers, skeletons. */
async function census(label: string): Promise<void> {
  const c = await browser.execute((sel: string) => {
    const grid = document.querySelector(sel) as HTMLElement;
    return {
      cards: grid.children.length,
      covers: grid.querySelectorAll('img').length,
      loading: grid.querySelectorAll('.skeleton').length,
    };
  }, GRID);
  console.log(`PERF census ${label}: ${JSON.stringify(c)}`);
}

/** Puts the grid's scroll container at `fraction` of its height. */
async function scrollTo(fraction: number): Promise<void> {
  await browser.execute(
    (sel: string, frac: number) => {
      const grid = document.querySelector(sel) as HTMLElement;
      let sc: HTMLElement | null = grid.parentElement;
      while (sc && !/(auto|scroll)/.test(getComputedStyle(sc).overflowY)) sc = sc.parentElement;
      // No scrolling box around the grid: the page itself scrolls.
      if (!sc) sc = document.scrollingElement as HTMLElement;
      sc.scrollTop = sc.scrollHeight * frac;
    },
    GRID,
    fraction,
  );
}

/** Scrolls the grid's container by `step` px every animation frame for `ms`,
 *  the way a wheel or trackpad fling does. */
async function fling(step: number, ms: number): Promise<void> {
  await browser.execute(
    async (sel: string, px: number, dur: number) => {
      const grid = document.querySelector(sel) as HTMLElement;
      let sc: HTMLElement | null = grid.parentElement;
      while (sc && !/(auto|scroll)/.test(getComputedStyle(sc).overflowY)) sc = sc.parentElement;
      // No scrolling box around the grid: the page itself scrolls.
      if (!sc) sc = document.scrollingElement as HTMLElement;
      const end = performance.now() + dur;
      await new Promise<void>((resolve) => {
        const tick = () => {
          sc!.scrollTop += px;
          if (performance.now() < end) requestAnimationFrame(tick);
          else resolve();
        };
        requestAnimationFrame(tick);
      });
    },
    GRID,
    step,
    ms,
  );
}

/** Times a platform click: to the first card, the first cover and ten covers. */
async function timeOpen(label: string): Promise<void> {
  const t = await browser.execute(
    (btn: string, gridSel: string) =>
      new Promise<{ firstCard: number; firstCover: number; tenCovers: number; longest: number }>((resolve) => {
        const t0 = performance.now();
        const old = document.querySelector(gridSel)?.firstElementChild ?? null;
        let firstCard = -1;
        let firstCover = -1;
        let longest = 0;
        let last = t0;
        const tick = (now: number) => {
          longest = Math.max(longest, now - last);
          last = now;
          const g = document.querySelector(gridSel);
          const first = g?.firstElementChild ?? null;
          if (firstCard < 0 && first && first !== old) firstCard = now - t0;
          const loaded =
            firstCard < 0 || !g
              ? 0
              : Array.from(g.querySelectorAll('img:not(.backdrop)')).filter(
                  (i) => (i as HTMLImageElement).naturalWidth > 0,
                ).length;
          if (firstCover < 0 && loaded > 0) firstCover = now - t0;
          if (loaded >= 10 || now - t0 > 60_000) {
            resolve({
              firstCard: Math.round(firstCard),
              firstCover: Math.round(firstCover),
              tenCovers: Math.round(now - t0),
              longest: Math.round(longest),
            });
          } else requestAnimationFrame(tick);
        };
        (document.querySelector(btn) as HTMLElement).click();
        requestAnimationFrame(tick);
      }),
    PLATFORM_BTN,
    GRID,
  );
  console.log(
    `PERF ${label}: first card ${t.firstCard}ms · first cover ${t.firstCover}ms · ` +
      `10 covers ${t.tenCovers}ms · longest frame ${t.longest}ms`,
  );
}

/**
 * Pointer browsing, driven from inside the page: WebDriver pointer moves do
 * not reach this WebKitGTK page (no mouseenter, nothing :hover). Each card in
 * the visible rows in turn gets the :hover look (scale, overlay, Play,
 * actions) and the real mouseenter/mouseleave its handlers listen to, with a
 * window pointermove about every frame while the pointer travels, then
 * `restMs` without moving. Logs frame times and how often the background art
 * layer changed.
 */
async function browse(label: string, moveMs: number, restMs = 0): Promise<void> {
  await scrollTo(0);
  await browser.execute(() => {
    if (document.getElementById('perf-hover-css')) return;
    const st = document.createElement('style');
    st.id = 'perf-hover-css';
    st.textContent =
      '[data-testid^="game-card-"].perf-hover{z-index:2}' +
      '.perf-hover .cover{transform:translateY(-4px)!important}' +
      '.perf-hover .shade,.perf-hover .bar{opacity:1!important}';
    document.head.appendChild(st);
  });
  await browser.pause(3000);
  await startFrames();
  const r = await browser.execute(
    async (sel: string, move: number, rest: number) => {
      const grid = document.querySelector(sel) as HTMLElement;
      const top = grid.parentElement!.getBoundingClientRect().top;
      const cards = Array.from(grid.children).filter((c) => {
        const b = c.getBoundingClientRect();
        return b.top > top + 10 && b.bottom < window.innerHeight - 40;
      }).slice(0, 40) as HTMLElement[];
      let artChanges = 0;
      const obs = new MutationObserver((recs) => (artChanges += recs.length));
      document
        .querySelectorAll('[data-testid="background-art"] > *')
        .forEach((l) => obs.observe(l, { attributes: true, attributeFilter: ['style', 'class'] }));
      const sleep = (ms: number) => new Promise((res) => setTimeout(res, ms));
      let prev: HTMLElement | null = null;
      for (const c of cards) {
        if (prev) {
          prev.classList.remove('perf-hover');
          prev.dispatchEvent(new MouseEvent('mouseleave'));
        }
        c.classList.add('perf-hover');
        c.dispatchEvent(new MouseEvent('mouseenter'));
        prev = c;
        const b = c.getBoundingClientRect();
        const until = performance.now() + move;
        let x = b.left + 4;
        while (performance.now() < until) {
          x = Math.min(b.right - 4, x + 6);
          window.dispatchEvent(new PointerEvent('pointermove', { clientX: x, clientY: b.top + b.height / 3 }));
          await sleep(16);
        }
        if (rest > 0) await sleep(rest);
      }
      prev?.classList.remove('perf-hover');
      prev?.dispatchEvent(new MouseEvent('mouseleave'));
      await sleep(800);
      obs.disconnect();
      return { cards: cards.length, artChanges };
    },
    GRID,
    moveMs,
    restMs,
  );
  const how = `${r.cards} cards, ${moveMs}ms moving${restMs ? ` + ${restMs}ms resting` : ''} each`;
  await stopFrames(`${label} (${how}; art layer changes: ${r.artChanges})`);
}

describe('large platform: performance probe (measurement only)', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT });
    await $(testId('connect-server-url')).setValue(LIVE_URL || mockUrl());
    await $(testId('connect-secret')).setValue(LIVE_URL ? (process.env.E2E_LIVE_TOKEN ?? '') : FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(PLATFORM_BTN).waitForExist({ timeout: 30_000 });
    await browser.setWindowSize(WINDOW_W, WINDOW_H);
    console.log(`PERF mode: ${LIVE_URL ? 'live server' : 'mock'}, platform ${PLATFORM_ID}, window ${WINDOW_W}x${WINDOW_H}`);
    await timeOpen('first open');
    await browser.waitUntil(
      () => browser.execute((sel: string) => (document.querySelector(sel)?.children.length ?? 0) > 0, GRID),
      { timeout: 60_000 },
    );
  });

  it('idle baseline', async () => {
    await browser.pause(3000);
    await startFrames();
    await browser.pause(2000);
    await stopFrames('idle');
    await census('top, settled');
  });

  it('re-selecting the platform', async () => {
    await timeOpen('reselect');
  });

  it('the backend call alone: list_games over IPC', async () => {
    for (let i = 0; i < 3; i++) {
      const r = await browser.execute(async (pid: number) => {
        const internals = (
          window as unknown as {
            __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown[]> };
          }
        ).__TAURI_INTERNALS__;
        const t0 = performance.now();
        const games = await internals.invoke('list_games', { platformId: pid });
        return { ms: Math.round(performance.now() - t0), n: games.length, bytes: JSON.stringify(games).length };
      }, PLATFORM_ID);
      console.log(`PERF list_games: ${r.ms}ms for ${r.n} games, ${(r.bytes / 1e6).toFixed(2)} MB over IPC`);
    }
  });

  it('scrolling onto rows not seen yet (covers still loading)', async () => {
    for (const at of [0.2, 0.5]) {
      await scrollTo(at);
      await browser.pause(2500);
      await startFrames();
      await fling(60, 2500);
      await stopFrames(`cold scroll from ${at * 100}%`);
      await census(`after cold scroll from ${at * 100}%`);
    }
  });

  it('steady and fast scrolling from the top', async () => {
    await scrollTo(0);
    await browser.pause(1500);
    await startFrames();
    await fling(60, 4000);
    await stopFrames('scroll 60px/frame');
    await browser.pause(1500);
    await startFrames();
    await fling(400, 3000);
    await stopFrames('scroll 400px/frame');
  });

  it('jumping to the middle (the whole window remounts)', async () => {
    await browser.pause(1500);
    await startFrames();
    await scrollTo(0.5);
    await browser.pause(1500);
    await stopFrames('jump to middle');
  });

  it('which style costs the scrolled frames (A/B, one suspect off at a time)', async function () {
    this.timeout(180_000);
    const variants: [string, string][] = [
      ['as shipped', ''],
      ['no background art', '[data-testid="background-art"]{display:none!important}'],
      ['cards hidden (what painting them costs)', '[data-testid="server-grid"] > *{visibility:hidden!important}'],
      ['art at full opacity', '[data-testid="background-art"] .layer.visible{opacity:1!important}'],
      [
        'art at full opacity on its own layer',
        '[data-testid="background-art"]{will-change:transform}[data-testid="background-art"] .layer.visible{opacity:1!important}',
      ],
      [
        'art at full opacity, art and scroller layers',
        '[data-testid="background-art"]{will-change:transform}[data-testid="background-art"] .layer.visible{opacity:1!important}[data-testid="server-section"] .body{will-change:transform}',
      ],
    ];
    const art = await browser.execute(() =>
      [...document.querySelectorAll('[data-testid="background-art"] .layer')].map((l) => {
        const cs = getComputedStyle(l);
        return `${cs.backgroundImage.slice(-60)} size=${cs.backgroundSize} opacity=${cs.opacity}`;
      }),
    );
    console.log(`PERF art layers: ${JSON.stringify(art)}`);
    // Warm the band the variants scroll through first, so the first variant
    // is not the one that pays for decoding its covers.
    await scrollTo(0.3);
    await browser.pause(1500);
    await fling(60, 2500);
    await browser.pause(1500);
    // Each variant twice, in two passes, so drift over the run (a cover
    // still arriving, the container's other load) lands on all of them.
    for (const [name, css] of [...variants, ...variants]) {
      await browser.execute((text: string) => {
        let st = document.getElementById('perf-ab-css');
        if (!st) {
          st = document.createElement('style');
          st.id = 'perf-ab-css';
          document.head.appendChild(st);
        }
        st.textContent = text;
      }, css);
      await scrollTo(0.3);
      await browser.pause(1500);
      await startFrames();
      await fling(60, 2500);
      await stopFrames(`A/B scroll 60px/frame, ${name}`);
      if (process.env.E2E_SCREENSHOTS) {
        const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'perf-ab');
        mkdirSync(dir, { recursive: true });
        await browser.saveScreenshot(path.join(dir, `${name.replace(/[^a-z0-9]+/gi, '-')}.png`));
      }
    }
    await browser.execute(() => document.getElementById('perf-ab-css')?.remove());
  });

  it('scrolling by transform instead of by scroll position (what repainting costs)', async () => {
    // Within the rendered band only (the window renders 600px past the
    // screen), so every card it shows is already in the DOM: what differs from
    // a real scroll is only that nothing is scrolled, and so nothing should
    // need repainting.
    await scrollTo(0.3);
    await browser.pause(1500);
    for (const layer of [false, true]) {
      await startFrames();
      await browser.execute(
        async (sel: string, promote: boolean) => {
          const grid = document.querySelector(sel) as HTMLElement;
          if (promote) grid.style.willChange = 'transform';
          const end = performance.now() + 2500;
          let y = 0;
          let dir = -1;
          await new Promise<void>((resolve) => {
            const tick = () => {
              y += dir * 60;
              if (y <= -480 || y >= 0) dir = -dir;
              grid.style.transform = `translateY(${y}px)`;
              if (performance.now() < end) requestAnimationFrame(tick);
              else resolve();
            };
            requestAnimationFrame(tick);
          });
          grid.style.transform = '';
          grid.style.willChange = '';
        },
        GRID,
        layer,
      );
      await stopFrames(`transform scroll 60px/frame${layer ? ', grid on its own layer' : ''}`);
    }
  });

  it('pointer browsing across cards', async () => {
    // At most 40 visible cards in turn (`browse`), so the resting pass fits
    // the suite's two-minute limit at 4K too.
    await browse('hover flick', 30);
    await browse('hover browse', 150);
    await browse('hover rest', 200, 1300);
  });
});
