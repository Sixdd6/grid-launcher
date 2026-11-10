// The sticks (checklist items 35–37; tvmode.js `pollPads`, `stickScroll`).
// Pure. The Rust reader sends −1…1 with Y positive UP.
import type { TvButton } from './buttons';

/** The left stick moves focus once past half its travel on an axis. */
export const LEFT_THRESHOLD = 0.5;
/** The right stick scrolls past a quarter of its travel. */
export const RIGHT_DEAD_ZONE = 0.25;
/**
 * Scroll per 60 Hz frame at full tilt, in rem: the prototype's 22px per frame
 * at its 20px root, so the speed looks the same at 720p and 4K.
 */
export const RIGHT_REM_PER_FRAME = 1.1;
const FRAME_MS = 1000 / 60;

/** The directions the left stick holds. */
export function leftStickDirections(x: number, y: number): TvButton[] {
  const out: TvButton[] = [];
  if (x < -LEFT_THRESHOLD) out.push('LEFT');
  if (x > LEFT_THRESHOLD) out.push('RIGHT');
  if (y > LEFT_THRESHOLD) out.push('UP');
  if (y < -LEFT_THRESHOLD) out.push('DOWN');
  return out;
}

/** The right stick is tilted enough to scroll (and to count as a press for
 *  "the controller pressed last"). */
export function rightStickActive(y: number): boolean {
  return Math.abs(y) > RIGHT_DEAD_ZONE;
}

/**
 * Pixels to scroll down (negative: up) for `dtMs` of the right stick held at
 * `y`, proportional to the tilt. Tilting up scrolls up.
 */
export function rightStickScroll(y: number, dtMs: number, remPx: number): number {
  if (!rightStickActive(y)) return 0;
  return -y * RIGHT_REM_PER_FRAME * remPx * (dtMs / FRAME_MS);
}
