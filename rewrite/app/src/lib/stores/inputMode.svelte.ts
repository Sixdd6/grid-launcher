// Which input method the user is driving the app with right now. Module
// scoped `$state`, like `lastViewed.svelte.ts`: every view and card reads it,
// and none of them own it.
//
// User ruling 2026-09-05: selection follows the active input method. A
// pointer user hovers, so no card carries the selection grow until a
// directional input (keyboard or gamepad) happens.

export type InputKind = 'pointer' | 'keyboard' | 'gamepad';

const state = $state<{ current: InputKind }>({ current: 'pointer' });

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
 * Records the input that just happened. A no-op when unchanged, so a stream
 * of pointer moves does not churn the store.
 */
export function noteInput(kind: InputKind): void {
  if (state.current !== kind) state.current = kind;
}

/** Test-only reset. */
export function resetInputMode(): void {
  state.current = 'pointer';
}
