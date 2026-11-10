// The folder browser (checklist items 349–352; tvmode.js `openBrowser`):
// one page for every path — Locate…, Change library folder, Relocate the
// library, Program, Connect's library folder. Pure: its rows from a
// `list_folder` answer, and the hints.
import type { FolderListing } from '../../api';
import type { Hint } from '../hints';

export type BrowserRow =
  /** "Up one folder", the first row anywhere but the root (item 350). */
  | { kind: 'up'; key: 'up'; path: string }
  | { kind: 'dir'; key: string; name: string; path: string }
  /** File mode (Program): a file, which A picks (item 352). */
  | { kind: 'file'; key: string; name: string; path: string };

/** `name` inside `dir`, with the folder's own separator. */
export function childPath(dir: string, name: string): string {
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;
}

/** The rows for a folder: Up one folder (absent at the root), then its
 *  folders and, in file mode, its files. */
export function browserRows(listing: FolderListing, files: boolean): BrowserRow[] {
  const rows: BrowserRow[] = [];
  if (listing.parent !== null) rows.push({ kind: 'up', key: 'up', path: listing.parent });
  for (const e of listing.entries) {
    if (!e.is_dir && !files) continue;
    rows.push({ kind: e.is_dir ? 'dir' : 'file', key: `fs-${e.name}`, name: e.name, path: childPath(listing.path, e.name) });
  }
  return rows;
}

/** A row's A hint (items 351–352). */
export function rowAction(row: BrowserRow): string {
  return row.kind === 'up' ? 'Up' : row.kind === 'dir' ? 'Open' : 'Choose';
}

/** The page's own hints: Y "Use this folder", none in file mode (item 352). */
export function browserHints(files: boolean): Hint[] {
  return files ? [] : [{ button: 'Y', label: 'Use this folder' }];
}
