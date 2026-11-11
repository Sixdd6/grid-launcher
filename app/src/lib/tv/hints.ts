// The hint bar (checklist items 128–137; tvmode.js `hintsNow`,
// `paintHints`). Pure: what each button does right now, and the left side.
import type { GlyphButton } from './glyphs/glyphs';

/** A hint; `off` greys it out (the button does nothing right now, as the
 *  on-screen keyboard's R3 Paste with nothing on the clipboard). */
export type Hint = { button: GlyphButton; label: string; off?: boolean };

export type HintInput = {
  /** The top overlay: its own hints, or the default Select / Back. */
  layer: { hints: Hint[] | null; blocking: boolean } | null;
  /** The focused item: its A label (`data-al`, default "Select"), whether it
   *  is disabled, and its X label (`data-xl`: a cover's quick action, a
   *  row's extra action), if any. */
  focus: { a: string; disabled: boolean; x: string | null } | null;
  /** Select mode's own hints replace the rest (P13). */
  select: Hint[] | null;
  /** The screen's own hints (Y Search, LT Sort, …). */
  screen: Hint[];
  pages: number;
  startup: boolean;
  /** Start presses the page's main button here (so no Menu hint). */
  main: boolean;
};

/**
 * In order: A for the focused item (left out when it is disabled), X, the
 * screen's own hints, B "Back" on pages (not on startup screens), Menu
 * "Menu" (left out where Start presses the page's main button; a startup
 * screen's Menu is the short one, user decision 2026-11-10).
 */
export function buildHints(i: HintInput): Hint[] {
  if (i.layer) {
    if (i.layer.hints) return i.layer.hints;
    const h: Hint[] = [{ button: 'A', label: 'Select' }];
    if (!i.layer.blocking) h.push({ button: 'B', label: 'Back' });
    return h;
  }
  const h: Hint[] = [];
  if (i.focus && !i.focus.disabled) h.push({ button: 'A', label: i.focus.a });
  if (i.select) return i.select;
  if (i.focus?.x) h.push({ button: 'X', label: i.focus.x });
  h.push(...i.screen);
  if (i.pages > 0 && !i.startup) h.push({ button: 'B', label: 'Back' });
  if (!i.main) h.push({ button: 'MENU', label: 'Menu' });
  return h;
}

export type HintLeft =
  | { kind: 'noServer'; text: string }
  | { kind: 'offline'; pill: string; text: string }
  | { kind: 'pad'; text: string };

export const NO_SERVER_LINE = 'No server · Library and backup drives only';
export const OFFLINE_PILL = 'Offline';
export const OFFLINE_LINE = 'Installed games still play';
/** Ruling 24: the left side with no controller connected. */
export const NO_CONTROLLER_LINE = 'No controller connected';

/**
 * The left side: GRID used without a server, or the server unreachable (not
 * on the startup screens); else the controller in use, or the no-controller
 * line.
 */
export function hintLeft(o: { noServer: boolean; offline: boolean; startup: boolean; padName: string | null }): HintLeft {
  if (o.noServer && !o.startup) return { kind: 'noServer', text: NO_SERVER_LINE };
  if (o.offline && !o.startup) return { kind: 'offline', pill: OFFLINE_PILL, text: OFFLINE_LINE };
  return { kind: 'pad', text: o.padName ?? NO_CONTROLLER_LINE };
}
