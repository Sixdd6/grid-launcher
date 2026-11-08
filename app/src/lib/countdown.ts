// The auto-dismiss countdown every notification card carries (Toast.svelte's
// cards and the setup card). Timing is JS-driven, never `animationend`: under
// reduced motion the countdown bar does not animate, but the card must still
// close itself.
//
// The `Countdown` value and its functions are pure, for vitest; the
// `CountdownTimer` class under them arms one `setTimeout` at a time and
// re-arms it with whatever is left on resume.

export type Countdown = {
  /** Time left when the countdown was last paused or started. */
  remainingMs: number;
  /** When it last started running; `null` while paused. */
  startedAt: number | null;
};

export function startCountdown(durationMs: number, now: number): Countdown {
  return { remainingMs: Math.max(0, durationMs), startedAt: now };
}

/** Time left at `now`, never below zero. */
export function remainingMs(c: Countdown, now: number): number {
  if (c.startedAt === null) return c.remainingMs;
  return Math.max(0, c.remainingMs - (now - c.startedAt));
}

/** Freezes the countdown at what is left. A no-op when already paused. */
export function pauseCountdown(c: Countdown, now: number): Countdown {
  if (c.startedAt === null) return c;
  return { remainingMs: remainingMs(c, now), startedAt: null };
}

/** Restarts the countdown from what was left. A no-op when already running. */
export function resumeCountdown(c: Countdown, now: number): Countdown {
  if (c.startedAt !== null) return c;
  return { remainingMs: c.remainingMs, startedAt: now };
}

/**
 * Calls `onDone` once `durationMs` of unpaused time has passed. `pause` and
 * `resume` are idempotent; `stop` cancels it for good (a card dismissed by
 * hand, or dropped from a full stack).
 */
export class CountdownTimer {
  #state: Countdown;
  #handle: ReturnType<typeof setTimeout> | null = null;
  #stopped = false;
  readonly #onDone: () => void;

  constructor(durationMs: number, onDone: () => void) {
    this.#onDone = onDone;
    this.#state = startCountdown(durationMs, Date.now());
    this.#arm();
  }

  get paused(): boolean {
    return this.#state.startedAt === null;
  }

  get remainingMs(): number {
    return remainingMs(this.#state, Date.now());
  }

  pause(): void {
    if (this.#stopped || this.paused) return;
    this.#state = pauseCountdown(this.#state, Date.now());
    this.#clear();
  }

  resume(): void {
    if (this.#stopped || !this.paused) return;
    this.#state = resumeCountdown(this.#state, Date.now());
    this.#arm();
  }

  stop(): void {
    this.#stopped = true;
    this.#clear();
  }

  #arm(): void {
    this.#clear();
    this.#handle = setTimeout(() => {
      this.#handle = null;
      this.#stopped = true;
      this.#onDone();
    }, this.#state.remainingMs);
  }

  #clear(): void {
    if (this.#handle !== null) clearTimeout(this.#handle);
    this.#handle = null;
  }
}

/** What holds a card's countdown: the pointer over it, or focus inside it. */
export type Engagement = { hovered: boolean; focused: boolean };

export type EngagementEvent = 'pointerenter' | 'pointerleave' | 'focusin' | 'focusout';

export const IDLE: Engagement = { hovered: false, focused: false };

export function nextEngagement(e: Engagement, event: EngagementEvent): Engagement {
  switch (event) {
    case 'pointerenter':
      return { ...e, hovered: true };
    case 'pointerleave':
      return { ...e, hovered: false };
    case 'focusin':
      return { ...e, focused: true };
    case 'focusout':
      return { ...e, focused: false };
  }
}

export function holds(e: Engagement): boolean {
  return e.hovered || e.focused;
}

/**
 * Svelte action for a notification card: reports `true` while the pointer is
 * over the card or focus is inside it, `false` once neither is. A `focusout`
 * to another element inside the card is ignored, so moving focus between the
 * card's buttons never lets the countdown run.
 */
export function holdWhileEngaged(node: HTMLElement, onHold: (held: boolean) => void) {
  let state = IDLE;
  let callback = onHold;
  const handle = (event: Event) => {
    const before = holds(state);
    if (event.type === 'focusout') {
      const next = (event as FocusEvent).relatedTarget;
      if (next instanceof Node && node.contains(next)) return;
    }
    state = nextEngagement(state, event.type as EngagementEvent);
    const after = holds(state);
    if (after !== before) callback(after);
  };
  const events: EngagementEvent[] = ['pointerenter', 'pointerleave', 'focusin', 'focusout'];
  for (const name of events) node.addEventListener(name, handle);
  return {
    update(next: (held: boolean) => void) {
      callback = next;
    },
    destroy() {
      for (const name of events) node.removeEventListener(name, handle);
    },
  };
}
