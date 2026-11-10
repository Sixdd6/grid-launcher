// TV mode's spatial focus engine (checklist items 77–91; tvmode.js `move`,
// `restoreFocus`), rewritten over plain rects so every rule is unit-tested.
// `dom.ts` builds a snapshot from the page and applies the answer.
//
// The markup carries the model: `data-f` focusable, `data-k` its stable
// identity, `data-nav="h|v|grid"` an area that keeps focus inside it along
// its axis, `data-enter="sel|first"` how an area is entered, `data-edge` an
// area of rows that is left only straight past its top or bottom edge, and
// `data-up` / `data-down` / `data-left` / `data-right` an explicit link.
import type { Direction } from '../input/buttons';

export type Rect = { left: number; top: number; right: number; bottom: number };

export type Container = {
  rect: Rect;
  /** `data-nav`. */
  nav: 'h' | 'v' | 'grid' | null;
  /** `data-enter`. */
  enter: 'sel' | 'first' | null;
  /** `data-edge`. */
  edge: boolean;
  /** `data-up` etc.: the `data-k` a move that way goes to. */
  links: Partial<Record<Direction, string>>;
  /** The item last focused inside it, while it is still there and shown. */
  last: number | null;
  /** Its selected item (a rail's `.sel` row). */
  selected: number | null;
  /** Its items, in document order. */
  items: number[];
};

export type Item = {
  key: string | null;
  rect: Rect;
  disabled: boolean;
  autofocus: boolean;
  /** Its own `data-up` etc. */
  links: Partial<Record<Direction, string>>;
  /** The containers around it, innermost first (indexes into `containers`). */
  chain: number[];
  /** A rail row: Right does what A does (opens the pane beside it). */
  railOpen: boolean;
};

export type Snapshot = {
  /** Every shown focusable in the scope, in document order. */
  items: Item[];
  containers: Container[];
  current: number | null;
};

export type MoveResult =
  | { kind: 'focus'; item: number }
  | { kind: 'railOpen' }
  | { kind: 'restore' }
  | { kind: 'none' };

/** Score weight of the offset across the direction against the gap along it. */
export const CROSS_WEIGHT = 2.2;

const cx = (r: Rect) => (r.left + r.right) / 2;
const cy = (r: Rect) => (r.top + r.bottom) / 2;

function navOf(snap: Snapshot, item: Item): number | null {
  for (const c of item.chain) if (snap.containers[c].nav !== null) return c;
  return null;
}

function inside(item: Item, container: number): boolean {
  return item.chain.includes(container);
}

/**
 * The best item lying in `dir` from `from`: only items whose centre is past
 * the current centre and whose near edge is past the current near edge
 * qualify; the lowest gap along + 2.2 × offset across wins (item 81).
 */
function pick(snap: Snapshot, from: number, dir: Direction, candidates: number[]): number | null {
  const a = snap.items[from].rect;
  const ax = cx(a);
  const ay = cy(a);
  let best: number | null = null;
  let bs = Infinity;
  for (const i of candidates) {
    if (i === from) continue;
    const b = snap.items[i].rect;
    const bx = cx(b);
    const by = cy(b);
    let p: number;
    let s: number;
    if (dir === 'right') {
      if (bx <= ax + 2 || b.left < a.left + 2) continue;
      p = Math.max(0, b.left - a.right);
      s = Math.abs(by - ay);
    } else if (dir === 'left') {
      if (bx >= ax - 2 || b.right > a.right - 2) continue;
      p = Math.max(0, a.left - b.right);
      s = Math.abs(by - ay);
    } else if (dir === 'down') {
      if (by <= ay + 2 || b.top < a.top + 2) continue;
      p = Math.max(0, b.top - a.bottom);
      s = Math.abs(bx - ax);
    } else {
      if (by >= ay - 2 || b.bottom > a.bottom - 2) continue;
      p = Math.max(0, a.top - b.bottom);
      s = Math.abs(bx - ax);
    }
    const sc = p + s * CROSS_WEIGHT;
    if (sc < bs) {
      bs = sc;
      best = i;
    }
  }
  return best;
}

function all(snap: Snapshot): number[] {
  return snap.items.map((_, i) => i);
}

export function move(snap: Snapshot, dir: Direction): MoveResult {
  const curIdx = snap.current;
  if (curIdx === null || !snap.items[curIdx]) return { kind: 'restore' };
  const cur = snap.items[curIdx];

  // Right from a rail is A: into the grid or pane beside it (item 85).
  if (dir === 'right' && cur.railOpen) return { kind: 'railOpen' };

  // An explicit link wins over geometry (item 91): the item's own, else the
  // nearest container's.
  const linkKey = cur.links[dir] ?? cur.chain.map((c) => snap.containers[c].links[dir]).find((k) => k !== undefined);
  if (linkKey !== undefined) {
    const to = snap.items.findIndex((it) => it.key === linkKey);
    if (to >= 0 && to !== curIdx) return { kind: 'focus', item: to };
  }

  const contIdx = navOf(snap, cur);
  const cont = contIdx === null ? null : snap.containers[contIdx];
  const axis = cont?.nav ?? null;
  const along = (axis === 'h' && (dir === 'left' || dir === 'right')) || (axis === 'v' && (dir === 'up' || dir === 'down'));

  let t: number | null = null;
  if (along && cont && contIdx !== null) {
    // Inside the list along its axis; never wraps off the end (item 82).
    t = pick(snap, curIdx, dir, cont.items);
    if (t === null) {
      // At the end, carry on to what sits past it in the same column (or
      // line), never sideways into a neighbouring panel (items 83–84).
      const cr = cont.rect;
      const fwd = dir === 'down' || dir === 'right';
      const out = all(snap).filter((i) => {
        const it = snap.items[i];
        if (inside(it, contIdx)) return false;
        // A rail row counts as spanning its whole rail across the list's
        // axis: Left from any Discover shelf reaches the rail, even when the
        // rail's rows end above that shelf (README § Discover).
        const own = navOf(snap, it);
        const span = own !== null && snap.containers[own].enter === 'sel' ? snap.containers[own].rect : it.rect;
        const b = { left: it.rect.left, right: it.rect.right, top: it.rect.top, bottom: it.rect.bottom };
        if (axis === 'h') {
          b.top = Math.min(b.top, span.top);
          b.bottom = Math.max(b.bottom, span.bottom);
        } else {
          b.left = Math.min(b.left, span.left);
          b.right = Math.max(b.right, span.right);
        }
        if (axis === 'v') return b.right > cr.left && b.left < cr.right && (fwd ? b.top >= cr.bottom - 2 : b.bottom <= cr.top + 2);
        return b.bottom > cr.top && b.top < cr.bottom && (fwd ? b.left >= cr.right - 2 : b.right <= cr.left + 2);
      });
      // The nearest line past the list, then its first item in reading order.
      out.sort((xi, yi) => {
        const X = snap.items[xi].rect;
        const Y = snap.items[yi].rect;
        const d = axis === 'v' ? (fwd ? X.top - Y.top : Y.bottom - X.bottom) : fwd ? X.left - Y.left : Y.right - X.right;
        return Math.abs(d) > 8 ? d : axis === 'v' ? X.left - Y.left : X.top - Y.top;
      });
      t = out.length ? out[0] : null;
      // Into a rail: its selected row. Back into a group: the item left there.
      const tgIdx = t === null ? null : navOf(snap, snap.items[t]);
      const tg = tgIdx === null ? null : snap.containers[tgIdx];
      if (tg && tg.enter === 'sel') t = tg.selected ?? t;
      else if (tg && tg.last !== null) t = tg.last;
    }
    if (t === null) return { kind: 'none' };
  } else {
    // An area of rows (Discover's shelves) is left by its edges like a grid:
    // Up from the top row or Down from the bottom row only reaches what sits
    // straight past that edge, never the rail (item 174).
    const areaIdx = cur.chain.find((c) => snap.containers[c].edge);
    if (areaIdx !== undefined && (dir === 'up' || dir === 'down')) {
      const area = snap.containers[areaIdx];
      t = pick(snap, curIdx, dir, area.items);
      if (t === null) {
        const ar = area.rect;
        t = pick(
          snap,
          curIdx,
          dir,
          all(snap).filter((i) => {
            const it = snap.items[i];
            return !inside(it, areaIdx) && it.rect.right > ar.left && it.rect.left < ar.right;
          }),
        );
        if (t === null) return { kind: 'none' };
      }
    } else if (cont && contIdx !== null && axis === 'grid') {
      t = pick(snap, curIdx, dir, cont.items);
      // Off a grid's edge, only to what sits straight past that edge, never
      // diagonally; nothing there: stay put (items 88–90).
      if (t === null) {
        const gr = cont.rect;
        const vert = dir === 'up' || dir === 'down';
        t = pick(
          snap,
          curIdx,
          dir,
          all(snap).filter((i) => {
            const it = snap.items[i];
            if (inside(it, contIdx)) return false;
            const b = it.rect;
            return vert ? b.right > gr.left && b.left < gr.right : b.bottom > gr.top && b.top < gr.bottom;
          }),
        );
        if (t === null) return { kind: 'none' };
      }
    }
    if (t === null) t = pick(snap, curIdx, dir, all(snap));
    if (t !== null) {
      const tcIdx = navOf(snap, snap.items[t]);
      const tc = tcIdx === null ? null : snap.containers[tcIdx];
      if (tc && tcIdx !== contIdx) {
        const live = tc.last;
        // Into a rail: always its selected row (item 86).
        if (tc.enter === 'sel') t = tc.selected ?? live ?? t;
        // Into an area entered at its start: where you left it, else its
        // first item (item 87).
        else if (tc.enter === 'first') t = live ?? tc.items[0] ?? t;
        else if (live !== null && tc.nav !== 'grid') t = live;
      }
    }
  }
  return t === null ? { kind: 'none' } : { kind: 'focus', item: t };
}

/**
 * Which item a screen opens on (item 78): the one last focused there (by
 * its key), else the screen's designated default (`data-autofocus`: the
 * checked option, Cancel), else the first; a disabled one gives way to the
 * first enabled item when there is one.
 */
export function restore(items: Pick<Item, 'key' | 'disabled' | 'autofocus'>[], memoryKey: string | null): number | null {
  if (items.length === 0) return null;
  let i = memoryKey !== null ? items.findIndex((it) => it.key === memoryKey) : -1;
  if (i < 0) i = items.findIndex((it) => it.autofocus);
  if (i < 0) i = 0;
  if (items[i].disabled) {
    const enabled = items.findIndex((it) => !it.disabled);
    if (enabled >= 0) i = enabled;
  }
  return i;
}

/**
 * After a change of data removed the focused item (item 115): the enabled
 * item now at the same position in the same panel, clamped to its end.
 */
export function inPlace(enabled: number[], index: number): number | null {
  if (enabled.length === 0) return null;
  return enabled[Math.max(0, Math.min(index, enabled.length - 1))];
}

/**
 * How focus returns when an overlay closes (item 117): the opener itself
 * keeps its scroll; an item refound by key or in place after an in-place
 * redraw under the overlay is jumped to.
 */
export function closeRefocus<T>(el: T, opener: T | null): { scroll: false } | { instant: true } {
  return el === opener ? { scroll: false } : { instant: true };
}
