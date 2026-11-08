// Design §8: "Segments Active (live), Queued, Completed (terminal,
// dismissable), each with a count". No legend line: the three section titles
// say enough (ProposedApp README, "Downloads"). The view
// renders all three stacked in this order (see the plan's "Deliberate
// deviations": a filter control would make rows vanish mid-transition and
// break every spec that waits on `download-detail-<id>`).
import type { DownloadEntry, DownloadStatus } from '../api';

export type Segment = 'active' | 'queued' | 'completed';

export const SEGMENTS: readonly Segment[] = ['active', 'queued', 'completed'];

export function segmentOf(status: DownloadStatus): Segment {
  switch (status) {
    case 'downloading':
    case 'installing':
    case 'cancelling':
      return 'active';
    case 'queued':
      return 'queued';
    default:
      return 'completed';
  }
}

export function segmentLabel(seg: Segment): string {
  switch (seg) {
    case 'active':
      return 'Active';
    case 'queued':
      return 'Queued';
    default:
      return 'Completed';
  }
}

export function segmentEmptyText(seg: Segment): string {
  switch (seg) {
    case 'active':
      return 'No active transfers';
    case 'queued':
      return 'Nothing waiting';
    default:
      return 'Nothing finished yet';
  }
}

/** Splits a snapshot (newest first) into the three segments, order kept. */
export function groupBySegment(entries: DownloadEntry[]): Record<Segment, DownloadEntry[]> {
  const groups: Record<Segment, DownloadEntry[]> = { active: [], queued: [], completed: [] };
  for (const e of entries) groups[segmentOf(e.status)].push(e);
  return groups;
}
