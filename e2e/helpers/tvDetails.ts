// Helpers the `tv-details` specs share (docs/tv-mode/plan.md P9).
import { TRANSITION_TIMEOUT } from './env.js';
import { focused, key, waitFocused } from './tv.js';

export const testId = (id: string) => `[data-testid="${id}"]`;

export async function text(id: string): Promise<string> {
  return ((await browser.execute((s: string) => document.querySelector(s)?.textContent ?? '', testId(id))) as string).replace(/\s+/g, ' ').trim();
}

export async function waitText(id: string, want: string | RegExp): Promise<void> {
  let last = '';
  await browser
    .waitUntil(
      async () => {
        last = await text(id);
        return typeof want === 'string' ? last === want : want.test(last);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${id} never read ${String(want)}` },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (it reads "${last}")`);
    });
}

export async function waitGone(id: string, timeoutMsg?: string): Promise<void> {
  await browser.waitUntil(async () => !(await $(testId(id)).isExisting()), { timeout: TRANSITION_TIMEOUT, timeoutMsg: timeoutMsg ?? `${id} never went away` });
}

/** Captures page errors from now on, for a spec's last test. */
export async function captureErrors(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as { __errs: string[] };
    w.__errs = [];
    addEventListener('error', (e) => w.__errs.push(`error: ${e.message} ${e.error?.stack ?? ''}`));
    addEventListener('unhandledrejection', (e) => w.__errs.push(`rejection: ${String(e.reason)} ${e.reason?.stack ?? ''}`));
    const ce = console.error.bind(console);
    console.error = (...a: unknown[]) => {
      w.__errs.push(`console: ${a.map((x) => (x instanceof Error ? `${x.message} ${x.stack}` : String(x))).join(' ')}`);
      ce(...a);
    };
  });
}

export async function pageErrors(): Promise<string[]> {
  return (await browser.execute(() => (window as unknown as { __errs: string[] }).__errs ?? [])) as string[];
}

/** The Library grid's cover for `title` (its `data-k`). */
export async function libCover(title: string): Promise<string> {
  let k: string | null = null;
  await browser.waitUntil(
    async () => {
      k = (await browser.execute(
        (t: string) =>
          Array.from(document.querySelectorAll('#lib-grid [data-cover]')).find((c) => c.querySelector('.cv-l')?.textContent?.trim() === t)?.getAttribute('data-k') ?? null,
        title,
      )) as string | null;
      return k !== null;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no Library cover for ${title}` },
  );
  return k!;
}

/** Presses `dir` until `k` is focused (at most `n` times). */
export async function moveTo(k: string, dir: string, n = 12): Promise<void> {
  for (let i = 0; i < n && (await focused()) !== k; i++) {
    await key(dir);
    await browser.pause(80);
  }
  await waitFocused(k);
}

/** Opens Details for the Library cover titled `title`: from the Library's
 *  rail (All games), into the grid, to the cover, A. */
export async function openLibraryDetails(title: string): Promise<void> {
  const k = await libCover(title);
  await key('ArrowRight');
  await browser.pause(100);
  for (let i = 0; i < 20 && (await focused()) !== k; i++) {
    const cur = await focused();
    // The grid reads left to right, top to bottom.
    const next = (await browser.execute(
      (a: string | null, b: string) => {
        const covers = Array.from(document.querySelectorAll('#lib-grid [data-cover]')) as HTMLElement[];
        const els = covers.map((c) => c.getAttribute('data-k'));
        const top = covers[0]?.getBoundingClientRect().top ?? 0;
        const cols = Math.max(1, covers.filter((c) => Math.abs(c.getBoundingClientRect().top - top) < 2).length);
        const ia = a ? els.indexOf(a) : -1;
        const ib = els.indexOf(b);
        if (ia >= 0 && Math.floor(ib / cols) > Math.floor(ia / cols)) return 'ArrowDown';
        if (ia >= 0 && Math.floor(ib / cols) < Math.floor(ia / cols)) return 'ArrowUp';
        return ib > ia ? 'ArrowRight' : 'ArrowLeft';
      },
      cur,
      k,
    )) as string;
    await key(next);
    await browser.pause(80);
  }
  await waitFocused(k);
  await key('Enter');
  await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Details never opened for ${title}` });
  await waitText('tv-det-title', title);
}

/** The Details buttons' labels, left to right. */
export async function detButtons(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('#det-btns [data-k]')).map((b) => (b.textContent ?? '').replace(/\s+/g, ' ').trim()),
  )) as string[];
}

export async function waitButtons(want: string[]): Promise<void> {
  let last: string[] = [];
  await browser
    .waitUntil(
      async () => {
        last = await detButtons();
        return JSON.stringify(last) === JSON.stringify(want);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `Details' buttons never read ${want.join(' | ')}` },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (they read ${last.join(' | ')})`);
    });
}

/** The tab body's scroll position. */
export async function bodyScroll(): Promise<number> {
  return (await browser.execute(() => (document.querySelector('#det-body') as HTMLElement | null)?.scrollTop ?? -1)) as number;
}

/** Types on the on-screen keyboard with the physical keyboard. */
export async function typeKeys(s: string): Promise<void> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  for (const ch of s) await browser.keys(ch);
}

export async function waitOskClosed(): Promise<void> {
  await browser.waitUntil(async () => !(await $(testId('tv-osk')).isExisting()), { timeout: TRANSITION_TIMEOUT });
}
