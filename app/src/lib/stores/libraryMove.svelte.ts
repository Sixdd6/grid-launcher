// The library move's state (9b): what `library_move_status` says, kept
// current by the backend's `library-move-changed` event. Module-scoped so the
// Library pane, the Downloads row and the startup resume toast share one
// copy. The pure wording lives in `settings/library.ts`.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type LibraryMoveFinished, type LibraryMoveStatus } from '../api';
import { moveFinishedToast, resumeToast } from '../settings/library';
import { pushToast } from './toasts.svelte';

/** A move's outcome (and its report lines) needs longer than a routine toast. */
export const MOVE_TOAST_DURATION_MS = 12000;

const state = $state<{ status: LibraryMoveStatus | null; pathVersion: number }>({
  status: null,
  pathVersion: 0,
});

export const libraryMove = {
  get status() {
    return state.status;
  },
  /** The journal's phase, `null` when no move is in progress. */
  get phase() {
    return state.status?.phase ?? null;
  },
  /** Bumped on `library-path-changed`: the pane re-reads the folder. */
  get pathVersion() {
    return state.pathVersion;
  },
};

let resumeShown = false;

function apply(status: LibraryMoveStatus): void {
  state.status = status;
  const text = resumeShown ? null : resumeToast(status);
  if (text !== null) {
    resumeShown = true;
    pushToast(text, 'success', MOVE_TOAST_DURATION_MS);
  }
}

/** Re-reads `library_move_status`. A failed read keeps the last state. */
export async function refreshLibraryMove(): Promise<void> {
  try {
    apply(await api.libraryMoveStatus());
  } catch {
    // The next `library-move-changed` event brings it.
  }
}

/** Pulls the status once and follows the three move events. */
export async function initLibraryMove(): Promise<UnlistenFn> {
  const unlisteners = await Promise.all([
    listen<LibraryMoveStatus>('library-move-changed', (e) => apply(e.payload)),
    listen<LibraryMoveFinished>('library-move-finished', (e) => {
      const toast = moveFinishedToast(e.payload);
      pushToast(toast.text, toast.level, MOVE_TOAST_DURATION_MS);
    }),
    listen('library-path-changed', () => {
      state.pathVersion += 1;
    }),
  ]);
  await refreshLibraryMove();
  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}
