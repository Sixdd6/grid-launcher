// The unreachable-library dialog's pure half (9b). When the library folder
// cannot be reached at startup (or a move's library root goes away) and GRID
// has something stored there, the app must not run on it: the dialog blocks
// everything and offers exactly two ways out — relocate the library to the
// folder where it is now (stored paths are rewritten, no file moves), or
// close GRID without changing anything. A first run (no library folder set)
// is never "unreachable"; the Connect and First Run Setup flows handle it.

import type { LibraryRelocateOutcome, LibraryUnreachableStatus } from './api';

export const RELOCATE_PICKER_TITLE = 'Select the folder where your library is now';

/** The dialog blocks the app only for a library the backend reported unreachable. */
export function isBlocking(status: LibraryUnreachableStatus | null): boolean {
  return status !== null && status.status === 'unreachable';
}

export type UnreachableCopy = { title: string; reason: string; root: string; body: string };

/** What the dialog says; `null` when there is nothing to say. */
export function unreachableCopy(status: LibraryUnreachableStatus | null): UnreachableCopy | null {
  if (status === null || status.status !== 'unreachable') return null;
  return {
    title: 'GRID cannot reach your library',
    reason: status.message,
    root: status.root,
    body:
      'If you moved the library, relocate it to the folder where it is now: GRID updates its stored paths and moves no files. ' +
      'Otherwise connect the drive and start GRID again, or close GRID without changing anything.',
  };
}

/** Relocate with an empty folder field opens the folder picker first. */
export function relocateNeedsPicker(typed: string): boolean {
  return typed.trim() === '';
}

/** The refusal to show under the field; `null` after a relocate. */
export function relocateError(outcome: LibraryRelocateOutcome): string | null {
  return outcome.status === 'refused' ? outcome.message : null;
}

/** The toast after a relocate; `null` for a refusal. */
export function relocateSuccessText(outcome: LibraryRelocateOutcome): string | null {
  if (outcome.status !== 'relocated') return null;
  const done = `Library relocated to ${outcome.library_path}.`;
  const n = outcome.report.length;
  if (n === 0) return done;
  return `${done} ${n} ${n === 1 ? 'path' : 'paths'} could not be updated; see the log.`;
}
