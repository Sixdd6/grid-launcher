import { afterEach, describe, expect, it, vi } from 'vitest';
import { hasOpenDialog, registerDialog, resetDialogsForTest, routeDialogNav, stepIndex } from './dialogNav';

afterEach(() => resetDialogsForTest());

describe('routeDialogNav', () => {
  it('does not consume anything while no dialog is open', () => {
    expect(hasOpenDialog()).toBe(false);
    expect(routeDialogNav('back')).toBe(false);
    expect(routeDialogNav('down')).toBe(false);
  });

  it('hands every action to the open dialog and consumes it', () => {
    const handler = vi.fn(() => true);
    registerDialog(handler);
    expect(hasOpenDialog()).toBe(true);
    expect(routeDialogNav('accept')).toBe(true);
    expect(handler).toHaveBeenCalledWith('accept');
  });

  it('only the topmost dialog sees the action', () => {
    const lower = vi.fn(() => true);
    const upper = vi.fn(() => true);
    registerDialog(lower);
    registerDialog(upper);
    routeDialogNav('down');
    expect(upper).toHaveBeenCalledTimes(1);
    expect(lower).not.toHaveBeenCalled();
  });

  it('an action the dialog does not handle is still consumed, never leaked', () => {
    registerDialog(() => false);
    expect(routeDialogNav('left')).toBe(true);
    expect(routeDialogNav('back')).toBe(true);
  });

  it('closing the top dialog hands routing back to the one below', () => {
    const lower = vi.fn(() => true);
    const upper = vi.fn(() => true);
    registerDialog(lower);
    const unregisterUpper = registerDialog(upper);
    unregisterUpper();
    routeDialogNav('up');
    expect(lower).toHaveBeenCalledWith('up');
    expect(upper).not.toHaveBeenCalled();
  });

  it('unregistering a lower dialog leaves the top one in charge', () => {
    const lower = vi.fn(() => true);
    const upper = vi.fn(() => true);
    const unregisterLower = registerDialog(lower);
    registerDialog(upper);
    unregisterLower();
    routeDialogNav('up');
    expect(upper).toHaveBeenCalledWith('up');
    expect(lower).not.toHaveBeenCalled();
  });

  it('unregistering twice is harmless and the last dialog closing clears the stack', () => {
    const unregister = registerDialog(() => true);
    unregister();
    unregister();
    expect(hasOpenDialog()).toBe(false);
    expect(routeDialogNav('back')).toBe(false);
  });

  it('the same handler registered twice is two dialogs, each removed once', () => {
    const handler = vi.fn(() => true);
    const first = registerDialog(handler);
    registerDialog(handler);
    first();
    expect(hasOpenDialog()).toBe(true);
  });

  it('a dialog closed by its own back action stops receiving events', () => {
    let unregister = () => {};
    const handler = vi.fn((action: string) => {
      if (action === 'back') unregister();
      return true;
    });
    unregister = registerDialog(handler);
    expect(routeDialogNav('back')).toBe(true);
    expect(routeDialogNav('back')).toBe(false);
    expect(handler).toHaveBeenCalledTimes(1);
  });
});

describe('stepIndex', () => {
  it('down and up move one place, clamped at both ends', () => {
    expect(stepIndex(0, 'down', 3)).toBe(1);
    expect(stepIndex(2, 'down', 3)).toBe(2);
    expect(stepIndex(2, 'up', 3)).toBe(1);
    expect(stepIndex(0, 'up', 3)).toBe(0);
  });

  it('with nothing focused (-1) either vertical direction lands on the first item', () => {
    expect(stepIndex(-1, 'down', 3)).toBe(0);
    expect(stepIndex(-1, 'up', 3)).toBe(0);
  });

  it('left and right do nothing unless horizontal movement is asked for', () => {
    expect(stepIndex(1, 'left', 3)).toBe(1);
    expect(stepIndex(1, 'right', 3)).toBe(1);
    expect(stepIndex(1, 'left', 3, true)).toBe(0);
    expect(stepIndex(1, 'right', 3, true)).toBe(2);
  });

  it('an empty list has no index to move to', () => {
    expect(stepIndex(-1, 'down', 0)).toBe(-1);
  });

  it('a non-direction action leaves the index alone', () => {
    expect(stepIndex(1, 'accept', 3)).toBe(1);
  });
});
