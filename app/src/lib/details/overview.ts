// The Overview tab's description, as paragraphs, and the facts that came
// from the old Files tab (size, version, extra content). Pure, like
// `header.ts`.
import type { RomFile } from '../api';
import { contentRows, fileSizeText, gameRows } from './files';

/** The server's description split into paragraphs at its line breaks, with
 *  blank and whitespace-only lines dropped. A description with no breaks is
 *  one paragraph: the text is the server's, so it is never split by guess. */
export function descriptionParagraphs(description: string): string[] {
  return description
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '');
}

/** The game's own files' total size ("12.7 GB"), leaving out PS4 / Xbox 360
 *  update and DLC files; `''` when the server states no size. */
export function sizeFact(files: RomFile[]): string {
  const ids = new Set(gameRows(files).map((row) => row.id));
  const bytes = files.filter((f) => ids.has(f.id)).reduce((sum, f) => sum + Math.max(0, f.file_size_bytes), 0);
  return bytes > 0 ? fileSizeText(bytes) : '';
}

/**
 * The version fact. Installed: the installed version, with the server's
 * beside it when they differ ("v1.0.0 · server v1.1.0"). Not installed: the
 * server's. `''` when neither side is known.
 */
export function versionFact(installedNow: boolean, installed: string, server: string): string {
  if (!installedNow) return server;
  if (installed === '') return server === '' ? '' : `unknown · server ${server}`;
  if (server === '' || server === installed) return installed;
  return `${installed} · server ${server}`;
}

/** The update / DLC content the server holds ("1 update · 2 DLC"), or `''`
 *  when it holds none. `installedDlc` (a game installed by running its GOG
 *  installer: the DLC file names installed into it) adds how many of the
 *  DLC are installed: "3 DLC (1 installed)", "(all installed)",
 *  "(none installed)". */
export function contentFact(files: RomFile[], installedDlc: string[] | null = null): string {
  const rows = contentRows(files);
  const updates = rows.filter((row) => row.category === 'update').length;
  const dlcRows = rows.filter((row) => row.category === 'dlc');
  const dlc = dlcRows.length;
  const parts: string[] = [];
  if (updates > 0) parts.push(updates === 1 ? '1 update' : `${updates} updates`);
  if (dlc > 0) parts.push(installedDlc === null ? `${dlc} DLC` : `${dlc} DLC (${installedText(dlcRows, installedDlc)})`);
  return parts.join(' · ');
}

function installedText(dlcRows: { name: string }[], installedDlc: string[]): string {
  const installed = dlcRows.filter((row) => installedDlc.includes(row.name)).length;
  if (installed === 0) return 'none installed';
  if (installed === dlcRows.length) return 'all installed';
  return `${installed} installed`;
}
