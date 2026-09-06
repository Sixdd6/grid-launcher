// @vitest-environment node
import { beforeEach, describe, expect, it } from 'vitest';
import { inputMode, noteInput, resetInputMode } from './inputMode.svelte';

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
