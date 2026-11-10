// The app-wide message surface: the cards in Toast.svelte's bottom-right
// stack (originally the port of `show_toast`, grid_launcher/ui/toast.py:97).
// Module-scoped `$state` so any component can push without prop drilling,
// mirroring `stores/appUpdate.svelte.ts`.
//
// The `append*`/`remove*` pair below is pure and exported for vitest; the
// `$state` wrapper under it is the part components read. Same split as
// `stores/updates.svelte.ts`'s `labelFor`.

import { CountdownTimer } from '../countdown';

export type ToastLevel = 'success' | 'error';

/** What a card offers beside its text. The desktop's cards show neither;
 *  TV mode's cards show the second line, and View runs the action (TV mode
 *  plan P6, checklist items 144, 148). */
export type ToastAction = { label: string; run: () => void };
export type ToastExtra = { sub?: string; action?: ToastAction };

/** A pushed card, as a subscriber (TV mode's card stack) sees it. */
export type PushedToast = { text: string; level: ToastLevel; durationMs: number } & ToastExtra;

export type Toast = {
  id: number;
  text: string;
  level: ToastLevel;
  /** The card's whole life, which its countdown bar runs down over. */
  durationMs: number;
  /** True while the pointer is over the card or focus is inside it. */
  paused: boolean;
};

/**
 * Every message is a card in one stack at the bottom right (design: "One
 * notification style"). Four fit above the download strip; a fifth drops
 * the oldest.
 */
export const TOAST_LIMIT = 4;

/**
 * A card closes itself after ten seconds (its Dismiss bar runs down over
 * that), paused while pointed at. A few one-time notices that need reading
 * and acting on pass a longer duration of their own.
 */
export const TOAST_DURATION_MS = 10_000;

/** `show_message` ignores a blank message (toast.py:64-66); so does this. */
export function appendToast(list: Toast[], next: Toast, limit: number = TOAST_LIMIT): Toast[] {
  if (next.text.trim() === '') return list;
  return [...list, next].slice(-limit);
}

export function removeToast(list: Toast[], id: number): Toast[] {
  return list.filter((t) => t.id !== id);
}

export function setPaused(list: Toast[], id: number, paused: boolean): Toast[] {
  return list.map((t) => (t.id === id && t.paused !== paused ? { ...t, paused } : t));
}

const state = $state<{ list: Toast[] }>({ list: [] });

/** One countdown per card on screen, keyed by toast id. */
const timers = new Map<number, CountdownTimer>();

let nextId = 0;

const subscribers = new Set<(t: PushedToast) => void>();

/**
 * Calls `fn` with every card pushed from now on (blank ones excepted), so
 * TV mode can show the same events in its own stack with its own limit.
 * Answers the function that unsubscribes. The desktop stack is unchanged.
 */
export function onToastPushed(fn: (t: PushedToast) => void): () => void {
  subscribers.add(fn);
  return () => {
    subscribers.delete(fn);
  };
}

export const toasts = {
  get list() {
    return state.list;
  },
};

/**
 * Shows `text` for `durationMs` (default [`TOAST_DURATION_MS`]) of unpaused
 * time. Returns the new toast's id, or `null` when the message was blank and
 * nothing was shown.
 */
export function pushToast(
  text: string,
  level: ToastLevel = 'success',
  durationMs: number = TOAST_DURATION_MS,
  extra: ToastExtra = {},
): number | null {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  for (const fn of subscribers) fn({ text: trimmed, level, durationMs, ...extra });
  nextId += 1;
  const id = nextId;
  const next = appendToast(state.list, { id, text: trimmed, level, durationMs, paused: false });
  // Cards that fell off the top of a full stack take their timers with them.
  const kept = new Set(next.map((t) => t.id));
  for (const t of state.list) if (!kept.has(t.id)) stopTimer(t.id);
  state.list = next;
  timers.set(id, new CountdownTimer(durationMs, () => dismissToast(id)));
  return id;
}

export function dismissToast(id: number): void {
  stopTimer(id);
  state.list = removeToast(state.list, id);
}

/** Holds a card's countdown while it is pointed at or focused. */
export function pauseToast(id: number): void {
  timers.get(id)?.pause();
  state.list = setPaused(state.list, id, true);
}

export function resumeToast(id: number): void {
  timers.get(id)?.resume();
  state.list = setPaused(state.list, id, false);
}

function stopTimer(id: number): void {
  timers.get(id)?.stop();
  timers.delete(id);
}
