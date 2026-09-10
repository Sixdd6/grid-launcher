// SVG path strings for the transfer sparklines (design §8: "120×38: network
// in primary, disk in teal, 60 one-second samples"). Pure string maths so
// the component stays a template and vitest covers the geometry.
import { SAMPLE_COUNT, type Sample } from './ring';

export type SparklineBox = { width: number; height: number };

/** The scale both series share: the largest value seen, never below 1. */
export function sharedMax(samples: Sample[]): number {
  let max = 1;
  for (const s of samples) {
    if (s.net > max) max = s.net;
    if (s.disk > max) max = s.disk;
  }
  return max;
}

function fmt(n: number): string {
  return Number(n.toFixed(2)).toString();
}

/**
 * One series as `M x y L x y …`. The newest value sits on the right edge;
 * a partial buffer starts part-way across so the line grows leftwards as
 * samples arrive rather than stretching. One pixel of padding keeps a
 * stroke on 0 or `max` inside the box. A lone value becomes a zero-length
 * segment so `stroke-linecap: round` draws a dot.
 */
export function linePath(values: number[], capacity: number, box: SparklineBox, max: number): string {
  const n = values.length;
  if (n === 0) return '';
  const step = capacity > 1 ? box.width / (capacity - 1) : 0;
  const inner = box.height - 2;
  const parts: string[] = [];
  for (let i = 0; i < n; i += 1) {
    const x = (capacity - n + i) * step;
    const ratio = Math.min(1, Math.max(0, values[i] / max));
    const y = box.height - 1 - ratio * inner;
    parts.push(`${i === 0 ? 'M' : 'L'} ${fmt(x)} ${fmt(y)}`);
  }
  if (n === 1) parts.push(parts[0].replace(/^M/, 'L'));
  return parts.join(' ');
}

/** Both series, on one shared scale, ready for two `<path d>` attributes. */
export function sparklinePaths(
  samples: Sample[],
  box: SparklineBox,
  capacity: number = SAMPLE_COUNT,
): { net: string; disk: string; max: number } {
  const max = sharedMax(samples);
  return {
    net: linePath(samples.map((s) => s.net), capacity, box, max),
    disk: linePath(samples.map((s) => s.disk), capacity, box, max),
    max,
  };
}
