import { describe, expect, it } from 'vitest';
import { clearIfBottom, initialSlotState, outgoingSlot, withNextCover } from './backgroundSlots';

describe('withNextCover / clearIfBottom', () => {
  it('the first image fades into the bottom slot, leaving the other empty', () => {
    const s1 = withNextCover(initialSlotState, 'https://romm/x.png');
    expect(s1).toEqual({ top: 'b', a: null, b: 'https://romm/x.png' });
  });

  it('a second image writes the other slot and keeps the old image until cleared', () => {
    const s1 = withNextCover(initialSlotState, 'https://romm/x.png'); // top: b, b: x
    const s2 = withNextCover(s1, 'https://romm/y.png'); // top: a, a: y, b still x

    expect(s2).toEqual({ top: 'a', a: 'https://romm/y.png', b: 'https://romm/x.png' });

    // The slot the caller should schedule a clear for is the one that just
    // stopped being on top — still holding the old image.
    expect(outgoingSlot(s2)).toBe('b');

    // Clearing it once the fade has finished drops the stale image.
    const cleared = clearIfBottom(s2, 'b');
    expect(cleared).toEqual({ top: 'a', a: 'https://romm/y.png', b: null });
  });

  it('a third cover arriving mid-fade reclaims the slot before its clear fires, and the guard leaves it alone', () => {
    const s1 = withNextCover(initialSlotState, 'https://romm/x.png'); // top: b, b: x
    const s2 = withNextCover(s1, 'https://romm/y.png'); // top: a, a: y, b: x (b's clear is now pending)

    // Before that pending clear for 'b' fires, a third cover arrives.
    const s3 = withNextCover(s2, 'https://romm/z.png'); // top: b, b: z, a: y (a's clear is now pending)
    expect(s3).toEqual({ top: 'b', a: 'https://romm/y.png', b: 'https://romm/z.png' });

    // The stale clear for 'b' (scheduled after the y-write) must not wipe
    // out z now that 'b' is on top again.
    const guarded = clearIfBottom(s3, 'b');
    expect(guarded).toEqual(s3);

    // The correct pending clear is for 'a', which is now the bottom slot.
    expect(outgoingSlot(s3)).toBe('a');
    const cleared = clearIfBottom(s3, 'a');
    expect(cleared).toEqual({ top: 'b', a: null, b: 'https://romm/z.png' });
  });

  it('keeps alternating across a 5s cycle through three images', () => {
    let state = initialSlotState;
    state = withNextCover(state, 'one');
    expect(state.top).toBe('b');
    state = clearIfBottom(state, outgoingSlot(state));
    state = withNextCover(state, 'two');
    expect(state.top).toBe('a');
    expect(state.a).toBe('two');
    // The outgoing image is still on screen for the fade.
    expect(state.b).toBe('one');
    state = withNextCover(state, 'three');
    expect(state.top).toBe('b');
    expect(state.b).toBe('three');
  });

  it('clearing an already-empty slot is a no-op', () => {
    expect(clearIfBottom(initialSlotState, 'a')).toEqual(initialSlotState);
  });
});
