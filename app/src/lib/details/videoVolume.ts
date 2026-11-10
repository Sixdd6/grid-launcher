// The video players' own volume (user decisions 2026-11-10): a percentage,
// 0–100, separate from the system volume. Every player in the media viewer —
// the YouTube relay and a server video, desktop and TV mode — opens at the
// stored `[ui] video_volume` (default 50). LT / RT in TV mode step it by 10;
// a change is stored about 500 ms after the last one.

export const VOLUME_DEFAULT = 50;
export const VOLUME_STEP = 10;
export const VOLUME_SAVE_MS = 500;
/** How long TV mode's "Volume N%" readout stays after the last change. */
export const VOLUME_READOUT_MS = 1500;

/** A whole percentage in 0–100; anything that is not a finite number is the
 *  default. */
export function clampVolume(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return VOLUME_DEFAULT;
  return Math.max(0, Math.min(100, Math.round(value)));
}

/** One LT (-1) or RT (+1) step: 10 points, stopping at 0 and 100. */
export function stepVolume(value: number, dir: -1 | 1): number {
  return clampVolume(clampVolume(value) + dir * VOLUME_STEP);
}

/** HTMLMediaElement.volume (0–1) ⇄ the stored percentage. */
export function toElementVolume(percent: number): number {
  return clampVolume(percent) / 100;
}

export function fromElementVolume(volume: number): number {
  return clampVolume(volume * 100);
}

/** The readout beside the player. */
export function volumeLabel(percent: number): string {
  return `Volume ${clampVolume(percent)}%`;
}

export type Timers = {
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
};

const realTimers: Timers = {
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
};

/** `fn`, run once `ms` after the last call. */
export function debounced(fn: () => void, ms: number, timers: Timers = realTimers): () => void {
  let handle: unknown = null;
  return () => {
    if (handle !== null) timers.clear(handle);
    handle = timers.set(() => {
      handle = null;
      fn();
    }, ms);
  };
}
