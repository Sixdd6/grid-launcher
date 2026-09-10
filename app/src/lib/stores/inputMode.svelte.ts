// Which input method the user is driving the app with right now. Module
// scoped `$state`, like `lastViewed.svelte.ts`: every view and card reads it,
// and none of them own it.
//
// User ruling 2026-09-05: selection follows the active input method. A
// pointer user hovers, so no card carries the selection grow until a
// directional input (keyboard or gamepad) happens.

export type InputKind = 'pointer' | 'keyboard' | 'gamepad';

/** A pointer position in client (viewport) coordinates. */
export type PointerPoint = { x: number; y: number };

/**
 * How far the pointer must travel before a `pointermove` counts as the user
 * reaching for the mouse. MAX-AXIS distance (Chebyshev), not Euclidean: the
 * two agree to within a factor of √2 and one `Math.abs` pair per move is
 * cheaper than a square root on an event that fires per frame.
 *
 * Why any threshold at all: WebKit dispatches a synthetic `pointermove` at
 * the UNCHANGED cursor position after a scroll, so `:hover` can be recomputed
 * for whatever slid under a stationary cursor. Arrow-key navigation scrolls
 * (`scrollIntoView`), so without this a keyboard user whose mouse merely
 * rests on the grid would be thrown back into pointer mode mid-navigation and
 * lose the selection grow. 4px also absorbs a sub-pixel jitter without
 * costing a real mouse user anything — a hand moving a mouse crosses 4px in
 * one frame.
 */
export const POINTER_WAKE_PX = 4;

/**
 * Whether a move from `last` to `now` is big enough to be a user reaching for
 * the mouse. Pure, so the rule is testable without a window or a component.
 *
 * `last === null` means no position was recorded when the directional input
 * happened, so there is nothing to measure against: answer `false` and let
 * the caller take this move as the new reference. That is the safe direction
 * — a real mouse move is a stream of events, so the next one switches.
 */
export function shouldSwitchToPointer(last: PointerPoint | null, now: PointerPoint): boolean {
  if (last === null) return false;
  return Math.max(Math.abs(now.x - last.x), Math.abs(now.y - last.y)) >= POINTER_WAKE_PX;
}

const state = $state<{ current: InputKind }>({ current: 'pointer' });

/** Where the pointer was when the last DIRECTIONAL input fired, or `null`
 *  when none has fired since the last switch to pointer mode. Deliberately
 *  NOT `$state`: nothing renders from it, and a per-frame write to reactive
 *  state would wake every reader of the store. */
let anchor: PointerPoint | null = null;
/** The most recent position any `pointermove` reported, so a directional
 *  writer can anchor without the caller carrying coordinates around. */
let lastPointer: PointerPoint | null = null;

export const inputMode = {
  get current(): InputKind {
    return state.current;
  },
  /** Keyboard and gamepad move a selection; the pointer only hovers. */
  get directional(): boolean {
    return state.current !== 'pointer';
  },
};

/**
 * Records the input that just happened, unconditionally. A no-op when
 * unchanged, so a stream of pointer events does not churn the store.
 *
 * The pointer path uses this for `pointerdown` only — a click is always a
 * deliberate pointer action. A bare move goes through `notePointerAt`.
 */
export function noteInput(kind: InputKind): void {
  if (kind === 'pointer') anchor = null;
  else anchor = lastPointer;
  if (state.current !== kind) state.current = kind;
}

/**
 * Records a keyboard or gamepad input and remembers where the pointer was
 * sitting at that moment, which is what `notePointerAt` measures against.
 */
export function noteDirectional(kind: 'keyboard' | 'gamepad'): void {
  noteInput(kind);
}

/**
 * Records a `pointermove` at `(x, y)`. Switches back to pointer mode only
 * when the pointer has actually travelled [`POINTER_WAKE_PX`] from where it
 * sat at the last directional input — see `shouldSwitchToPointer`.
 */
export function notePointerAt(x: number, y: number): void {
  const now = { x, y };
  lastPointer = now;
  if (state.current === 'pointer') return;
  if (anchor === null) {
    anchor = now;
    return;
  }
  if (!shouldSwitchToPointer(anchor, now)) return;
  anchor = null;
  state.current = 'pointer';
}

/** Test-only reset. */
export function resetInputMode(): void {
  state.current = 'pointer';
  anchor = null;
  lastPointer = null;
}
