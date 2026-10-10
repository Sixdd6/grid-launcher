// The unreachable-library state (9b). Pulled once at startup (the check ran
// before the window existed) and updated by `library-unreachable` when a
// move's library root goes away while the app runs. While it says
// unreachable, App.svelte shows only the blocking dialog.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type LibraryRelocateOutcome, type LibraryUnreachableStatus } from '../api';
import { isBlocking, relocateSuccessText } from '../libraryUnreachable';
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

/** Relocates the library to `path`. On success the dialog goes away. */
export async function relocateLibrary(path: string): Promise<LibraryRelocateOutcome> {
  const outcome = await api.libraryRelocate(path);
  const text = relocateSuccessText(outcome);
  if (text !== null) {
    pushToast(text, outcome.status === 'relocated' && outcome.report.length > 0 ? 'error' : 'success');
    await refreshLibraryUnreachable();
  }
  return outcome;
}

/** "Close without changing anything": the backend exits the app. */
export function closeWithoutChanges(): Promise<void> {
  return api.libraryUnreachableClose();
}
