// TV mode's logical buttons and the keyboard that stands in for a controller
// (TVMode README § Try it; checklist items 34, 71). Pure.

/** What a press means, after the controller's button assignment. */
export type TvButton =
  | 'A' | 'B' | 'X' | 'Y' | 'LB' | 'RB' | 'LT' | 'RT' | 'VIEW' | 'MENU'
  | 'UP' | 'DOWN' | 'LEFT' | 'RIGHT';

export type Direction = 'up' | 'down' | 'left' | 'right';

export const DIRECTION_BUTTONS: readonly TvButton[] = ['UP', 'DOWN', 'LEFT', 'RIGHT'];

export function isDirection(b: TvButton): b is 'UP' | 'DOWN' | 'LEFT' | 'RIGHT' {
  return b === 'UP' || b === 'DOWN' || b === 'LEFT' || b === 'RIGHT';
}

export function directionOf(b: 'UP' | 'DOWN' | 'LEFT' | 'RIGHT'): Direction {
  return b.toLowerCase() as Direction;
}

/**
 * The keyboard as a controller (item 71): arrows move, Enter or Space is A,
 * Esc or Backspace is B, X, Y, Q = LB, E = RB, Z = LT, R = RT, M = Menu,
 * V = View. Page Up / Page Down are the right stick (the router handles
 * them). The prototype's G (Guide) is left out: Guide does nothing (ruling 5).
 */
export const KEYMAP: Readonly<Record<string, TvButton>> = {
  ArrowUp: 'UP',
  ArrowDown: 'DOWN',
  ArrowLeft: 'LEFT',
  ArrowRight: 'RIGHT',
  Enter: 'A',
  ' ': 'A',
  Escape: 'B',
  Backspace: 'B',
  x: 'X',
  y: 'Y',
  q: 'LB',
  e: 'RB',
  z: 'LT',
  r: 'RT',
  m: 'MENU',
  v: 'VIEW',
};

/** The button a key stands for, or null. Chords with Ctrl, Alt or Meta are
 *  never buttons, so the window's own shortcuts keep working. */
export function buttonForKey(key: string, mods: { ctrl?: boolean; alt?: boolean; meta?: boolean } = {}): TvButton | null {
  if (mods.ctrl || mods.alt || mods.meta) return null;
  return KEYMAP[key] ?? KEYMAP[key.toLowerCase()] ?? null;
}
