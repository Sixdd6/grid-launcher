export type NavDirection = 'up' | 'down' | 'left' | 'right';

/** Spatial focus movement in a left-to-right grid: clamped, no wrap. */
export function moveFocus(index: number, action: NavDirection, columns: number, count: number): number {
  if (count <= 0) return 0;
  const row = Math.floor(index / columns);
  const col = index % columns;
  let next = index;
  if (action === 'left' && col > 0) next = index - 1;
  if (action === 'right' && col < columns - 1 && index + 1 < count) next = index + 1;
  if (action === 'up' && row > 0) next = index - columns;
  if (action === 'down') {
    const candidate = index + columns;
    if (candidate < count) next = candidate;
    else if (row < Math.floor((count - 1) / columns)) next = count - 1;
  }
  return Math.min(Math.max(next, 0), count - 1);
}
