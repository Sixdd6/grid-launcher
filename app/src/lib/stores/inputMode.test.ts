// @vitest-environment node
import { beforeEach, describe, expect, it } from 'vitest';
import {
  inputMode,
  noteDirectional,
  noteInput,
  notePointerAt,
  POINTER_WAKE_PX,
  resetInputMode,
  shouldSwitchToPointer,
} from './inputMode.svelte';

// Module-scoped `$state`, so every case starts from the documented default
// through the store's own test-only reset rather than a module reload.
beforeEach(() => {
  resetInputMode();
});

describe('inputMode', () => {
  it('starts on the pointer, which is not a directional input', () => {
    expect(inputMode.current).toBe('pointer');
    expect(inputMode.directional).toBe(false);
  });

  it('treats the keyboard as directional', () => {
    noteInput('keyboard');
    expect(inputMode.current).toBe('keyboard');
    expect(inputMode.directional).toBe(true);
  });

  it('treats the gamepad as directional', () => {
    noteInput('gamepad');
    expect(inputMode.current).toBe('gamepad');
    expect(inputMode.directional).toBe(true);
  });

  it('drops back out of directional mode when the pointer moves again', () => {
    noteInput('keyboard');
    noteInput('pointer');
    expect(inputMode.current).toBe('pointer');
    expect(inputMode.directional).toBe(false);
  });

  // The window pointermove listener calls `noteInput` on every move, so an
  // unchanged kind must not write to the state and wake its readers.
  it('repeats the same kind without changing the recorded mode', () => {
    noteInput('pointer');
    noteInput('pointer');
    expect(inputMode.current).toBe('pointer');
    noteInput('gamepad');
    noteInput('gamepad');
    expect(inputMode.current).toBe('gamepad');
  });

  it('resets to the pointer default', () => {
    noteInput('keyboard');
    resetInputMode();
    expect(inputMode.current).toBe('pointer');
    expect(inputMode.directional).toBe(false);
  });
});

// The rule finding 1 exists for: WebKit dispatches a synthetic `pointermove`
// at the UNCHANGED cursor position after a scroll, and arrow-key navigation
// scrolls. A keyboard user whose mouse rests on the grid must keep the mode.
describe('shouldSwitchToPointer', () => {
  it('refuses a move with no recorded reference position', () => {
    expect(shouldSwitchToPointer(null, { x: 800, y: 600 })).toBe(false);
  });

  it('refuses a move that did not move at all', () => {
    expect(shouldSwitchToPointer({ x: 400, y: 300 }, { x: 400, y: 300 })).toBe(false);
  });

  it('refuses a move shorter than the threshold on either axis', () => {
    const last = { x: 400, y: 300 };
    expect(shouldSwitchToPointer(last, { x: 403, y: 303 })).toBe(false);
    expect(shouldSwitchToPointer(last, { x: 397, y: 297 })).toBe(false);
  });

  // Max-axis, not Euclidean: one axis reaching the threshold is enough.
  it('accepts a move that reaches the threshold on one axis', () => {
    const last = { x: 400, y: 300 };
    expect(shouldSwitchToPointer(last, { x: 400 + POINTER_WAKE_PX, y: 300 })).toBe(true);
    expect(shouldSwitchToPointer(last, { x: 400, y: 300 - POINTER_WAKE_PX })).toBe(true);
  });
});

describe('notePointerAt', () => {
  it('keeps the keyboard mode through a synthetic move at the same position', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    // What a `scrollIntoView` produces: the cursor has not moved.
    notePointerAt(400, 300);
    notePointerAt(400, 300);
    expect(inputMode.current).toBe('keyboard');
    expect(inputMode.directional).toBe(true);
  });

  it('keeps the keyboard mode through a sub-threshold jitter', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    notePointerAt(402, 301);
    notePointerAt(403, 302);
    expect(inputMode.current).toBe('keyboard');
  });

  it('switches back to the pointer once the mouse really moves', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    notePointerAt(440, 320);
    expect(inputMode.current).toBe('pointer');
    expect(inputMode.directional).toBe(false);
  });

  // The delta is measured from where the pointer sat at the DIRECTIONAL
  // input, not from the previous move, so a slow drift still adds up.
  it('measures the delta from the position held at the last directional input', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    notePointerAt(402, 300);
    expect(inputMode.current).toBe('keyboard');
    notePointerAt(404, 300);
    expect(inputMode.current).toBe('pointer');
  });

  // No move has ever reached the window (a fresh window, or WebDriver, which
  // never moves its synthetic pointer): the first move becomes the reference
  // rather than switching, which is the safe direction for a keyboard user.
  it('takes the first move after a directional input as the reference', () => {
    noteDirectional('keyboard');
    notePointerAt(400, 300);
    expect(inputMode.current).toBe('keyboard');
    notePointerAt(400, 300);
    expect(inputMode.current).toBe('keyboard');
    notePointerAt(500, 300);
    expect(inputMode.current).toBe('pointer');
  });

  it('re-anchors on each directional input, so a second run needs its own move', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    notePointerAt(440, 320);
    expect(inputMode.current).toBe('pointer');
    noteDirectional('keyboard');
    notePointerAt(441, 321);
    expect(inputMode.current).toBe('keyboard');
  });

  it('costs nothing while the pointer is already the active mode', () => {
    notePointerAt(400, 300);
    notePointerAt(900, 700);
    expect(inputMode.current).toBe('pointer');
  });
});

// A click is always a deliberate pointer action, whatever the delta.
describe('noteInput on a pointer down', () => {
  it('switches to the pointer without any movement at all', () => {
    notePointerAt(400, 300);
    noteDirectional('keyboard');
    noteInput('pointer');
    expect(inputMode.current).toBe('pointer');
    expect(inputMode.directional).toBe(false);
  });
});
