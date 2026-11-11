// The DOM half of the focus engine: builds `engine.ts` snapshots from the
// page, finds scrolling areas and applies `reveal.ts` targets. Everything a
// rule decides lives in the pure modules; this file only measures and
// scrolls. Focus is a class (`.f`), never `element.focus()`, so the browser
// never scrolls on its own (plan risk R7).
import type { Direction } from '../input/buttons';
import { move, type Container, type Item, type MoveResult, type Rect } from './engine';
import { readStep, revealX, revealY } from './reveal';

/** Each area's last focused item (the prototype's `_last`). Weak, so a
 *  redrawn area starts afresh, as in the prototype. */
const lastIn = new WeakMap<Element, Element>();

export function rememberLast(el: Element): void {
  const area = el.closest('[data-nav]');
  if (area) lastIn.set(area, el);
  // The area beside a rail too: Discover's shelves are each their own row,
  // so Right from the rail would otherwise find nothing remembered for the
  // shelves as a whole and start at the first one (item 85).
  const target = el.closest('[data-rail-target]');
  if (target && target !== area) lastIn.set(target, el);
}

/** Shown (not inside a `display: none` subtree). */
function shown(el: Element): boolean {
  return (el as HTMLElement).offsetParent !== null || el.getClientRects().length > 0;
}

/** The focusable items in `root`, in document order. */
export function focusables(root: Element | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>('[data-f]')).filter(shown);
}

export function isDisabled(el: Element): boolean {
  return el.hasAttribute('disabled') || el.getAttribute('aria-disabled') === 'true';
}

function rectOf(el: Element): Rect {
  const r = el.getBoundingClientRect();
  return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
}

const DIRS: Direction[] = ['up', 'down', 'left', 'right'];

function linksOf(el: Element): Partial<Record<Direction, string>> {
  const out: Partial<Record<Direction, string>> = {};
  for (const d of DIRS) {
    const v = el.getAttribute(`data-${d}`);
    if (v !== null) out[d] = v;
  }
  return out;
}

function isContainer(el: Element): boolean {
  return el.hasAttribute('data-nav') || el.hasAttribute('data-edge') || DIRS.some((d) => el.hasAttribute(`data-${d}`));
}

export type DomSnapshot = { els: HTMLElement[]; snap: { items: Item[]; containers: Container[]; current: number | null } };

/** A snapshot of `root` around `current`. */
export function snapshot(root: Element, current: Element | null): DomSnapshot {
  const els = focusables(root);
  const containerEls: Element[] = [];
  const index = new Map<Element, number>();
  const items: Item[] = els.map((el) => {
    const chain: number[] = [];
    for (let p = el.parentElement; p && p !== root.parentElement; p = p.parentElement) {
      if (!isContainer(p)) continue;
      let i = index.get(p);
      if (i === undefined) {
        i = containerEls.length;
        containerEls.push(p);
        index.set(p, i);
      }
      chain.push(i);
      if (p === root) break;
    }
    return {
      key: el.getAttribute('data-k'),
      rect: rectOf(el),
      disabled: isDisabled(el),
      autofocus: el.hasAttribute('data-autofocus'),
      links: linksOf(el),
      chain,
      railOpen: el.hasAttribute('data-rail-open'),
    };
  });
  const containers: Container[] = containerEls.map((c, ci) => {
    const members: number[] = [];
    items.forEach((it, i) => {
      if (it.chain.includes(ci)) members.push(i);
    });
    const last = lastIn.get(c);
    const lastIdx = last ? els.indexOf(last as HTMLElement) : -1;
    const nav = c.getAttribute('data-nav');
    const enter = c.getAttribute('data-enter');
    return {
      rect: rectOf(c),
      nav: nav === 'h' || nav === 'v' || nav === 'grid' ? nav : null,
      enter: enter === 'sel' || enter === 'first' ? enter : null,
      edge: c.hasAttribute('data-edge'),
      links: linksOf(c),
      last: lastIdx >= 0 && members.includes(lastIdx) ? lastIdx : null,
      // A closed folder's header stands in for the selected row it hides
      // (`data-holds-sel`), so entering the rail never selects another row.
      selected: members.find((i) => els[i].classList.contains('sel')) ?? members.find((i) => els[i].hasAttribute('data-holds-sel')) ?? null,
      items: members,
    };
  });
  const cur = current ? els.indexOf(current as HTMLElement) : -1;
  return { els, snap: { items, containers, current: cur >= 0 ? cur : null } };
}

/** Runs the engine for a move in `root`. */
export function moveIn(root: Element, current: Element | null, dir: Direction): { result: MoveResult; els: HTMLElement[] } {
  const { els, snap } = snapshot(root, current);
  return { result: move(snap, dir), els };
}

/** The live "last left there" item of an area, else its first item. */
export function entryOf(area: Element): HTMLElement | null {
  const last = lastIn.get(area);
  if (last && area.contains(last) && shown(last)) return last as HTMLElement;
  return focusables(area)[0] ?? null;
}

export function remPx(): number {
  return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}

export function reducedMotion(): boolean {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
}

/** The nearest vertical scrolling area around `el` that actually scrolls
 *  (sideways rows excluded). */
export function vScroller(el: Element): HTMLElement | null {
  for (let v = el.parentElement; v && v !== document.body; v = v.parentElement) {
    const oy = getComputedStyle(v).overflowY;
    if ((oy === 'auto' || oy === 'scroll') && v.scrollHeight > v.clientHeight + 1 && !v.hasAttribute('data-track')) return v;
  }
  return null;
}

/**
 * Scrolls everything around `el` by the rules (`reveal.ts`). `smooth`
 * glides; a first paint, a redraw and reduced motion jump.
 */
export function reveal(el: HTMLElement, smooth: boolean): void {
  const rem = remPx();
  const how: ScrollBehavior = smooth && !reducedMotion() ? 'smooth' : 'auto';
  const track = el.closest<HTMLElement>('[data-track]');
  if (track) {
    const tr = track.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    const x = revealX({
      trackLeft: tr.left,
      trackRight: tr.right,
      itemLeft: r.left,
      itemRight: r.right,
      edge: parseFloat(getComputedStyle(track).paddingLeft) || 1.2 * rem,
      scrollLeft: track.scrollLeft,
      scrollWidth: track.scrollWidth,
      clientWidth: track.clientWidth,
    });
    if (x !== null) track.scrollTo({ left: x, behavior: how });
  }
  const v = vScroller(el);
  if (!v) return;
  const vr = v.getBoundingClientRect();
  const b = el.getBoundingClientRect();
  const shelf = v.hasAttribute('data-shelves') ? el.closest<HTMLElement>('[data-shelf]') : null;
  const all = focusables(v);
  // A windowed grid's first and last covers are by index: the rendered
  // ones at either end of the window are not the list's (risk R8).
  const cell = el.closest<HTMLElement>('[data-i]');
  const wgrid = cell?.closest<HTMLElement>('[data-window]') ?? null;
  const wi = wgrid && v.contains(wgrid) ? Number(cell!.getAttribute('data-i')) : null;
  const wn = wgrid ? Number(wgrid.getAttribute('data-count')) : 0;
  const y = revealY({
    areaTop: vr.top,
    areaBottom: vr.bottom,
    itemTop: b.top,
    itemBottom: b.bottom,
    rem,
    scrollTop: v.scrollTop,
    scrollHeight: v.scrollHeight,
    clientHeight: v.clientHeight,
    first: wi !== null ? wi === 0 && el === all[0] : el === all[0],
    last: wi !== null ? wi === wn - 1 && el === all[all.length - 1] : el === all[all.length - 1],
    ...(shelf ? { shelfTop: shelf.previousElementSibling ? shelf.getBoundingClientRect().top : null } : {}),
  });
  if (y !== null) v.scrollTo({ top: y, behavior: how });
}

/** Up / Down on a reading item: scrolls its area and answers true, or
 *  answers false when the move should happen instead. */
export function readScroll(el: HTMLElement, dir: 'up' | 'down', smooth: boolean): boolean {
  const area = el.closest<HTMLElement>('[data-read-area]') ?? vScroller(el);
  if (!area) return false;
  const pr = el.getBoundingClientRect();
  const vr = area.getBoundingClientRect();
  const step = readStep(dir, pr.top, pr.bottom, vr.top, vr.bottom, remPx());
  if (step === null) return false;
  area.scrollBy({ top: step, behavior: smooth && !reducedMotion() ? 'smooth' : 'auto' });
  return true;
}

/** Every scrolling area of a screen, keyed by `data-scroll` (or its order). */
export function saveScroll(root: Element): Record<string, [number, number]> {
  const out: Record<string, [number, number]> = {};
  root.querySelectorAll<HTMLElement>('[data-scroll]').forEach((e, i) => {
    out[e.getAttribute('data-scroll') || `#${i}`] = [e.scrollLeft, e.scrollTop];
  });
  return out;
}

export function loadScroll(root: Element, saved: Record<string, [number, number]> | undefined): void {
  if (!saved) return;
  root.querySelectorAll<HTMLElement>('[data-scroll]').forEach((e, i) => {
    const v = saved[e.getAttribute('data-scroll') || `#${i}`];
    if (!v) return;
    e.scrollLeft = v[0];
    e.scrollTop = v[1];
  });
}
