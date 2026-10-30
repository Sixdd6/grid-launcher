// The shape of a grid's cells, from the covers it shows (look-and-feel plan
// 2026-10-30, follow-up). Pure: `coverShapes.svelte.ts` records what loads,
// this module turns the samples into one width-over-height ratio.
//
// A uniform 3:4 cell left a tall empty band above every row of square
// PlayStation covers. Each grid now takes the median shape of its covers,
// so a platform of square covers gets square cells and a platform of tall
// Switch covers gets tall ones. Rows stay uniform, which the windowed grid
// needs (`window.ts`): one ratio per grid, never per card.
import { CARD_CELL_ASPECT } from './size';

/** Covers measured before the grid trusts the median. Until then, 3:4. */
export const MIN_SAMPLES = 6;
/** The most covers kept per grid: enough for a stable median. */
export const MAX_SAMPLES = 40;

/** The tallest cell (Switch boxes are about 0.62). */
export const MIN_CELL_ASPECT = 0.6;
/**
 * The widest cell: square. A wider cell would be too short for the hover
 * bar to clear the card root's centre on the Small size (`size.test.ts`), and
 * wide art fits a square cell with little room lost.
 */
export const MAX_CELL_ASPECT = 1;

/** A median this close to 3:4 stays 3:4, so near-portrait platforms do not
 *  shift by a few pixels. */
const SNAP = 0.05;

/** One grid's cell ratio (width over height) from its covers' ratios. */
export function cellAspect(samples: readonly number[]): number {
  const usable = samples.filter((r) => Number.isFinite(r) && r > 0);
  if (usable.length < MIN_SAMPLES) return CARD_CELL_ASPECT;
  const sorted = [...usable].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  const median = sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
  if (Math.abs(median - CARD_CELL_ASPECT) < SNAP) return CARD_CELL_ASPECT;
  const clamped = Math.min(MAX_CELL_ASPECT, Math.max(MIN_CELL_ASPECT, median));
  return Math.round(clamped * 100) / 100;
}
