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
import { entryOf, focusables, isDisabled, loadScroll, moveIn, readScroll, rememberLast, reveal, saveScroll } from './focus/dom';
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

/** An overlay (P6 fills these in). */
export type Layer = { id: number; blocking: boolean; hints: Hint[] | null };

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

function mem(key = screenKey()) {
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
  if (state.layers.length && layersRoot) {
    const last = layersRoot.lastElementChild as HTMLElement | null;
    if (last) return last;
  }
  return stage;
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
  if (cur && cur !== el) cur.classList.remove('f');
  cur = el;
  el.classList.add('f');
  rememberLast(el);
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

/** B from a grid or pane: the rail's selected row (item 95). */
export function toRail(): void {
  const rail = scope()?.querySelector('[data-rail]');
  if (!rail) return;
  const sel = rail.querySelector<HTMLElement>('.sel[data-f]') ?? focusables(rail)[0] ?? null;
  setFocus(sel);
}

function saveScreen(): void {
  if (stage) mem().scroll = saveScroll(stage);
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
  if (stage) loadScroll(stage, mem().scroll);
  instant = true;
  try {
    if (!state.layers.length) restoreFocus();
    hooks.get(screenKey())?.arrive?.();
  } finally {
    instant = false;
  }
  refreshFocus();
}

/** LB / RB at a tab's top level, a top-bar tab click, a notice's action. */
export async function showTab(tab: TvTab): Promise<void> {
  if (state.tab === tab && state.pages.length === 0) return;
  saveScreen();
  state.pages = [];
  state.tab = tab;
  await showScreen();
}

/** Pushes a page over the tab (item 319). */
export async function pushPage(page: Omit<Page, 'id'>): Promise<number> {
  saveScreen();
  const id = nextPageId++;
  state.pages = [...state.pages, { ...page, id }];
  await showScreen();
  return id;
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
      el = focusables(root).find((e) => e.getAttribute('data-k') === place.key) ?? null;
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
    layer: layer ? { blocking: layer.blocking } : null,
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
    // TODO(P6): closeLayer — overlays arrive with TvLayer.
    // TODO(P13): selToggle / selPick / selAll / selAct — select mode.
    default:
      return;
  }
}
