// What a TV screen opens over itself (TV mode plan P6; checklist items
// 360–388; tvmode.js `openSheet`, `confirmBox`, `quitBox`, `openOSK`,
// `guideMenu`). Each is a layer on nav's overlay stack (nav.svelte.ts
// `openLayer` / `closeLayer`), drawn by TvShell inside its `.layers` box.
import { api } from '../../api';
import type { Hint } from '../hints';
import { closeLayer, openLayer } from '../nav.svelte';
import { DONE, OSK_BINDS, oskOpen, oskRepeats, oskType, physicalKey, SYMBOLS, SYMBOLS_KEY, type Osk } from './osk';
import TvConfirm from './TvConfirm.svelte';
import TvKeyboard from './TvKeyboard.svelte';
import TvList from './TvList.svelte';
import TvMenu from './TvMenu.svelte';

/** One row of a list: an option, or a group heading (not focusable, item 370). */
export type ListItem =
  | {
      label: string;
      /** A muted line on the option's right (item 371). */
      sub?: string;
      /** The current value: a check, and focus when the list opens (item 369).
       *  Set it (true or false) on every option of a choice list, so each
       *  has room for the check. */
      checked?: boolean;
      danger?: boolean;
      /** Runs after the list closes (item 372). */
      onPick?: () => void;
      testId?: string;
    }
  | { group: string };

export type ListSpec = { title: string; sub?: string; items: ListItem[] };

/**
 * A centred list (item 368): title, an optional line under it, the options
 * with a check on the current value. A picks and closes; B closes with no
 * change; the hints are A "Select", B "Back".
 */
export function openList(spec: ListSpec): Promise<number> {
  return openLayer({ component: TvList, props: { spec } });
}

export type ConfirmSpec = {
  title: string;
  body: string;
  /** The action button's label ("Quit", "Delete"). */
  ok: string;
  /** Danger-styled for a destructive action, primary otherwise (item 375). */
  danger?: boolean;
  /** Runs after the dialog closes. */
  onOk: () => void;
};

/**
 * Asks first (items 373–375): a centred dialog with a title, a body, then
 * Cancel and the action, right-aligned. It opens with Cancel focused, so a
 * double press of A can't confirm by accident; B cancels.
 */
export function confirmBox(spec: ConfirmSpec): Promise<number> {
  return openLayer({ component: TvConfirm, props: { spec } });
}

/** Menu › Quit GRID (item 376). */
export function quitBox(): Promise<number> {
  return confirmBox({
    title: 'Quit GRID?',
    body: 'Downloads in progress pause and resume the next time GRID starts.',
    ok: 'Quit',
    onOk: () => void api.quitApp(),
  });
}

export type KeyboardSpec = {
  /** The field's name, muted above it ("Search", "API token"). */
  title: string;
  value?: string;
  /** A token or password: dots, and it opens empty (item 386). */
  mask?: boolean;
  /** Every change of the value (a search narrows as you type, item 387). */
  onInput?: (value: string) => void;
  /** Done (A on Done, Menu, or Enter after physical typing): the value. */
  onDone?: (value: string) => void;
  /** B: the field keeps the value it had (item 385). */
  onCancel?: () => void;
};

/** The keyboard's hints (item 384). */
export const KEYBOARD_HINTS: Hint[] = [
  { button: 'A', label: 'Type' },
  { button: 'X', label: 'Delete' },
  { button: 'Y', label: 'Space' },
  { button: 'LB', label: '◀' },
  { button: 'RB', label: '▶' },
  { button: 'MENU', label: 'Done' },
  { button: 'B', label: 'Cancel' },
];

/** The keyboard's state, shared by its layer's bindings and TvKeyboard. */
export type KeyboardModel = { osk: Osk; mask: boolean; title: string; type: (key: string) => void };

/**
 * The on-screen keyboard (items 377–388): slides up from the bottom, over
 * the stage and up to the hint bar, which shows its hints (ruling 20).
 * X deletes, Y adds a space, LB / RB move the caret, LT is Shift, Menu is
 * Done, B cancels; held typing buttons repeat (items 42–43); a physical
 * keyboard types straight in (item 74).
 */
export function openKeyboard(spec: KeyboardSpec): Promise<number> {
  const mask = !!spec.mask;
  let done = false;
  const model: KeyboardModel = $state({
    osk: oskOpen(spec.value ?? '', mask),
    mask,
    title: spec.title,
    type: (key: string) => {
      if (key === DONE) {
        done = true;
        const value = model.osk.value;
        void closeLayer().then(() => spec.onDone?.(value));
        return;
      }
      if (key === SYMBOLS_KEY) {
        void openList({ title: 'Symbols', items: SYMBOLS.map((c) => ({ label: c, onPick: () => model.type(c) })) });
        return;
      }
      const before = model.osk.value;
      model.osk = oskType(model.osk, key);
      if (model.osk.value !== before) spec.onInput?.(model.osk.value);
    },
  });
  const binds: Record<string, () => void> = {};
  for (const [b, key] of Object.entries(OSK_BINDS)) binds[b] = () => model.type(key as string);
  return openLayer({
    component: TvKeyboard,
    props: { model },
    cls: 'kb',
    hints: KEYBOARD_HINTS,
    binds,
    typingRepeat: (b, el) => oskRepeats(b, el?.getAttribute('data-key') ?? null),
    keydown: (e) => {
      const hit = physicalKey(model.osk, { key: e.key, ctrl: e.ctrlKey, meta: e.metaKey });
      if (!hit) return false;
      if (e.key.length === 1) model.osk = { ...model.osk, phys: true };
      model.type(hit.type);
      return true;
    },
    onClose: () => {
      if (!done) spec.onCancel?.();
    },
  });
}

/** The Menu panel's hints (item 366). */
export const MENU_HINTS: Hint[] = [
  { button: 'A', label: 'Select' },
  { button: 'B', label: 'Close' },
];

/** The Menu (Start; items 362–366): a panel from the right. */
export function openMenu(): Promise<number> {
  return openLayer({ component: TvMenu, cls: 'side', hints: MENU_HINTS });
}

/** The Menu on a startup screen: the same panel with only Switch to desktop
 *  mode and Quit GRID (user decision 2026-11-10). */
export function openStartupMenu(): Promise<number> {
  return openLayer({ component: TvMenu, cls: 'side', hints: MENU_HINTS, props: { startup: true } });
}
