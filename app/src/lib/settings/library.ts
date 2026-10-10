// Settings › Library (Q2, 9b): the pure half of the Library pane and its change
// dialog. The dialog walks: choose (Move / Start fresh / Cancel) → for Move,
// the confirmation with the preview → for Start
// fresh, "Delete the old game files, or leave them on disk?" → for Delete,
// a confirmation with the folder list and the total size.

import type {
  LibraryMoveFinished,
  LibraryMoveStartOutcome,
  LibraryMoveStatus,
  LibraryPathCheck,
  MovePreview,
  MovePreviewItem,
  StartFreshOutcome,
  StartFreshPreview,
} from '../api';
import type { ToastLevel } from '../stores/toasts.svelte';

// Phase 9b adds "Move existing files": choose → move-confirm (the preview:
// what moves, same drive or copied, the space) → a background job whose
// Downloads row shows the progress.
export type ChangeStep = 'choose' | 'fresh-ask' | 'delete-confirm' | 'move-confirm';

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

// --- Move existing files (9b) -------------------------------------------------

/** Move needs an accepted new folder and a current library to move. */
export function canMove(check: LibraryPathCheck | null, typed: string, currentPath: string): boolean {
  return canStartFresh(check, typed) && currentPath.trim() !== '';
}

/**
 * The confirmation's one-line answer to "how long, how much space": a move
 * that stays on one drive is a rename per folder; anything on another drive
 * is copied and checked first.
 */
export function movePreviewSummary(preview: MovePreview): string {
  if (preview.items.length === 0) return 'Nothing to move';
  if (preview.copy_bytes === 0) return 'Same drive — instant';
  const free = preview.free_bytes === null ? 'free space unknown' : `${formatBytes(preview.free_bytes)} free`;
  return `Copies ${formatBytes(preview.copy_bytes)}; ${free}`;
}

/** One folder in the confirmation's list. */
export function moveItemLine(item: MovePreviewItem): string {
  const how = item.strategy === 'rename' ? 'same drive' : 'copied and checked';
  return `${item.name}/ · ${formatBytes(item.bytes)} · ${how}`;
}

/** A move runs or waits (to resume, or after it stopped). */
export function moveInProgress(status: LibraryMoveStatus | null): boolean {
  return status !== null && status.phase !== null;
}

/** The Library pane's line while a move is in progress; `null` otherwise. */
export function paneMoveText(status: LibraryMoveStatus | null): string | null {
  if (status === null || status.phase === null) return null;
  if (status.running) return `Moving to ${status.new_root}…`;
  if (status.error !== null) {
    const next = status.phase === 'copy' ? 'Retry or cancel it in Downloads.' : 'Retry it in Downloads.';
    return `The move to ${status.new_root} stopped: ${status.error} ${next}`;
  }
  return `The move to ${status.new_root} waits to resume.`;
}

/** What the pane says after Start move. */
export function moveStartNotice(outcome: LibraryMoveStartOutcome): Notice {
  if (outcome.status === 'started') {
    return { kind: 'success', text: `Moving the library to ${outcome.new_root}. Follow it in Downloads.` };
  }
  return { kind: 'error', text: outcome.message };
}

/** The toast for `library-move-finished`. Report lines are warnings. */
export function moveFinishedToast(finished: LibraryMoveFinished): { level: ToastLevel; text: string } {
  switch (finished.status) {
    case 'finished': {
      const moved = `Library moved to ${finished.new_root}.`;
      if (finished.report.length === 0) return { level: 'success', text: moved };
      return { level: 'error', text: [moved, ...finished.report].join(' ') };
    }
    case 'cancelled':
      return { level: 'success', text: 'Library move cancelled. Nothing changed.' };
    case 'failed':
      return { level: 'error', text: `Library move stopped: ${finished.error}` };
  }
}

/** The startup toast for a move resumed in the background; `null` otherwise. */
export function resumeToast(status: LibraryMoveStatus | null): string | null {
  if (status === null || status.phase === null || !status.resumed) return null;
  return `Resuming the library move to ${status.new_root}`;
}
