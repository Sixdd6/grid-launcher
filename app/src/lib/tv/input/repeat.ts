// Held buttons and their repeat, per controller (checklist items 40–41,
// 44–45; tvmode.js `pollPads`). Pure apart from the injected timer, so the
// tests drive it with a fake clock.
//
// A press fires at once. While it is held, a direction repeats after 400 ms,
// then every 90 ms. The on-screen keyboard's typing buttons repeat after
// 500 ms, then every 50 ms (P6 plugs that in through `kind`). Every other
// button fires once. Releasing ends the repeat at once. Each source (a
// controller id, or the keyboard) keeps its own held state, so one never
// cancels or re-fires another's button.
import type { TvButton } from './buttons';

/** How a held button repeats right now: `dir` (D-pad, left stick, arrows),
 *  `typing` (P6's keyboard) or not at all. Asked at the press and again at
 *  each repeat, as the prototype re-checks every frame. */
export type RepeatKind = 'dir' | 'typing' | null;

export const DIR_DELAY_MS = 400;
export const DIR_INTERVAL_MS = 90;
export const TYPING_DELAY_MS = 500;
export const TYPING_INTERVAL_MS = 50;

export type Timers = {
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
};

const realTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
};

export class Repeater<S extends string | number = string | number> {
  private held = new Map<S, Map<TvButton, unknown>>();

  constructor(
    private readonly fire: (button: TvButton, source: S) => void,
    private readonly kind: (button: TvButton) => RepeatKind,
    private readonly timers: Timers = realTimers,
  ) {}

  /** A button went down. A button already held by this source is ignored. */
  down(source: S, button: TvButton): void {
    let mine = this.held.get(source);
    if (!mine) {
      mine = new Map();
      this.held.set(source, mine);
    }
    if (mine.has(button)) return;
    const first = this.kind(button) === 'typing' ? TYPING_DELAY_MS : DIR_DELAY_MS;
    mine.set(button, null);
    this.fire(button, source);
    // The press may have released it (a handler that tears the source down).
    if (!this.held.get(source)?.has(button)) return;
    mine.set(button, this.timers.set(() => this.tick(source, button), first));
  }

  private tick(source: S, button: TvButton): void {
    const mine = this.held.get(source);
    if (!mine?.has(button)) return;
    const k = this.kind(button);
    if (k === null) {
      // Held, but not a repeating button (now): keep it held, fire nothing.
      mine.set(button, null);
      return;
    }
    this.fire(button, source);
    if (!this.held.get(source)?.has(button)) return;
    mine.set(button, this.timers.set(() => this.tick(source, button), k === 'typing' ? TYPING_INTERVAL_MS : DIR_INTERVAL_MS));
  }

  /** A button went up: its repeat ends at once. */
  up(source: S, button: TvButton): void {
    const mine = this.held.get(source);
    if (!mine?.has(button)) return;
    const h = mine.get(button);
    if (h !== null && h !== undefined) this.timers.clear(h);
    mine.delete(button);
  }

  /** Every button of one source is up (a controller disconnected, the
   *  keyboard lost the window). */
  releaseSource(source: S): void {
    const mine = this.held.get(source);
    if (!mine) return;
    for (const b of [...mine.keys()]) this.up(source, b);
    this.held.delete(source);
  }

  /** Everything is up (the Rust gate closed: `released_all`). */
  releaseAll(): void {
    for (const s of [...this.held.keys()]) this.releaseSource(s);
  }

  isHeld(source: S, button: TvButton): boolean {
    return this.held.get(source)?.has(button) ?? false;
  }
}

/** Which physical causes hold each logical button of one source: a D-pad
 *  direction and the left stick both hold UP, and UP is up only when both
 *  are released. `add` answers true when the button just went down, `remove`
 *  when it just went up. */
export class Causes {
  private map = new Map<TvButton, Set<string>>();

  add(button: TvButton, cause: string): boolean {
    let set = this.map.get(button);
    if (!set) {
      set = new Set();
      this.map.set(button, set);
    }
    const was = set.size;
    set.add(cause);
    return was === 0 && set.size === 1;
  }

  remove(button: TvButton, cause: string): boolean {
    const set = this.map.get(button);
    if (!set || !set.delete(cause)) return false;
    return set.size === 0;
  }

  /** Every cause of one kind (a stick), with the buttons that went up. */
  removeCause(cause: string): TvButton[] {
    const up: TvButton[] = [];
    for (const [b, set] of this.map) if (set.delete(cause) && set.size === 0) up.push(b);
    return up;
  }

  /** The physical button `cause` is holding, if any. */
  heldBy(cause: string): TvButton | null {
    for (const [b, set] of this.map) if (set.has(cause)) return b;
    return null;
  }

  clear(): void {
    this.map.clear();
  }
}
