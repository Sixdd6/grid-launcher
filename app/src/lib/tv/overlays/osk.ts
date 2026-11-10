// The on-screen keyboard's model (checklist items 42–43, 74, 377–388;
// tvmode.js `KEYS`, `openOSK`, `oskRepeats`, `keydown`). Pure: TvKeyboard
// draws it, overlays.svelte.ts binds the buttons to it.
//
// A masked field (a token, a password) shows dots and opens empty; the value
// lives only in the keyboard's state and goes straight to the caller's
// `onDone` (plan § rules: secrets never reach config, logs or IPC).
import type { TvButton } from '../input/buttons';

/** The four rows of keys (item 378). */
export const KEY_ROWS: readonly (readonly string[])[] = [
  ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
  ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
  ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', '-'],
  ['z', 'x', 'c', 'v', 'b', 'n', 'm', '.', '/', ':'],
];

/** The symbols list (item 382). */
export const SYMBOLS: readonly string[] = ['@', '_', '#', '%', '?', '=', '&', '"', '!', '+'];

/** What the last row's keys send (`data-key`). */
export const SHIFT = 'SHIFT';
export const SYMBOLS_KEY = '@';
export const SPACE = ' ';
export const BACK = 'BACK';
export const DONE = 'DONE';
/** LB / RB: the caret moves. */
export const CARET_LEFT = 'L';
export const CARET_RIGHT = 'R';

export type Osk = {
  value: string;
  caret: number;
  /** The next letter is a capital (item 381). */
  shift: boolean;
  /** Something was typed on a physical keyboard: Enter is then Done (item 74). */
  phys: boolean;
};

/** A keyboard opening on `value`; a masked field opens empty (item 386). */
export function oskOpen(value: string, mask: boolean): Osk {
  const v = mask ? '' : value;
  return { value: v, caret: v.length, shift: false, phys: false };
}

/**
 * The keyboard after one key: Shift toggles (and capitalises the next
 * letter only), Delete removes the character before the caret, L / R move
 * the caret, anything else is inserted at the caret. Done and the symbols
 * key are the caller's (they close the keyboard or open a list).
 */
export function oskType(k: Osk, key: string): Osk {
  switch (key) {
    case SHIFT:
      return { ...k, shift: !k.shift };
    case BACK:
      if (k.caret === 0) return k;
      return { ...k, value: k.value.slice(0, k.caret - 1) + k.value.slice(k.caret), caret: k.caret - 1 };
    case CARET_LEFT:
      return { ...k, caret: Math.max(0, k.caret - 1) };
    case CARET_RIGHT:
      return { ...k, caret: Math.min(k.value.length, k.caret + 1) };
    case DONE:
    case SYMBOLS_KEY:
      return k;
    default: {
      // The letter keys already show (and send) capitals while Shift is on.
      const value = k.value.slice(0, k.caret) + key + k.value.slice(k.caret);
      return { ...k, value, caret: k.caret + key.length, shift: k.shift && key.length === 1 ? false : k.shift };
    }
  }
}

/** A key's face and what it sends: a capital while Shift is on. */
export function keyFace(key: string, shift: boolean): string {
  return shift ? key.toUpperCase() : key;
}

/** The field as shown: dots for a masked field (item 386), split at the caret. */
export function oskShown(k: Osk, mask: boolean): { before: string; after: string } {
  const shown = mask ? '•'.repeat(k.value.length) : k.value;
  return { before: shown.slice(0, k.caret), after: shown.slice(k.caret) };
}

/**
 * Whether a held button repeats as typing (items 42–43): X, Y, LB and RB
 * always; A on a letter, number, symbol, Space or Delete key, but never on
 * Shift, the symbols key or Done. `focusedKey` is the focused key's
 * `data-key`, or null.
 */
export function oskRepeats(b: TvButton, focusedKey: string | null): boolean {
  if (b === 'X' || b === 'Y' || b === 'LB' || b === 'RB') return true;
  if (b !== 'A' || focusedKey === null) return false;
  return focusedKey !== SHIFT && focusedKey !== SYMBOLS_KEY && focusedKey !== DONE;
}

/** The buttons the keyboard binds itself (item 383): X deletes, Y adds a
 *  space, LB / RB move the caret, LT is Shift, Menu is Done. */
export const OSK_BINDS: Readonly<Partial<Record<TvButton, string>>> = {
  X: BACK,
  Y: SPACE,
  LB: CARET_LEFT,
  RB: CARET_RIGHT,
  LT: SHIFT,
  MENU: DONE,
};

/**
 * What a physical key does while the keyboard is open (item 74): a
 * printable key types itself (Space included), Backspace deletes, Enter is
 * Done once something was typed on the physical keyboard. Anything else
 * (arrows, Escape, Enter before typing) is the controller stand-in's.
 */
export function physicalKey(k: Osk, e: { key: string; ctrl?: boolean; meta?: boolean }): { type: string } | null {
  if (e.ctrl || e.meta) return null;
  if (e.key.length === 1) return { type: e.key };
  if (e.key === 'Backspace') return { type: BACK };
  if (e.key === 'Enter' && k.phys) return { type: DONE };
  return null;
}
