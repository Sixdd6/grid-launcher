// The alert after an uninstall that left folders on disk (G9 follow-up).
// The removal guard refuses some paths a record names — a platform folder, the
// library folder, home — and the uninstall skips them. The user sees each one
// with its reason. Used by Details uninstall, emulator delete, and Settings ›
// Library › Start fresh › Delete.

import type { SkippedRemoval } from './api';

/** Long enough to read a few paths; the default toast is 4 s. */
export const SKIPPED_TOAST_MS = 12000;

/**
 * The alert text, or `''` when nothing was skipped (a blank toast is never
 * shown). One line: the toast does not keep line breaks.
 */
export function skippedRemovalText(skipped: readonly SkippedRemoval[] | null | undefined): string {
  if (!skipped || skipped.length === 0) return '';
  const count = skipped.length;
  const head = `${count} ${count === 1 ? 'folder was' : 'folders were'} kept on disk for safety:`;
  const items = skipped.map((skip) => `${skip.path} (${skip.reason})`).join('; ');
  return `${head} ${items}.`;
}
