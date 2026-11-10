// The unreachable-library state (9b). Pulled once at startup (the check ran
// before the window existed) and updated by `library-unreachable` when a
// move's library root goes away while the app runs. While it says
// unreachable, App.svelte shows only the blocking dialog.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type LibraryRelocateOutcome, type LibraryUnreachableStatus } from '../api';
import { isBlocking, relocateError, relocateSuccessText } from '../libraryUnreachable';
import { pushToast } from './toasts.svelte';

const state = $state<{ status: LibraryUnreachableStatus | null }>({ status: null });

export const libraryUnreachable = {
  get status() {
    return state.status;
  },
  get blocking() {
    return isBlocking(state.status);
  },
};

export async function refreshLibraryUnreachable(): Promise<void> {
  try {
    state.status = await api.libraryUnreachableStatus();
  } catch {
    // Unknown is not unreachable: never block on a failed read.
  }
}

export async function initLibraryUnreachable(): Promise<UnlistenFn> {
  const unlisten = await listen<LibraryUnreachableStatus>('library-unreachable', (e) => {
    state.status = e.payload;
  });
  await refreshLibraryUnreachable();
  return unlisten;
}

/** What a relocate says once it is done; the desktop's card by default
 *  (TV mode shows its own words, `tv/startup/unreachable.ts`). */
export type RelocateNotice = (outcome: Extract<LibraryRelocateOutcome, { status: 'relocated' }>) => void;

function desktopNotice(outcome: Extract<LibraryRelocateOutcome, { status: 'relocated' }>): void {
  const text = relocateSuccessText(outcome);
  if (text !== null) pushToast(text, outcome.report.length > 0 ? 'error' : 'success');
}

/** Relocates the library to `path`. On success the dialog goes away. */
export async function relocateLibrary(path: string, notice: RelocateNotice = desktopNotice): Promise<LibraryRelocateOutcome> {
  const outcome = await api.libraryRelocate(path);
  if (outcome.status === 'relocated') {
    notice(outcome);
    await refreshLibraryUnreachable();
  }
  return outcome;
}

/**
 * The dialog's Relocate, for the desktop dialog and the TV screen alike:
 * relocates to the typed folder and answers the line to show under the
 * field (the refusal, or why the call failed), or `null` after a relocate.
 */
export async function relocateTo(typed: string, notice?: RelocateNotice): Promise<string | null> {
  try {
    return relocateError(await relocateLibrary(typed.trim(), notice));
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}

/** "Close without changing anything": the backend exits the app. */
export function closeWithoutChanges(): Promise<void> {
  return api.libraryUnreachableClose();
}
