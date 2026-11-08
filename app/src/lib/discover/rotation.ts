// Recently added's timer: every `periodMs` the highlight moves on. Driven
// from JavaScript, never from `animationend`, so it keeps running under
// reduced motion (where the bar's animation is off). Pausing keeps what is
// left of the period; resuming waits only that long.

export type Timers = {
  now: () => number;
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
};

const realTimers: Timers = {
  now: () => Date.now(),
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export type Rotation = {
  /** A full period from now (a new row was chosen). Keeps a pause. */
  restart: () => void;
  /** Holds the timer with what is left of the period. */
  pause: () => void;
  /** Runs the rest of the period. */
  resume: () => void;
  /** Stops for good (the section unmounted). */
  stop: () => void;
  /** Milliseconds left in the current period. */
  remaining: () => number;
  paused: () => boolean;
};

export function createRotation(periodMs: number, onAdvance: () => void, timers: Timers = realTimers): Rotation {
  let left = periodMs;
  let startedAt = 0;
  let handle: unknown = null;
  let isPaused = false;
  let stopped = false;

  function disarm() {
    if (handle !== null) timers.clear(handle);
    handle = null;
  }

  function arm() {
    disarm();
    if (isPaused || stopped) return;
    startedAt = timers.now();
    handle = timers.set(() => {
      handle = null;
      left = periodMs;
      onAdvance();
      // The caller usually restarts on advance; arm again if it did not.
      if (handle === null) arm();
    }, left);
  }

  return {
    restart() {
      left = periodMs;
      arm();
    },
    pause() {
      if (isPaused || stopped) return;
      if (handle !== null) left = Math.max(0, left - (timers.now() - startedAt));
      isPaused = true;
      disarm();
    },
    resume() {
      if (!isPaused || stopped) return;
      isPaused = false;
      arm();
    },
    stop() {
      stopped = true;
      disarm();
    },
    remaining() {
      if (isPaused || handle === null) return left;
      return Math.max(0, left - (timers.now() - startedAt));
    },
    paused: () => isPaused,
  };
}
