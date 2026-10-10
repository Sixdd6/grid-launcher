// Settings › Library (Q2): the pure half of the Library pane and its change
// dialog. The dialog walks: choose (Move / Start fresh / Cancel) → for Start
// fresh, "Delete the old game files, or leave them on disk?" → for Delete,
// a confirmation with the folder list and the total size.

import type { LibraryPathCheck, StartFreshOutcome, StartFreshPreview } from '../api';

/**
 * "Move existing files" is phase 9b (a background job). Until it lands the
 * choice is shown disabled with a "coming soon" note. The 9b seam: flip this
 * and wire the button to the move command.
 */
export const MOVE_AVAILABLE = false;

export type ChangeStep = 'choose' | 'fresh-ask' | 'delete-confirm';

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** A byte count for people: binary steps, one decimal past bytes. */
export function formatBytes(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${value} B` : `${value.toFixed(1)} ${UNITS[unit]}`;
}

/** Start fresh is offered only once the backend accepted the typed path. */
export function canStartFresh(check: LibraryPathCheck | null, typed: string): boolean {
  return check !== null && check.status === 'ok' && typed.trim() !== '' && check.path === typed.trim();
}

function games(count: number): string {
  return `${count} ${count === 1 ? 'game' : 'games'}`;
}

/** The line above the delete confirmation's folder list. */
export function previewSummary(preview: StartFreshPreview): string {
  if (preview.games.length === 0) return 'No games';
  return `${games(preview.games.length)}, ${formatBytes(preview.total_bytes)}`;
}

export type Notice = { kind: 'success' | 'error'; text: string };

/** What the pane says after a Start fresh call. */
export function outcomeNotice(outcome: StartFreshOutcome, deleted: boolean): Notice {
  if (outcome.status !== 'switched') return { kind: 'error', text: outcome.message };
  const parts = ['Library folder changed.'];
  const n = outcome.rows_removed;
  if (n > 0) {
    parts.push(
      deleted
        ? `${games(n)} ${n === 1 ? 'was' : 'were'} deleted.`
        : `${games(n)} left the library; their files stay on disk.`,
    );
  }
  if (outcome.failures.length > 0) {
    const count = outcome.failures.length;
    parts.push(
      `${count} ${count === 1 ? 'file or folder' : 'files or folders'} could not be removed; those games stay in the library.`,
    );
    return { kind: 'error', text: parts.join(' ') };
  }
  return { kind: 'success', text: parts.join(' ') };
}
