// What a press does, given what is on screen (checklist items 92–103;
// tvmode.js `press`). Pure: the shell builds a `PressContext`, runs this,
// and carries out the command it answers.
import { directionOf, isDirection, type Direction, type TvButton } from './buttons';

export type PressContext = {
  /** The top overlay, if one is open (P6 adds the keyboard and the media
   *  viewer, which bind more buttons). */
  layer: { blocking: boolean } | null;
  /** Pages pushed over the tab (or over the startup screen). */
  pages: number;
  /** A startup screen (Connect, Choose your systems) is showing. */
  startup: boolean;
  /** Select mode on a Library or Server grid (P13). */
  selMode: boolean;
  /** The focused item, or null. */
  focus: {
    /** A slider: Left / Right adjust it. */
    adjust: boolean;
    /** Reads like a page (Details' summary): Up / Down scroll it first. */
    read: boolean;
    disabled: boolean;
    /** Inside the screen's rail. */
    inRail: boolean;
    /** A cover (select mode picks it). */
    cover: boolean;
  } | null;
  /** The screen has a rail. */
  hasRail: boolean;
  /** Which of its own handlers the screen (tab or page) registered. */
  screen: {
    x: boolean;
    y: boolean;
    lt: boolean;
    rt: boolean;
    lb: boolean;
    rb: boolean;
    /** Start presses the page's main button (`press`), says why it is
     *  disabled (`why`), or the page has none (null). */
    main: 'press' | 'why' | null;
  };
};

export type Command =
  | { kind: 'none' }
  | { kind: 'move'; dir: Direction }
  | { kind: 'adjust'; delta: -1 | 1 }
  | { kind: 'read'; dir: 'up' | 'down' }
  | { kind: 'activate' }
  /** A on a disabled control: say why (its data-why), as an error notice. */
  | { kind: 'why' }
  | { kind: 'closeLayer' }
  | { kind: 'pop' }
  | { kind: 'selToggle' }
  | { kind: 'selPick' }
  | { kind: 'selAll' }
  | { kind: 'selAct' }
  | { kind: 'toRail' }
  /** X on the focused item (its quick or extra action), else the screen's. */
  | { kind: 'x' }
  | { kind: 'screen'; handler: 'y' | 'lt' | 'rt' | 'lb' | 'rb' }
  | { kind: 'tab'; step: -1 | 1 }
  | { kind: 'main' }
  | { kind: 'mainWhy' }
  | { kind: 'menu' }
  | { kind: 'view' };

const NONE: Command = { kind: 'none' };

export function press(b: TvButton, ctx: PressContext): Command {
  const top = ctx.layer;
  const f = ctx.focus;

  if (isDirection(b)) {
    if (f?.adjust && (b === 'LEFT' || b === 'RIGHT') && !top) return { kind: 'adjust', delta: b === 'LEFT' ? -1 : 1 };
    if (f?.read && (b === 'UP' || b === 'DOWN')) return { kind: 'read', dir: b === 'UP' ? 'up' : 'down' };
    return { kind: 'move', dir: directionOf(b) };
  }

  if (b === 'A') {
    if (!f) return NONE;
    if (f.disabled) return { kind: 'why' };
    if (!top && ctx.selMode && ctx.pages === 0 && f.cover) return { kind: 'selPick' };
    return { kind: 'activate' };
  }

  if (b === 'B') {
    // An overlay closes, unless it is blocking (item 92).
    if (top) return top.blocking ? NONE : { kind: 'closeLayer' };
    // Startup: only out of a page pushed from the first screen (item 97).
    if (ctx.startup) return ctx.pages > 1 ? { kind: 'pop' } : NONE;
    if (ctx.pages > 0) return { kind: 'pop' }; // item 93
    if (ctx.selMode) return { kind: 'selToggle' }; // item 94
    // From a grid, a pane or Discover's content to the rail (item 95);
    // anywhere else at a tab's top level, nothing (item 96).
    if (ctx.hasRail && f && !f.inRail) return { kind: 'toRail' };
    return NONE;
  }

  // An open overlay binds nothing else here (item 103); P6's keyboard and
  // media viewer answer their own buttons before this runs.
  if (top) return NONE;

  if (ctx.selMode && ctx.pages === 0 && (b === 'X' || b === 'Y')) return { kind: b === 'X' ? 'selAll' : 'selAct' };
  if (b === 'RT') return ctx.screen.rt ? { kind: 'screen', handler: 'rt' } : NONE;
  if (b === 'X') return { kind: 'x' };
  if (b === 'Y') return ctx.screen.y ? { kind: 'screen', handler: 'y' } : NONE;
  if (b === 'LT') return ctx.screen.lt ? { kind: 'screen', handler: 'lt' } : NONE;
  if (b === 'LB' || b === 'RB') {
    if (b === 'LB' && ctx.screen.lb) return { kind: 'screen', handler: 'lb' };
    if (b === 'RB' && ctx.screen.rb) return { kind: 'screen', handler: 'rb' };
    // Nothing on pages and startup screens (item 99); tabs otherwise (item 98).
    if (ctx.pages > 0 || ctx.startup) return NONE;
    return { kind: 'tab', step: b === 'LB' ? -1 : 1 };
  }
  if (b === 'MENU') {
    // Start presses the page's main button, or says why (items 100–101).
    if (ctx.screen.main === 'press') return { kind: 'main' };
    if (ctx.screen.main === 'why') return { kind: 'mainWhy' };
    // Elsewhere the Menu, which the startup screens don't have (ruling 6).
    return ctx.startup ? NONE : { kind: 'menu' };
  }
  if (b === 'VIEW') return { kind: 'view' };
  return NONE;
}

/** The tab LB / RB lands on, wrapping (item 98). */
export const TAB_ORDER = ['library', 'server', 'downloads', 'settings'] as const;
export type TvTab = (typeof TAB_ORDER)[number];

export function stepTab(tab: TvTab, step: -1 | 1): TvTab {
  const i = TAB_ORDER.indexOf(tab);
  return TAB_ORDER[(i + step + TAB_ORDER.length) % TAB_ORDER.length];
}
