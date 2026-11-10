// TV mode's navigation and focus: the tab, the page stack, the overlay stack
// (P6), each screen's focus memory and scroll positions, and the one focused
// item (TV mode plan § 1.2; tvmode.js `setFocus`, `restoreFocus`, `render`,
// `repaint`, `press`). Module scoped: one TV shell at a time.
//
// A "screen" is the tab on show, or the top page. Screens register hooks
// (`registerScreen`) for their own hints, X / Y / LT / RT / LB / RB and the
// page's main button (Start). Items register what A, X and Left / Right do
// through `use:tvf` (focus/actions.ts).
import { flushSync, tick, type Component } from 'svelte';
import { pushToast } from '../stores/toasts.svelte';
import type { TvPane } from '../uiMode';
import type { Hint } from './hints';
import { handlersOf } from './focus/actions';
import { restore, inPlace } from './focus/engine';
import { entryOf, focusables, isDisabled, loadScroll, moveIn, readScroll, reducedMotion, rememberLast, reveal, saveScroll } from './focus/dom';
import { gridMove } from './focus/grid';
import { materialize, windowOf, windowsIn } from './focus/windows';
import type { Direction, TvButton } from './input/buttons';
import { press, stepTab, type Command, type PressContext, type TvTab } from './input/press';

export type ScreenHooks = {
  /** The screen's own hints, after A and X (Y Search, LT Sort, …). */
  hints?: (focused: HTMLElement | null) => Hint[];
  x?: () => void;
  y?: () => void;
  lt?: () => void;
  rt?: () => void;
  lb?: () => void;
  rb?: () => void;
  /**
   * The page's main button, which Start presses from anywhere on the page
   * (items 100–101): null when the page has none right now. While it is
   * disabled, Start says `why` as an error notice instead.
   */
  main?: () => { disabled: boolean; why?: string; press: () => void } | null;
  /** After the screen is shown and focus restored: a screen may move focus
   *  (P7: arriving on Server › Discover focuses its content, item 104). */
  arrive?: () => void;
};

export type Page = {
  /** Unique for the session. */
  id: number;
  /** The breadcrumb's last part (item 122). */
  title: string;
  component: Component<any>;
  props?: Record<string, unknown>;
};

/**
 * An overlay (P6; tvmode.js `openLayer`): a list, a confirmation, the
 * on-screen keyboard, the Menu, or (P9) the media viewer. Focus stays inside
 * the top one until it closes (item 361); closing it puts focus back on the
 * item it was opened from, found again by its key after a redraw.
 */
export type Layer = {
  id: number;
  /** B and a click outside don't close it (the unreachable library). */
  blocking: boolean;
  /** Its own hints, or null for A "Select", B "Back". */
  hints: Hint[] | null;
  /** Buttons it binds itself (item 103: the keyboard, the media viewer);
   *  every other button but A, B and the directions does nothing. */
  binds: Partial<Record<TvButton, () => void>>;
  /** A physical key while it is on top; answers true when it took it
   *  (the keyboard's typing, item 74). */
  keydown?: (e: KeyboardEvent) => boolean;
  /** Held typing buttons repeat (the keyboard, items 42–43). */
  typingRepeat?: (b: TvButton, focused: HTMLElement | null) => boolean;
  /** Extra class on the scrim: `side` (the Menu), `kb` (the keyboard). */
  cls: string;
  component: Component<any>;
  props: Record<string, unknown>;
  onClose?: () => void;
};

export type LayerSpec = Omit<Layer, 'id' | 'blocking' | 'hints' | 'binds' | 'cls' | 'props'> &
  Partial<Pick<Layer, 'blocking' | 'hints' | 'binds' | 'cls' | 'props'>>;

type FocusInfo = { key: string | null; a: string; x: string | null; disabled: boolean; present: boolean };

const state = $state<{
  tab: TvTab;
  /** Settings' pane (kept here so leaving TV lands on it, item 523). */
  pane: TvPane;
  pages: Page[];
  layers: Layer[];
  startup: boolean;
  selMode: boolean;
  focus: FocusInfo;
  /** Bumped when a screen registers or drops hooks. */
  hooksVersion: number;
}>({
  tab: 'server',
  pane: 'appearance',
  pages: [],
  layers: [],
  startup: false,
  selMode: false,
  focus: { key: null, a: 'Select', x: null, disabled: false, present: false },
  hooksVersion: 0,
});

let stage: HTMLElement | null = null;
let layersRoot: HTMLElement | null = null;
let cur: HTMLElement | null = null;
/** Where the focused item sat, for a redraw that removes it (item 115). */
let curPlace: { key: string | null; boxId: string | null; index: number } | null = null;
/** Set while a screen first appears or redraws: scrolls jump (item 114). */
let instant = false;
let nextPageId = 1;
let nextLayerId = 1;
/** Where each open layer was opened from (tvmode.js `o.prev`). */
const layerPrev = new Map<number, { el: HTMLElement | null; key: string | null; boxId: string | null; index: number }>();
/** Set while an overlay closes: the stage's observer leaves focus to
 *  `closeLayer`, which puts it back on the opener (else, with nothing
 *  focused for a moment, it would restore the screen's focus, and focusing
 *  a rail row selects it). */
let closing = false;
/** Work that waits for the last overlay to close (item 117). */
let afterLayers: (() => void)[] = [];

const memory = new Map<string, { k: string | null; scroll?: Record<string, [number, number]> }>();
const hooks = new Map<string, ScreenHooks>();

/** What the Menu and View buttons do; P6 registers both. */
let menuHandler: (() => void) | null = null;
let viewHandler: (() => void) | null = null;

export const nav = {
  get tab(): TvTab {
    return state.tab;
  },
  get pane(): TvPane {
    return state.pane;
  },
  get pages(): readonly Page[] {
    return state.pages;
  },
  get topPage(): Page | null {
    return state.pages[state.pages.length - 1] ?? null;
  },
  get layers(): readonly Layer[] {
    return state.layers;
  },
  get startup(): boolean {
    return state.startup;
  },
  get selMode(): boolean {
    return state.selMode;
  },
  get focus(): FocusInfo {
    return state.focus;
  },
  /** The current screen's key: `tab:<tab>` or `page:<id>`. */
  get screen(): string {
    return screenKey();
  },
  /** The current screen's hooks (reactive on registration). */
  get hooks(): ScreenHooks {
    void state.hooksVersion;
    return hooks.get(screenKey()) ?? {};
  },
};

function screenKey(): string {
  const p = state.pages[state.pages.length - 1];
  return p ? `page:${p.id}` : `tab:${state.tab}`;
}

/** Focus memory's key: the top overlay's while one is open, else the screen's. */
function memKey(): string {
  const l = state.layers[state.layers.length - 1];
  return l ? `layer:${l.id}` : screenKey();
}

function mem(key = memKey()) {
  let m = memory.get(key);
  if (!m) {
    m = { k: null };
    memory.set(key, m);
  }
  return m;
}

/** Fresh state for a TV shell that is mounting. */
export function resetNav(tab: TvTab, pane: TvPane | null): void {
  state.tab = tab;
  if (pane !== null) state.pane = pane;
  state.pages = [];
  state.layers = [];
  layerPrev.clear();
  afterLayers = [];
  state.selMode = false;
  memory.clear();
  cur = null;
  curPlace = null;
  refreshFocus();
}

export function attachRoots(stageEl: HTMLElement | null, layersEl: HTMLElement | null): void {
  stage = stageEl;
  layersRoot = layersEl;
}

/** Where focus lives now: the top overlay, else the stage. */
export function scope(): HTMLElement | null {
  const top = state.layers[state.layers.length - 1];
  if (top && layersRoot) {
    const el = layersRoot.querySelector<HTMLElement>(`[data-layer="${top.id}"]`);
    if (el) return el;
  }
  return stage;
}

/** The top overlay, or null. */
export function topLayer(): Layer | null {
  return state.layers[state.layers.length - 1] ?? null;
}

/**
 * Opens an overlay over the stage (tvmode.js `openLayer`): focus moves into
 * it, to its `data-autofocus` item (Cancel, the checked option) or its first.
 * Answers its id.
 */
export async function openLayer(spec: LayerSpec): Promise<number> {
  const id = nextLayerId++;
  layerPrev.set(id, { el: cur, key: cur?.getAttribute('data-k') ?? null, boxId: curPlace?.boxId ?? null, index: curPlace?.index ?? -1 });
  state.layers = [
    ...state.layers,
    { blocking: false, hints: null, binds: {}, cls: '', props: {}, ...spec, id },
  ];
  await tick();
  if (topLayer()?.id === id) restoreFocus();
  return id;
}

/**
 * Closes the top overlay (tvmode.js `closeLayer`): focus goes back to the
 * item it was opened from, or the item with its key after a redraw, else the
 * screen's focus; then its `onClose` runs, then anything that waited for the
 * overlays to close (item 117).
 */
export async function closeLayer(): Promise<void> {
  const layer = state.layers[state.layers.length - 1];
  if (!layer) return;
  const prev = layerPrev.get(layer.id) ?? { el: null, key: null, boxId: null, index: -1 };
  layerPrev.delete(layer.id);
  memory.delete(`layer:${layer.id}`);
  closing = true;
  state.layers = state.layers.slice(0, -1);
  if (cur) cur.classList.remove('f');
  cur = null;
  try {
    await tick();
  } finally {
    closing = false;
  }
  const root = scope();
  let el: HTMLElement | null = null;
  if (prev.el && prev.el.isConnected && root?.contains(prev.el)) el = prev.el;
  else if (prev.key && root) el = byKey(root, prev.key);
  // The opener went while the overlay was up (a live search narrowed the
  // grid): the item now in its place in the same panel (item 115), else,
  // with the panel empty, the rail's selected row (as B would go), so
  // restoring focus never lands on another rail row and selects it.
  const box = !el && prev.boxId ? document.getElementById(prev.boxId) : null;
  if (box && root?.contains(box)) {
    const enabled = focusables(box).filter((e) => !isDisabled(e));
    const i = inPlace(enabled.map((_, n) => n), prev.index);
    const rail = root.querySelector('[data-rail]');
    el = i === null ? (rail ? railSelected(rail) : null) : enabled[i];
  }
  if (el) setFocus(el, { scroll: false });
  else restoreFocus();
  layer.onClose?.();
  if (!state.layers.length && afterLayers.length) {
    const run = afterLayers;
    afterLayers = [];
    for (const fn of run) fn();
  }
}

/** Runs `fn` now, or once the last overlay closes (item 117: a redraw a
 *  finished install causes waits for the overlay). */
export function afterOverlays(fn: () => void): void {
  if (state.layers.length) afterLayers.push(fn);
  else fn();
}

/** The screen registers its hooks; answers the function that drops them. */
export function registerScreen(key: string, h: ScreenHooks): () => void {
  hooks.set(key, h);
  state.hooksVersion++;
  return () => {
    if (hooks.get(key) === h) {
      hooks.delete(key);
      state.hooksVersion++;
    }
  };
}

export function registerMenu(fn: (() => void) | null): void {
  menuHandler = fn;
}

export function registerView(fn: (() => void) | null): void {
  viewHandler = fn;
}

/** A startup screen (Connect, Choose your systems, the unreachable
 *  library) is showing: no tabs, no Menu, no Back (items 97, 123, 132–133). */
export function setStartup(on: boolean): void {
  state.startup = on;
}

export function setPane(pane: TvPane): void {
  state.pane = pane;
}

export function focused(): HTMLElement | null {
  return cur;
}

/** The item with `data-k` `k` in `root`, rendering it first when it is a
 *  cover a windowed grid has left out. */
function byKey(root: Element, k: string): HTMLElement | null {
  return focusables(root).find((e) => e.getAttribute('data-k') === k) ?? materialize(root, k);
}

/** Re-reads the focused item's labels for the hint bar. */
export function refreshFocus(): void {
  const el = cur;
  if (!el || !el.isConnected) {
    state.focus = { key: null, a: 'Select', x: null, disabled: false, present: false };
    return;
  }
  // A class write elsewhere (a redraw) must not drop the ring.
  if (!el.classList.contains('f')) el.classList.add('f');
  const next: FocusInfo = {
    key: el.getAttribute('data-k'),
    a: el.getAttribute('data-al') || 'Select',
    x: el.getAttribute('data-xl'),
    disabled: isDisabled(el),
    present: true,
  };
  const f = state.focus;
  if (f.key !== next.key || f.a !== next.a || f.x !== next.x || f.disabled !== next.disabled || f.present !== next.present) state.focus = next;
}

/**
 * Focuses `el`: the one ring moves, the area remembers it, the screen
 * remembers its key, and the scrolling rules run unless `scroll` is false
 * (mouse pointing, closing an overlay).
 */
export function setFocus(el: HTMLElement | null, opts: { scroll?: boolean; instant?: boolean } = {}): void {
  if (!el) return;
  const prev = cur;
  if (cur && cur !== el) cur.classList.remove('f');
  cur = el;
  // Up from a Details tab into the buttons: the tab scrolls back to its
  // top (item 299; tvmode.js `setFocus`). `data-read-reset` marks the row.
  if (prev && prev !== el && el.closest('[data-read-reset]')) {
    const area = prev.closest<HTMLElement>('[data-read-area]');
    if (area && area.isConnected) area.scrollTo({ top: 0, behavior: opts.instant || instant || reducedMotion() ? 'auto' : 'smooth' });
  }
  el.classList.add('f');
  rememberLast(el);
  // A windowed grid keeps the focused cover's row rendered (risk R8).
  const w = windowOf(el);
  if (w) w.handle.focusAt(w.index);
  if (opts.scroll !== false) reveal(el, !(opts.instant || instant));
  const k = el.getAttribute('data-k');
  if (k) mem().k = k;
  const box = el.closest('[id]');
  curPlace = { key: k, boxId: box?.id ?? null, index: box ? focusables(box).indexOf(el) : -1 };
  handlersOf(el)?.focus?.();
  refreshFocus();
}

/** Focus where the screen left it, else its default, else its first item
 *  (item 78), without gliding. */
export function restoreFocus(): void {
  const root = scope();
  // The cover focus was on may be one a windowed grid has not rendered.
  const k = mem().k;
  if (k && root && !focusables(root).some((e) => e.getAttribute('data-k') === k)) materialize(root, k);
  const els = focusables(root);
  const i = restore(
    els.map((e) => ({ key: e.getAttribute('data-k'), disabled: isDisabled(e), autofocus: e.hasAttribute('data-autofocus') })),
    mem().k,
  );
  if (i === null) {
    if (cur) cur.classList.remove('f');
    cur = null;
    refreshFocus();
    return;
  }
  setFocus(els[i], { instant: true });
}

export function moveFocus(dir: Direction): void {
  const root = scope();
  if (!root) return;
  if (!cur || !root.contains(cur) || !focusables(root).includes(cur)) {
    restoreFocus();
    return;
  }
  // Inside a windowed grid, moves are by index, so covers that aren't
  // rendered are reached too; only leaving it by an edge is spatial (R8).
  const w = windowOf(cur);
  if (w) {
    const to = gridMove(w.index, dir, w.handle.columns(), w.handle.count());
    if (to !== 'edge') {
      const el = w.handle.ensure(to);
      if (el) setFocus(el);
      return;
    }
  }
  const { result, els } = moveIn(root, cur, dir);
  if (result.kind === 'focus') setFocus(els[result.item]);
  else if (result.kind === 'railOpen') railOpen();
  else if (result.kind === 'restore') restoreFocus();
}

/** A or Right on a rail row: into the area beside the rail
 *  (`data-rail-target`), where it was left, else its first item (item 85). */
export function railOpen(): void {
  // Focusing the row may have just switched the pane (a click focuses and
  // presses A at once): draw it before looking inside.
  flushSync();
  const box = scope()?.querySelector('[data-rail-target]');
  if (!box) return;
  const el = entryOf(box);
  if (el) setFocus(el);
}

/** A rail's selected row, or the closed folder header that hides it. */
function railSelected(rail: Element): HTMLElement | null {
  return rail.querySelector<HTMLElement>('.sel[data-f]') ?? rail.querySelector<HTMLElement>('[data-holds-sel][data-f]');
}

/** B from a grid or pane: the rail's selected row (item 95). */
export function toRail(): void {
  const rail = scope()?.querySelector('[data-rail]');
  if (!rail) return;
  const sel = railSelected(rail) ?? focusables(rail)[0] ?? null;
  setFocus(sel);
}

function saveScreen(): void {
  if (stage) mem(screenKey()).scroll = saveScroll(stage);
}

/** After the screen changed: its scroll positions come back, then focus,
 *  with no glide (items 79, 113–114). */
export async function showScreen(): Promise<void> {
  if (cur) cur.classList.remove('f');
  cur = null;
  // Another screen can reuse a key (Library's and Settings' "r-library"
  // rows): only a redraw of the same screen re-finds focus by key.
  curPlace = null;
  await tick();
  // Windowed grids measure first, so a saved scroll deep in a long list
  // isn't cut short, then render what the restored scroll shows.
  for (const w of windowsIn(stage)) w.sync();
  if (stage) loadScroll(stage, mem(screenKey()).scroll);
  for (const w of windowsIn(stage)) w.sync();
  instant = true;
  try {
    if (!state.layers.length) restoreFocus();
    hooks.get(screenKey())?.arrive?.();
  } finally {
    instant = false;
  }
  refreshFocus();
}

/**
 * Sets the item a screen focuses when it next appears (tvmode.js
 * `TABS.server._k = 'r-discover'`: Menu › Discover lands on the Discover
 * rail row, which Server's `arrive` then carries into Discover's content).
 */
export function rememberFocus(screen: string, k: string): void {
  mem(screen).k = k;
}

/** LB / RB at a tab's top level, a top-bar tab click, a notice's action.
 *  With `again`, a tab already on show appears afresh (its focus restored
 *  and `arrive` run), as tvmode.js `render` does. */
export async function showTab(tab: TvTab, opts: { again?: boolean } = {}): Promise<void> {
  if (state.tab === tab && state.pages.length === 0 && !opts.again) return;
  saveScreen();
  state.pages = [];
  state.tab = tab;
  await showScreen();
}

/** Pushes a page over the tab (item 319). With `fresh`, any pages close
 *  first (Menu › Notifications, item 356). */
export async function pushPage(page: Omit<Page, 'id'>, opts: { fresh?: boolean } = {}): Promise<number> {
  saveScreen();
  const id = nextPageId++;
  if (opts.fresh) for (const p of state.pages) memory.delete(`page:${p.id}`);
  state.pages = [...(opts.fresh ? [] : state.pages), { ...page, id }];
  await showScreen();
  return id;
}

/** Renames a page in the breadcrumb (Search everything's results follow
 *  the search, ruling 17). */
export function setPageTitle(id: number, title: string): void {
  state.pages = state.pages.map((p) => (p.id === id ? { ...p, title } : p));
}

/** Replaces a page's props (Details after Link to server game follows the
 *  row's new server game). The page redraws when it is next shown. */
export function setPageProps(id: number, props: Record<string, unknown>): void {
  state.pages = state.pages.map((p) => (p.id === id ? { ...p, props: { ...p.props, ...props } } : p));
}

/** B on a page: back one page, focus and scroll as they were (items 93, 319). */
export async function popPage(): Promise<void> {
  const top = state.pages[state.pages.length - 1];
  if (!top) return;
  memory.delete(`page:${top.id}`);
  state.pages = state.pages.slice(0, -1);
  await showScreen();
}

/**
 * The stage changed. A redraw keeps focus on the same item (by its key); if
 * that item is gone, on the enabled item now in its place in the same panel;
 * else the screen's default (item 115). With nothing focused yet (content
 * that arrived late), the screen's focus is restored.
 */
export function stageChanged(): void {
  if (closing) return;
  const root = scope();
  if (!root) return;
  if (cur && cur.isConnected && root.contains(cur)) {
    refreshFocus();
    return;
  }
  instant = true;
  if (!cur) {
    // Content that arrived after the screen was shown.
    try {
      if (focusables(root).length) restoreFocus();
      else refreshFocus();
    } finally {
      instant = false;
    }
    return;
  }
  try {
    const place = curPlace;
    let el: HTMLElement | null = null;
    if (place?.key) {
      el = byKey(root, place.key);
    }
    if (!el && place?.boxId) {
      const box = document.getElementById(place.boxId);
      if (box && root.contains(box)) {
        const enabled = focusables(box).filter((e) => !isDisabled(e));
        const i = inPlace(enabled.map((_, n) => n), place.index);
        el = i === null ? null : enabled[i];
      }
    }
    if (el) setFocus(el, { instant: true });
    else restoreFocus();
  } finally {
    instant = false;
  }
}

function context(): PressContext {
  const root = scope();
  const h = nav.hooks;
  const m = h.main?.() ?? null;
  const layer = state.layers[state.layers.length - 1] ?? null;
  return {
    layer: layer ? { blocking: layer.blocking, binds: Object.keys(layer.binds) as TvButton[] } : null,
    pages: state.pages.length,
    startup: state.startup,
    selMode: state.selMode,
    focus: cur
      ? {
          adjust: cur.hasAttribute('data-adj'),
          read: cur.hasAttribute('data-read'),
          disabled: isDisabled(cur),
          inRail: cur.closest('[data-rail]') !== null,
          cover: cur.hasAttribute('data-cover'),
        }
      : null,
    hasRail: !!root?.querySelector('[data-rail]'),
    screen: {
      x: !!h.x,
      y: !!h.y,
      lt: !!h.lt,
      rt: !!h.rt,
      lb: !!h.lb,
      rb: !!h.rb,
      main: m === null ? null : m.disabled ? (m.why ? 'why' : null) : 'press',
    },
  };
}

/** One press of a logical button, from any input. */
export function handlePress(b: TvButton): void {
  run(press(b, context()));
}

function run(cmd: Command): void {
  const h = nav.hooks;
  switch (cmd.kind) {
    case 'move':
      moveFocus(cmd.dir);
      return;
    case 'adjust':
      handlersOf(cur)?.adjust?.(cmd.delta);
      return;
    case 'read':
      if (cur && readScroll(cur, cmd.dir, true)) return;
      moveFocus(cmd.dir);
      return;
    case 'activate':
      handlersOf(cur)?.a?.();
      return;
    case 'why': {
      const why = cur?.getAttribute('data-why');
      if (why) pushToast(why, 'error');
      return;
    }
    case 'pop':
      void popPage();
      return;
    case 'toRail':
      toRail();
      return;
    case 'x': {
      const x = handlersOf(cur)?.x;
      if (x) x();
      else h.x?.();
      return;
    }
    case 'screen':
      h[cmd.handler]?.();
      return;
    case 'tab':
      void showTab(stepTab(state.tab, cmd.step));
      return;
    case 'main':
      h.main?.()?.press();
      return;
    case 'mainWhy': {
      const why = h.main?.()?.why;
      if (why) pushToast(why, 'error');
      return;
    }
    case 'menu':
      menuHandler?.();
      return;
    case 'view':
      viewHandler?.();
      return;
    case 'closeLayer':
      void closeLayer();
      return;
    case 'layerButton':
      topLayer()?.binds[cmd.button]?.();
      return;
    // TODO(P13): selToggle / selPick / selAll / selAct — select mode.
    default:
      return;
  }
}
