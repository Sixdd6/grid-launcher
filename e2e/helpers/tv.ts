/**
 * TV mode helpers (docs/tv-mode/plan.md § 2): controllers through
 * `e2e_gamepad_inject` (the same mapping and gate a real controller goes
 * through), the keyboard, and reads of the focused item and the hint bar.
 */
import { TRANSITION_TIMEOUT } from './env.js';

/** A controller as the Rust reader reports it (`PadInfo`). */
export type Pad = { id: number; key: string; name: string; vendor: number | null; product: number | null };

export type PadButton =
  | 'south' | 'east' | 'west' | 'north' | 'left_bumper' | 'right_bumper' | 'left_trigger' | 'right_trigger'
  | 'select' | 'start' | 'guide' | 'left_stick' | 'right_stick' | 'dpad_up' | 'dpad_down' | 'dpad_left' | 'dpad_right';

export const XBOX_PAD: Pad = {
  id: 7,
  key: '030000005e040000130b000005057200',
  name: 'Xbox Wireless Controller',
  vendor: 0x045e,
  product: 0x0b13,
};

export const NINTENDO_PAD: Pad = {
  id: 8,
  key: '030000007e0500000920000011810000',
  name: 'Pro Controller (Nintendo)',
  vendor: 0x057e,
  product: 0x2009,
};

/** Calls a Tauri command from the page. */
export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  return (await browser.execute(
    (c: string, a: Record<string, unknown>) =>
      (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__.invoke(c, a),
    cmd,
    args,
  )) as T;
}

export async function inject(events: unknown[]): Promise<void> {
  await invoke('e2e_gamepad_inject', { events });
}

export async function connectPad(pad: Pad): Promise<void> {
  await inject([{ kind: 'focus', focused: true }, { kind: 'connect', pad }]);
}

export async function disconnectPad(pad: Pad): Promise<void> {
  await inject([{ kind: 'disconnect', id: pad.id }]);
}

/** One press (down and up) of a physical button on `pad`. */
export async function press(button: PadButton, pad: Pad = XBOX_PAD): Promise<void> {
  await inject([
    { kind: 'input', id: pad.id, event: { kind: 'pressed', button } },
    { kind: 'input', id: pad.id, event: { kind: 'released', button } },
  ]);
}

export async function hold(button: PadButton, pad: Pad = XBOX_PAD): Promise<void> {
  await inject([{ kind: 'input', id: pad.id, event: { kind: 'pressed', button } }]);
}

export async function release(button: PadButton, pad: Pad = XBOX_PAD): Promise<void> {
  await inject([{ kind: 'input', id: pad.id, event: { kind: 'released', button } }]);
}

export async function axis(name: string, value: number, pad: Pad = XBOX_PAD): Promise<void> {
  await inject([{ kind: 'input', id: pad.id, event: { kind: 'axis', axis: name, value } }]);
}

/** A key on the keyboard that stands in for a controller. */
export async function key(k: string): Promise<void> {
  await browser.keys(k);
}

/** The `data-k` of the focused item (`.f`), or null. */
export async function focused(): Promise<string | null> {
  return (await browser.execute(() => document.querySelector('[data-tv-root] .f')?.getAttribute('data-k') ?? null)) as string | null;
}

export async function waitFocused(k: string, timeoutMsg?: string): Promise<void> {
  let last: string | null = null;
  await browser.waitUntil(
    async () => {
      last = await focused();
      return last === k;
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: timeoutMsg ?? `focus never reached ${k}` },
  ).catch((e) => {
    throw new Error(`${(e as Error).message} (focused: ${last})`);
  });
}

/** The hint bar's right side, as "<button> <label>" strings. */
export async function hints(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-hints"] [data-btn]')).map(
      (h) => `${h.getAttribute('data-btn')} ${(h.textContent ?? '').trim()}`,
    ),
  )) as string[];
}

/** The prompt set the glyphs draw from now. */
export async function promptSet(): Promise<string | null> {
  return (await browser.execute(() => document.querySelector('[data-testid="tv-hints"] .gli')?.getAttribute('data-set') ?? null)) as string | null;
}

export async function hintLeft(): Promise<string> {
  return (await $('[data-testid="tv-hints-left"]').getText()).trim();
}

export async function waitHintLeft(text: string): Promise<void> {
  await browser.waitUntil(async () => (await hintLeft()) === text, {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the hint bar never read "${text}"`,
  });
}

export async function waitPromptSet(set: string): Promise<void> {
  await browser.waitUntil(async () => (await promptSet()) === set, {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the prompts never became ${set}`,
  });
}

/** The current top-bar tab. */
export async function currentTab(): Promise<string | null> {
  return (await browser.execute(() => document.querySelector('[data-testid="tv-tabs"] .tab.on')?.getAttribute('data-tab') ?? null)) as string | null;
}

export async function waitTab(tab: string): Promise<void> {
  await browser.waitUntil(async () => (await currentTab()) === tab, {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `TV mode never showed the ${tab} tab`,
  });
}

/**
 * The focused item, its ring and a cover's growth, cut by no scrolling or
 * clipping area around it (plan risk R7; the prototype's clipping checker:
 * the focused element's ring box, inflated by 0.42rem, must sit inside every
 * ancestor whose overflow isn't visible). A cover is measured by its art,
 * which grows to 107% with a .42rem ring. Waits for scrolls to settle;
 * answers nothing, throws with what is cut.
 */
export async function assertNoClipping(): Promise<void> {
  let last = '';
  await browser
    .waitUntil(
      async () => {
        last = (await browser.execute(() => {
          const el = document.querySelector<HTMLElement>('[data-tv-root] .f');
          if (!el) return 'nothing is focused';
          const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
          // The ring box: a cover's art (it grows to 107% with a .42rem
          // ring), else the item (a .2rem outline at a .2rem offset), each
          // inflated by .42rem as the prototype's checker does.
          const art = el.querySelector('.cv-art');
          const r = (art ?? el).getBoundingClientRect();
          const pad = 0.42 * rem;
          const box = { left: r.left - pad, top: r.top - pad, right: r.right + pad, bottom: r.bottom + pad };
          for (let p = el.parentElement; p && !p.hasAttribute('data-tv-root'); p = p.parentElement) {
            const cs = getComputedStyle(p);
            const clipX = cs.overflowX !== 'visible';
            const clipY = cs.overflowY !== 'visible';
            if (!clipX && !clipY) continue;
            const pr = p.getBoundingClientRect();
            const left = pr.left + p.clientLeft;
            const top = pr.top + p.clientTop;
            const right = left + p.clientWidth;
            const bottom = top + p.clientHeight;
            const cut =
              (clipX && (box.left < left - 1 || box.right > right + 1)) || (clipY && (box.top < top - 1 || box.bottom > bottom + 1));
            if (cut) {
              const name = p.id || p.getAttribute('data-scroll') || p.className || p.tagName;
              return `${el.getAttribute('data-k')} is cut by ${name}: item ${JSON.stringify(box)}, area ${JSON.stringify({ left, top, right, bottom })}`;
            }
          }
          return '';
        })) as string;
        return last === '';
      },
      { timeout: 3000, interval: 150 },
    )
    .catch(() => {
      throw new Error(`clipped: ${last}`);
    });
}

/** The titles of the notice cards on screen, newest first. */
export async function toastTitles(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-toast-title"]')).map((e) => (e.textContent ?? '').trim()),
  )) as string[];
}

export async function waitToast(text: string | RegExp): Promise<void> {
  await browser.waitUntil(
    async () => (await toastTitles()).some((t) => (typeof text === 'string' ? t === text : text.test(t))),
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `no notice "${String(text)}" (cards: ${JSON.stringify(await toastTitles())})` },
  );
}

/** TV mode's root font (1rem) in CSS pixels. */
export async function remPx(): Promise<number> {
  return (await browser.execute(() => parseFloat(getComputedStyle(document.documentElement).fontSize) || 16)) as number;
}

/**
 * No visible text in TV mode is smaller than 0.85rem (ruling 2, checklist
 * item 3): every non-blank text node's computed `font-size`, on screen or
 * scrolled out of a scrolling area, in the shell, its layers and its cards.
 * Throws with the offenders (their text, size and where they sit).
 */
export async function assertMinText(minRem = 0.85): Promise<void> {
  const bad = (await browser.execute((min: number) => {
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    const floor = min * rem - 0.05;
    const root = document.querySelector('[data-tv-root]');
    if (!root) return ['no TV root'];
    const out: string[] = [];
    const seen = new Set<Element>();
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const text = (n.nodeValue ?? '').trim();
      const el = n.parentElement;
      if (text === '' || !el || seen.has(el)) continue;
      seen.add(el);
      // Not drawn: display: none somewhere above, or hidden.
      if (el.getClientRects().length === 0) continue;
      const cs = getComputedStyle(el);
      if (cs.visibility === 'hidden') continue;
      // Screen-reader-only text is never drawn at a size.
      if (el.closest('.sr')) continue;
      const px = parseFloat(cs.fontSize);
      if (px < floor) {
        const where = el.closest('[data-testid]')?.getAttribute('data-testid') ?? el.closest('[data-k]')?.getAttribute('data-k') ?? '';
        out.push(`"${text.slice(0, 40)}" ${(px / rem).toFixed(3)}rem (${px.toFixed(2)}px) <${el.tagName.toLowerCase()} class="${el.className}"> in ${where}`);
      }
    }
    return out;
  }, minRem)) as string[];
  if (bad.length > 0) throw new Error(`text under ${minRem}rem:\n  ${bad.join('\n  ')}`);
}

/**
 * The window itself never scrolls (checklist item 4): the document is at
 * its origin and nothing overflows it.
 */
export async function assertNoDocumentScroll(): Promise<void> {
  const why = (await browser.execute(() => {
    const d = document.scrollingElement ?? document.documentElement;
    const parts: string[] = [];
    if (window.scrollX !== 0 || window.scrollY !== 0) parts.push(`window scrolled to ${window.scrollX},${window.scrollY}`);
    if (d.scrollTop !== 0 || d.scrollLeft !== 0) parts.push(`document scrolled to ${d.scrollLeft},${d.scrollTop}`);
    if (document.body.scrollTop !== 0 || document.body.scrollLeft !== 0) parts.push(`body scrolled to ${document.body.scrollLeft},${document.body.scrollTop}`);
    if (d.scrollHeight > d.clientHeight + 1 || d.scrollWidth > d.clientWidth + 1)
      parts.push(`document overflows: ${d.scrollWidth}x${d.scrollHeight} in ${d.clientWidth}x${d.clientHeight}`);
    return parts.join('; ');
  })) as string;
  if (why !== '') throw new Error(`the window scrolls: ${why}`);
}

/**
 * The three bands keep their rem heights (checklist item 5): top bar
 * 4.6rem, hint bar 3.6rem, the stage the rest of the window.
 */
export async function assertBands(): Promise<void> {
  const m = (await browser.execute(() => {
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    const h = (id: string) => document.querySelector(`[data-testid="${id}"]`)?.getBoundingClientRect().height ?? -1;
    return { rem, top: h('tv-topbar'), stage: h('tv-stage'), hints: h('tv-hints'), vh: window.innerHeight };
  })) as { rem: number; top: number; stage: number; hints: number; vh: number };
  const errs: string[] = [];
  if (Math.abs(m.top - 4.6 * m.rem) > 1) errs.push(`top bar ${m.top}px, want ${4.6 * m.rem}px`);
  if (Math.abs(m.hints - 3.6 * m.rem) > 1) errs.push(`hint bar ${m.hints}px, want ${3.6 * m.rem}px`);
  if (Math.abs(m.top + m.stage + m.hints - m.vh) > 1) errs.push(`bands ${m.top}+${m.stage}+${m.hints} ≠ window ${m.vh}px`);
  if (errs.length > 0) throw new Error(`bands (1rem = ${m.rem}px): ${errs.join('; ')}`);
}

/** The layout sweep's three window sizes (docs/tv-mode/plan.md P14). */
export const LAYOUT_SIZES: [number, number][] = [
  [1280, 720],
  [1920, 1080],
  [3840, 2160],
];

/**
 * The screen on show passes the layout checks (`assertNoDocumentScroll`,
 * `assertBands`, `assertMinText`, `assertNoClipping`) at each of `sizes`;
 * the window goes back to the size it had. For a screen the `tv-layout`
 * group can't reach (Choose your systems needs a first-run profile).
 */
export async function assertLayoutAtSizes(sizes: [number, number][] = LAYOUT_SIZES): Promise<void> {
  const was = await browser.getWindowSize();
  try {
    for (const [w, h] of sizes) {
      await browser.setWindowSize(w, h);
      await browser.waitUntil(async () => ((await browser.execute(() => window.innerHeight)) as number) === h, {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `the window never became ${w}x${h}`,
      });
      await browser.pause(350);
      for (const check of [assertNoDocumentScroll, assertBands, () => assertMinText(0.85), assertNoClipping]) {
        await check().catch((e: Error) => {
          throw new Error(`@${w}x${h}: ${e.message}`);
        });
      }
    }
  } finally {
    await browser.setWindowSize(was.width, was.height);
    await browser.pause(350);
  }
}
