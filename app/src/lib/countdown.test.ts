import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CountdownTimer,
  holds,
  IDLE,
  nextEngagement,
  pauseCountdown,
  remainingMs,
  resumeCountdown,
  startCountdown,
} from './countdown';

describe('Countdown (pure)', () => {
  it('runs down from the start time', () => {
    const c = startCountdown(10_000, 1_000);
    expect(remainingMs(c, 1_000)).toBe(10_000);
    expect(remainingMs(c, 4_000)).toBe(7_000);
  });

  it('never goes below zero', () => {
    expect(remainingMs(startCountdown(10_000, 0), 99_000)).toBe(0);
  });

  it('freezes on pause and carries on from there on resume', () => {
    let c = startCountdown(10_000, 0);
    c = pauseCountdown(c, 3_000);
    expect(c.startedAt).toBeNull();
    expect(remainingMs(c, 50_000)).toBe(7_000);
    c = resumeCountdown(c, 50_000);
    expect(remainingMs(c, 52_000)).toBe(5_000);
  });

  it('treats a second pause or resume as a no-op', () => {
    const running = startCountdown(10_000, 0);
    expect(resumeCountdown(running, 5_000)).toBe(running);
    const paused = pauseCountdown(running, 2_000);
    expect(pauseCountdown(paused, 9_000)).toBe(paused);
  });
});

describe('CountdownTimer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('calls onDone once the duration has passed', () => {
    const done = vi.fn();
    new CountdownTimer(10_000, done);
    vi.advanceTimersByTime(9_999);
    expect(done).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(done).toHaveBeenCalledTimes(1);
  });

  it('does not count paused time', () => {
    const done = vi.fn();
    const timer = new CountdownTimer(10_000, done);
    vi.advanceTimersByTime(4_000);
    timer.pause();
    expect(timer.paused).toBe(true);
    vi.advanceTimersByTime(60_000);
    expect(done).not.toHaveBeenCalled();
    expect(timer.remainingMs).toBe(6_000);
    timer.resume();
    vi.advanceTimersByTime(5_999);
    expect(done).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(done).toHaveBeenCalledTimes(1);
  });

  it('never fires after stop, even when resumed', () => {
    const done = vi.fn();
    const timer = new CountdownTimer(10_000, done);
    timer.stop();
    timer.resume();
    vi.advanceTimersByTime(20_000);
    expect(done).not.toHaveBeenCalled();
  });

  it('ignores repeated pause and resume calls', () => {
    const done = vi.fn();
    const timer = new CountdownTimer(10_000, done);
    timer.resume();
    vi.advanceTimersByTime(2_000);
    timer.pause();
    timer.pause();
    timer.resume();
    timer.resume();
    vi.advanceTimersByTime(8_000);
    expect(done).toHaveBeenCalledTimes(1);
  });
});

describe('engagement', () => {
  it('holds while hovered or focused, and lets go only when neither', () => {
    let e = nextEngagement(IDLE, 'pointerenter');
    expect(holds(e)).toBe(true);
    e = nextEngagement(e, 'focusin');
    e = nextEngagement(e, 'pointerleave');
    expect(holds(e)).toBe(true);
    e = nextEngagement(e, 'focusout');
    expect(holds(e)).toBe(false);
  });

  it('starts idle', () => {
    expect(holds(IDLE)).toBe(false);
  });
});
