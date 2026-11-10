// The Downloads rows' actions, shared by both shells: Cancel, Retry and
// Dismiss, where a library move row's Cancel and Retry are the move's own
// commands (9b). Each throws the refusal as an `Error` with the backend's
// message, for the screen to show under the row.
import { api, type DownloadEntry } from '../api';

/** Cancels a library move. A failed move row that was undone makes way. */
export async function cancelLibraryMove(e: DownloadEntry): Promise<void> {
  const outcome = await api.libraryMoveCancel();
  if (outcome.status === 'not_cancellable' || outcome.status === 'no_move') throw new Error(outcome.message);
  if (outcome.status === 'cancelled' && e.status === 'failed') await api.dismissDownload(e.id);
}

/** Resumes a library move. A failed move row that was retried makes way:
 *  the retry runs on a new row. */
export async function resumeLibraryMove(e: DownloadEntry): Promise<void> {
  const outcome = await api.libraryMoveResume();
  if (outcome.status === 'refused') throw new Error(outcome.message);
  if (e.status === 'failed') await api.dismissDownload(e.id);
}

/** A row's Cancel. */
export function cancelEntry(e: DownloadEntry): Promise<void> {
  return e.kind === 'library_move' ? cancelLibraryMove(e) : api.cancelInstall(e.id);
}

/** A row's Retry. */
export function retryEntry(e: DownloadEntry): Promise<void> {
  return e.kind === 'library_move' ? resumeLibraryMove(e) : api.retryInstall(e.id);
}

/** A finished row's Dismiss. */
export function dismissEntry(id: number): Promise<void> {
  return api.dismissDownload(id);
}

/**
 * Runs one row's action while tracking its pending flag and its error: the
 * error is cleared when the action starts and set from its refusal. The
 * caller owns the two maps (a view's state), so each screen can show them
 * its own way.
 */
export async function runRowAction(
  id: number,
  action: () => Promise<void>,
  track: {
    clearError: (id: number) => void;
    setError: (id: number, message: string) => void;
    setPending: (id: number, pending: boolean) => void;
  },
): Promise<void> {
  track.clearError(id);
  track.setPending(id, true);
  try {
    await action();
  } catch (err) {
    track.setError(id, err instanceof Error ? err.message : String(err));
  } finally {
    track.setPending(id, false);
  }
}
