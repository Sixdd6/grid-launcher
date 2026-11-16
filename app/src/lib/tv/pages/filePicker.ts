// The TV file browser in file mode (TVRommStorefront README, "The TV file
// browser in file mode (Locate files)") and the Locate report page ("The
// Locate report"). Pure: the rows, the picks line, the hints and the
// report's words. TvFilePicker and TvLocateReport draw them.
import type { FolderListing } from '../../api';
import type { LocateAnswer } from '../../systems/locate';
import { isArchiveName } from '../../systems/locate';
import { shortName } from '../../systems/rows';
import type { Hint } from '../hints';
import { childPath } from './folderBrowser';
import { sizeText } from './firmwareRows';

export type PickRow =
  | { kind: 'up'; key: 'up'; path: string }
  | { kind: 'dir'; key: string; name: string; path: string }
  | { kind: 'file'; key: string; name: string; path: string; size: string };

/** Up one folder (not at a root), the folders, then the files. */
export function pickRows(listing: FolderListing): PickRow[] {
  const rows: PickRow[] = [];
  if (listing.parent !== null) rows.push({ kind: 'up', key: 'up', path: listing.parent });
  for (const e of listing.entries) {
    const path = childPath(listing.path, e.name);
    rows.push(
      e.is_dir
        ? { kind: 'dir', key: `d-${e.name}`, name: e.name, path }
        : { kind: 'file', key: `f-${e.name}`, name: e.name, path, size: pickSize(e.size) },
    );
  }
  return rows;
}

/** A file's size on its row: "512 KB", "197 MB", "4.4 GB"; `''` for an empty file. */
export function pickSize(bytes: number): string {
  const gib = 1024 * 1024 * 1024;
  if (bytes < gib) return sizeText(bytes);
  const gb = bytes / gib;
  return `${gb < 10 ? Math.round(gb * 10) / 10 : Math.round(gb)} GB`;
}

/** A on a row: Up, Open, Pick / Unpick (multi), Choose (single). */
export function pickRowAction(row: PickRow, picked: boolean, single: boolean): string {
  if (row.kind === 'up') return 'Up';
  if (row.kind === 'dir') return 'Open';
  if (single) return 'Choose';
  return picked ? 'Unpick' : 'Pick';
}

/** The base name of a path. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).filter((p) => p !== '').pop() ?? path;
}

/** The line under the path: what is picked so far (kept across folders). */
export function picksLine(picked: readonly string[]): string {
  if (picked.length === 0) return 'No files picked. Pick files with (A), or check this whole folder with (Y).';
  const names = picked.map(baseName).join(', ');
  return `${picked.length === 1 ? '1 file' : `${picked.length} files`} picked: ${names}`;
}

/** The page's own hints: Y checks the folder; Start checks the picks once
 *  something is picked. Single mode has neither. */
export function pickerHints(picked: number, single: boolean): Hint[] {
  if (single) return [];
  const hints: Hint[] = [{ button: 'Y', label: 'Check this folder' }];
  if (picked > 0) hints.push({ button: 'MENU', label: `Check ${picked === 1 ? '1 file' : `${picked} files`}` });
  return hints;
}

/** What Start says with nothing picked. */
export const NOTHING_PICKED = 'Pick files first, or press Y to check this folder.';

/** The page's sub line. */
export function pickerSub(emulator: string, single: boolean): string {
  const emu = shortName(emulator);
  return single
    ? `For ${emu}. GRID checks the update file, then ${emu} installs it.`
    : `For ${emu}. GRID checks each file and puts the ones it knows in ${emu}’s folders.`;
}

// --- The Locate report page ------------------------------------------------

export type TvReportItem = { key: string; label: string; sub: string; pill: string };
export type TvReportGroup = { kind: 'ok' | 'nv' | 'bad'; title: string; items: TvReportItem[] };

export type TvReport = {
  title: string;
  sub: string;
  groups: TvReportGroup[];
  /** "10 other files matched nothing GRID knows for Nintendo Switch.", or ''. */
  unmatched: string;
  /** The warning note when nothing was installed, or ''. */
  nothing: string;
  card: { title: string; text: string; error: boolean };
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** The report page for one check. */
export function tvReport(input: {
  system: string;
  emulator: string;
  folder: boolean;
  /** The folder searched, or the folder the files came from. */
  where: string;
  picked: number;
  answer: LocateAnswer;
}): TvReport {
  const { answer, system } = input;
  const emu = shortName(input.emulator);
  const ok: TvReportItem[] = answer.installed.map((f, i) => ({
    key: `ok-${i}`,
    label: f.as_name,
    sub:
      f.as_name === f.file && !isArchiveName(f.file)
        ? 'Size and MD5 match'
        : `From ${f.file} · size and MD5 match`,
    pill: 'Installed',
  }));
  const nv: TvReportItem[] = answer.unverified.map((f, i) => ({
    key: `nv-${i}`,
    label: f.as_name,
    sub: 'GRID can’t check this file: it’s different on every console',
    pill: 'Installed · not verified',
  }));
  const bad: TvReportItem[] = answer.refused.map((f, i) => ({ key: `bad-${i}`, label: f.file, sub: f.reason, pill: 'Refused' }));
  const groups: TvReportGroup[] = [];
  if (ok.length) groups.push({ kind: 'ok', title: `Installed · ${ok.length}`, items: ok });
  if (nv.length) groups.push({ kind: 'nv', title: `Installed · not verified · ${nv.length}`, items: nv });
  if (bad.length) groups.push({ kind: 'bad', title: `Refused · ${bad.length}`, items: bad });
  const total = ok.length + nv.length;
  const unmatched = answer.unmatched;
  const checked = input.folder
    ? `Checked ${input.where} for ${system} · ${emu}`
    : `Checked ${plural(input.picked, 'file')} you picked for ${system} · ${emu}`;
  return {
    title: 'Firmware check',
    sub: checked,
    groups,
    unmatched: unmatched > 0 ? `${plural(unmatched, 'other file')} matched nothing GRID knows for ${system}.` : '',
    nothing: total === 0 ? `Nothing was installed. The ${system} page lists the files ${emu} needs.` : '',
    card:
      total === 0
        ? {
            title: 'No firmware installed',
            text: bad.length > 0 ? `${plural(bad.length, 'file')} refused` : `Nothing matched ${system} firmware`,
            error: true,
          }
        : {
            title: `${plural(total, 'firmware file')} installed for ${emu}`,
            text: [nv.length ? `${nv.length} not verified` : '', bad.length ? `${bad.length} refused` : ''].filter(Boolean).join(' · '),
            error: false,
          },
  };
}
