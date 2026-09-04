// The Files tab's rows (design §7): `files[]` with name, size and the
// D-UI-10 version, plus the PS4 / Xbox 360 content rows. Pure.
import type { RomFile } from '../api';
import { fileVersionLabel } from './version';

const KIB = 1024;

/**
 * A file size for display. `0` is "the server did not state a size" — every
 * E2E fixture and plenty of real RomM rows send `0` for files it has not
 * measured — so it reads as an em dash rather than a confident "0 B".
 */
export function fileSizeText(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  if (bytes < KIB) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / KIB;
  let unit = 0;
  while (value >= KIB && unit < units.length - 1) {
    value /= KIB;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

export type FileRow = {
  id: number;
  name: string;
  sizeText: string;
  /** D-UI-10: the parsed tag, else the `last_modified` date, else `''`. */
  version: string;
  /** The server's file category, lowercased; `''` for an ordinary file. */
  category: string;
};

export function fileRows(files: RomFile[]): FileRow[] {
  return files.map((f) => ({
    id: f.id,
    name: f.file_name,
    sizeText: fileSizeText(f.file_size_bytes),
    version: fileVersionLabel(f.file_name, f.last_modified),
    category: f.category.trim().toLowerCase(),
  }));
}

/**
 * The PS4 update / Xbox 360 content files the server lists for this rom.
 * These are the same `category` values `content_availability` reads on the
 * backend (`app/src-tauri/src/commands/specials.rs`), so the rows and the
 * left column's Install Update / Install DLC buttons agree by construction.
 */
export function contentRows(files: RomFile[]): FileRow[] {
  return fileRows(files).filter((row) => row.category === 'update' || row.category === 'dlc');
}
