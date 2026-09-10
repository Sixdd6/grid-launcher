// Pure bookkeeping for BackgroundArt's two-layer cross-fade. Kept separate
// from the component so the sequencing (which slot gets the new image, when
// the old one is safe to drop) is testable without mounting Svelte.
//
// Two DOM layers, 'a' and 'b', alternate being "top" (visible, opacity
// fading in) and "bottom" (fading out). A new cover is written ONLY into
// the bottom slot — the outgoing top slot's image is left untouched so both
// images are on screen together for the opacity transition. Once that
// transition has had time to finish, the caller clears the now-bottom
// slot's image; `clearIfBottom` guards that clear so a cover that arrived
// mid-fade and already reclaimed the slot is never wiped out from under it.

export type SlotId = 'a' | 'b';

export interface SlotState {
  top: SlotId;
  a: string | null;
  b: string | null;
}

export const initialSlotState: SlotState = { top: 'a', a: null, b: null };

function other(slot: SlotId): SlotId {
  return slot === 'a' ? 'b' : 'a';
}

/**
 * A new cover resolved. Writes it into the current bottom slot and makes
 * that slot the top one, so it fades in while the previous top slot (whose
 * image is left in place) fades out underneath it.
 */
export function withNextCover(state: SlotState, url: string): SlotState {
  const nextTop = other(state.top);
  return { ...state, top: nextTop, [nextTop]: url };
}

/**
 * The slot to clear once the fade that started with `withNextCover` has had
 * time to finish: the slot that was on top before that call, i.e. the one
 * still holding the image `withNextCover` did not touch.
 */
export function outgoingSlot(state: SlotState): SlotId {
  return other(state.top);
}

/**
 * Drops `slot`'s image, but only while it is still the bottom (invisible)
 * slot. If a third cover arrived mid-fade and promoted this slot back to
 * top in the meantime, the clear is a no-op — clearing it would blank out
 * the image currently on screen.
 */
export function clearIfBottom(state: SlotState, slot: SlotId): SlotState {
  if (state.top === slot) return state;
  if (state[slot] === null) return state;
  return { ...state, [slot]: null };
}
