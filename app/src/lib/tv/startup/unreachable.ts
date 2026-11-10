// TV mode's unreachable-library dialog (checklist items 409–412; README
// § Startup screens › Unreachable library; tvmode.js `unreachable`). Pure:
// its words and the relocate's card.
import type { LibraryRelocateOutcome } from '../../api';

export const UNREACHABLE_TITLE = "Your library can't be reached";
export const RELOCATE_LABEL = 'Relocate the library…';
export const CLOSE_LABEL = 'Close without changing anything';
export const RELOCATE_BROWSER_TITLE = 'Relocate the library';

export function unreachableBody(root: string): string {
  return `${root} isn't there: its drive may not be connected. GRID can't run without it.`;
}

/** The card after a relocate (item 412): "Library relocated to <path>" /
 *  "No files were moved", plus the desktop's note when some stored paths
 *  could not be rewritten. */
export function relocatedNotice(outcome: Extract<LibraryRelocateOutcome, { status: 'relocated' }>): {
  title: string;
  sub: string;
  error: boolean;
} {
  const n = outcome.report.length;
  const failed = n === 0 ? '' : ` · ${n} ${n === 1 ? 'path' : 'paths'} could not be updated; see the log.`;
  return { title: `Library relocated to ${outcome.library_path}`, sub: `No files were moved${failed}`, error: n > 0 };
}
