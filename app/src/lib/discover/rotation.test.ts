import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRotation } from './rotation';

describe('createRotation', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('advances every period', () => {
    const advance = vi.fn();
    const r = createRotation(10_000, advance);
    r.restart();
    vi.advanceTimersByTime(9_999);
    expect(advance).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(advance).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(10_000);
    expect(advance).toHaveBeenCalledTimes(2);
    r.stop();
  });

  it('pauses with the rest of the period kept', () => {
    const advance = vi.fn();
    const r = createRotation(10_000, advance);
    r.restart();
    vi.advanceTimersByTime(4_000);
    r.pause();
    expect(r.paused()).toBe(true);
    expect(r.remaining()).toBe(6_000);
    vi.advanceTimersByTime(60_000);
    expect(advance).not.toHaveBeenCalled();
    r.resume();
    vi.advanceTimersByTime(5_999);
    expect(advance).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(advance).toHaveBeenCalledTimes(1);
    r.stop();
  });

  it('restarts a full period on a jump, even while paused', () => {
    const advance = vi.fn();
    const r = createRotation(10_000, advance);
    r.restart();
    vi.advanceTimersByTime(8_000);
    r.restart();
    vi.advanceTimersByTime(8_000);
    expect(advance).not.toHaveBeenCalled();
    r.pause();
    r.restart();
    expect(r.remaining()).toBe(10_000);
    vi.advanceTimersByTime(20_000);
    expect(advance).not.toHaveBeenCalled();
    r.resume();
    vi.advanceTimersByTime(10_000);
    expect(advance).toHaveBeenCalledTimes(1);
    r.stop();
  });

  it('does nothing once stopped', () => {
    const advance = vi.fn();
    const r = createRotation(1_000, advance);
    r.restart();
    r.stop();
    r.resume();
    vi.advanceTimersByTime(5_000);
    expect(advance).not.toHaveBeenCalled();
  });
});
