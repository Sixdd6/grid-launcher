// Locate files' report (RommStorefront README, "Report"; TVRommStorefront
// "The Locate report"): what the backend's `LocateReport` says, in the
// report's words. Pure, so both views read the same lines.
import type { LocateReport } from '../api';
import { shortName } from './rows';

/** The backend's report also counts the files in a picked folder that
 *  matched nothing (`unmatched`, sent beside the frozen TS type). */
export type LocateAnswer = LocateReport & { unmatched?: number };

/** How the files were picked. */
export type LocateMode = 'files' | 'folder' | 'pup';

export type ReportItem = {
  /** The picked file (or archive) name. */
  file: string;
  /** The name it was installed under, when it differs from `file`. */
  as: string | null;
  /** The archive it came from (a member found inside it). */
  from: string | null;
  note: string;
};

export type ReportSection = {
  kind: 'ok' | 'nv' | 'bad';
  /** "Installed · 2", "Installed · not verified · 1", "Not installed · 3". */
  title: string;
  items: ReportItem[];
};

export type ReportView = {
  title: string;
  lede: string;
  sections: ReportSection[];
  /** "1 other file in the folder matched nothing.", or `''`. */
  unmatched: string;
  /** Nothing was installed: offer Choose other files…. */
  nothing: boolean;
  /** The card a TV check ends with. */
  card: { title: string; text: string; error: boolean };
};

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Whether `name` is an archive Locate files looks inside. */
export function isArchiveName(name: string): boolean {
  return /\.(zip|7z|rar)$/i.test(name.trim());
}

const NOT_VERIFIED_NOTE =
  'GRID has no checksum for this file, so it can’t check it. If games don’t start, dump it from your console again.';

/** The report for one check. */
export function reportView(input: {
  system: string;
  emulator: string;
  mode: LocateMode;
  /** The folder the files came from (or the folder searched). */
  folder: string;
  /** How many files were picked (files and PUP mode). */
  picked: number;
  answer: LocateAnswer;
  /** A PUP check: RPCS3's installed version once it reports one. */
  pupVersion?: string | null;
}): ReportView {
  const { answer } = input;
  const emu = shortName(input.emulator);
  const pup = input.mode === 'pup';
  const ok: ReportItem[] = answer.installed.map((f) => {
    if (pup) {
      return {
        file: f.file,
        as: f.as_name !== f.file ? f.as_name : null,
        from: null,
        note: input.pupVersion
          ? `${emu} installed PS3 system software ${input.pupVersion}`
          : `${emu} is installing it. Finish in its window.`,
      };
    }
    if (isArchiveName(f.file) && f.as_name !== f.file) {
      return { file: f.as_name, as: null, from: f.file, note: 'Size and MD5 match' };
    }
    const renamed = f.as_name !== f.file;
    return {
      file: f.file,
      as: renamed ? f.as_name : null,
      from: null,
      note: renamed ? `Size and MD5 match; installed under the name ${emu} looks for` : 'Size and MD5 match',
    };
  });
  const nv: ReportItem[] = answer.unverified.map((f) => ({
    file: f.file,
    as: f.as_name !== f.file ? f.as_name : null,
    from: null,
    note: NOT_VERIFIED_NOTE,
  }));
  const bad: ReportItem[] = answer.refused.map((f) => ({ file: f.file, as: null, from: null, note: f.reason }));
  const sections: ReportSection[] = [];
  if (ok.length) sections.push({ kind: 'ok', title: `Installed · ${ok.length}`, items: ok });
  if (nv.length) sections.push({ kind: 'nv', title: `Installed · not verified · ${nv.length}`, items: nv });
  if (bad.length) sections.push({ kind: 'bad', title: `Not installed · ${bad.length}`, items: bad });
  const nothing = ok.length === 0 && nv.length === 0;
  const where =
    input.mode === 'folder'
      ? `Searched ${input.folder} and the folders in it.`
      : `${plural(input.picked, 'file')} from ${input.folder}.`;
  const done = nothing
    ? ' Nothing was installed.'
    : pup
      ? ` ${emu} installs it.`
      : ` Installed into ${emu}’s folders.`;
  const unmatched = answer.unmatched ?? 0;
  const installedCount = ok.length + nv.length;
  const card = nothing
    ? {
        title: 'No firmware installed',
        text: bad.length > 0 ? `${plural(bad.length, 'file')} refused` : `Nothing matched ${input.system} firmware`,
        error: true,
      }
    : {
        title: `${plural(installedCount, 'firmware file')} installed for ${emu}`,
        text: [nv.length ? `${nv.length} not verified` : '', bad.length ? `${bad.length} refused` : '']
          .filter((s) => s !== '')
          .join(' · '),
        error: false,
      };
  return {
    title: pup ? 'PS3 system software' : `${input.system} firmware`,
    lede: where + done,
    sections,
    unmatched: unmatched > 0 ? `${plural(unmatched, 'other file')} in the folder matched nothing.` : '',
    nothing,
    card,
  };
}

/** "Checking 5 files…". */
export function checkingTitle(count: number): string {
  return `Checking ${plural(count, 'file')}…`;
}

/** The spinner line while a check runs. */
export function checkingLine(emulator: string): string {
  return `Comparing sizes and MD5s with the files ${shortName(emulator)} needs`;
}

/** The folder a path is in (its parent), for the report's lede. */
export function folderOf(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i > 0 ? path.slice(0, i) : path;
}
