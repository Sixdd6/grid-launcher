// Settings › Library in TV mode (checklist items 245–249; tvmode.js
// `paneRows('library')`, `changeLibrary`): the library folder and Change
// library folder…, which walks the desktop's flow (settings/
// libraryChange.svelte.ts) through the folder browser and lists: the new
// folder is checked, then "Move to <path>?" offers Move existing files or
// Start fresh, and Start fresh asks to leave or delete the old game files
// (Delete asks once more). Pure words here; TvSettings runs the flow.
import type { LibraryPathCheck, MovePreview, StartFreshOutcome, StartFreshPreview } from '../../../../api';
import { formatBytes, movePreviewSummary, moveItemLine, outcomeNotice, paneMoveText } from '../../../../settings/library';
import type { LibraryMoveStatus } from '../../../../api';
import type { Row } from '../../../rows';

export type LibraryState = { path: string | null; move: LibraryMoveStatus | null };

export const CHANGE_LIBRARY_TITLE = 'Choose the new library folder';
export const MOVE_BLOCKED = 'Unavailable while the library is being moved.';

export function libraryRows(s: LibraryState, change: () => void): Row[] {
  const moving = paneMoveText(s.move);
  return [
    { type: 'info', key: 'lib-path', label: 'Library folder', value: s.path === null ? '…' : s.path.trim() === '' ? 'Not set' : s.path },
    {
      type: 'action',
      key: 'lib-change',
      label: 'Change library folder…',
      sub: moving ?? 'Move your games, emulators and saves, or start fresh',
      ...(moving !== null ? { disabled: true, why: MOVE_BLOCKED } : s.path === null ? { disabled: true, why: 'Reading the library folder…' } : {}),
      run: change,
    },
  ];
}

/** Where the folder browser opens: the folder holding the current library. */
export function browserStart(path: string | null): string {
  const p = (path ?? '').trim().replace(/[\\/]+$/, '');
  if (p === '') return '';
  const cut = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return cut > 0 ? p.slice(0, cut) : p;
}

export type Refusal = { title: string; sub: string };

/** A picked folder the backend refused (item 246): the current library or a
 *  folder inside it get the prototype's words; other refusals keep the
 *  backend's reason. */
export function refusalNotice(check: LibraryPathCheck): Refusal | null {
  if (check.status === 'ok') return null;
  if (check.reason === 'same_as_current' || check.reason === 'inside_current') {
    return { title: "Can't use that folder", sub: 'It is the current library or inside it.' };
  }
  return { title: "Can't use that folder", sub: check.message };
}

export function moveListTitle(path: string): string {
  return `Move to ${path}?`;
}

/** The move list's line (item 247): each folder and how it moves, then the
 *  desktop's copy summary ("Copies 84 GB; 1.2 TB free"). A refused preview
 *  shows its reason. */
export function moveListSub(preview: MovePreview | null, refusal: string | null): string {
  if (refusal !== null) return refusal;
  if (preview === null) return '';
  const items = preview.items.map(moveItemLine).join(' · ');
  const summary = movePreviewSummary(preview);
  return items ? `${items}. ${summary}.` : `${summary}.`;
}

export const FRESH_TITLE = 'Delete the old game files, or leave them on disk?';

export function deleteLabel(preview: StartFreshPreview): string {
  return `Delete them (${formatBytes(preview.total_bytes)})`;
}

export function deleteWords(preview: StartFreshPreview, oldRoot: string): { title: string; body: string; ok: string } {
  return {
    title: `Delete ${formatBytes(preview.total_bytes)} of game files?`,
    body: `games/ in ${oldRoot} is removed. Emulators and saves stay. This can't be undone.`,
    ok: 'Delete',
  };
}

/** Move existing files started (item 248). "Library moved" follows from the
 *  move's own event (stores/libraryMove.svelte.ts). */
export function moveStartedNotice(newRoot: string): { title: string; sub: string } {
  return { title: `Moving the library to ${newRoot}…`, sub: "Games can't install or start until it's done" };
}

/** Start fresh finished: "Library is now <path>", with what happened to the
 *  old games (the desktop's words) as the second line; a refusal is an
 *  error with the backend's reason. */
export function freshNotice(outcome: StartFreshOutcome, deleted: boolean): { title: string; sub: string; error: boolean } {
  if (outcome.status !== 'switched') return { title: "Couldn't change the library folder", sub: outcome.message, error: true };
  const n = outcomeNotice(outcome, deleted);
  const rest = n.text.replace(/^Library folder changed\.\s*/, '');
  return { title: `Library is now ${outcome.library_path}`, sub: rest || (deleted ? 'Old game files deleted' : ''), error: n.kind === 'error' };
}
