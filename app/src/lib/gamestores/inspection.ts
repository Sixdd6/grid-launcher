// The offline GOG installer's card after `offline_inspect` (GameStores
// README, "Offline GOG installer"; prototype `stFoundHTML`): a recognised
// installer (Linux, Windows, a newer version, DLC) or a refusal. Pure.
import type { InstallerInspection } from '../api';
import { sizeGb } from './text';

export const PICKER_TITLE = 'Choose a GOG installer';
export const PICKER_LEDE =
  'Pick a GOG offline installer you downloaded: setup_….exe (Windows) or ….sh (Linux).';
export const PICKER_FOOT = 'Multi-part installers: pick the .exe; its .bin parts must sit beside it.';
export const REFUSED_HEADING = 'Can’t install this file';

/** "Looking for the game’s goggame info in <file>". */
export function readingLine(path: string): string {
  return `Looking for the game’s goggame info in ${fileName(path)}`;
}

/** The last part of a path, either separator. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export type RefusedView = { kind: 'refused'; heading: string; title: string; body: string };

export type FoundView = {
  kind: 'found';
  /** "Install Paper Lanterns", "Update Ashen Tide to 1.0.5", "Install DLC for Ashen Tide". */
  heading: string;
  title: string;
  /** "Linux version · v1.1.0 · 1.1 GB" / "Digital Artbook · DLC for Ashen Tide · 118 MB". */
  what: string;
  /** "From a GOG offline installer · paper_lanterns_1_1_0_60321.sh". */
  from: string;
  /** How it installs. */
  how: string;
  /** The update's warning line, or `''`. */
  already: string;
  /** The compatibility-tool picker (a Windows game installer). */
  showCompat: boolean;
  /** The install folder row (a new game, not an update or DLC). */
  showFolder: boolean;
  /** "It joins your GOG games…" for a new game; `''` otherwise. */
  joins: string;
  /** Install / Update / Install DLC. */
  button: string;
};

export type InspectionView = RefusedView | FoundView;

/**
 * The card for `insp`. `compat` is the compatibility tool picked (the
 * Settings › Systems default); `gogConnected` words where updates come from.
 */
export function inspectionView(insp: InstallerInspection, opts: { compat: string; gogConnected: boolean }): InspectionView {
  const v = insp.verdict;
  if (v.kind === 'refused') return { kind: 'refused', heading: REFUSED_HEADING, title: v.title, body: v.body };
  const windows = insp.kind === 'windows_exe';
  const file = fileName(insp.path);
  const version = insp.version.trim();
  const size = sizeGb(insp.bytes);
  const from = `From a GOG offline installer · ${file}`;
  if (v.kind === 'dlc') {
    return {
      kind: 'found',
      heading: `Install DLC for ${v.base_title}`,
      title: v.base_title,
      what: `${dlcName(insp.title, v.base_title)} · DLC for ${v.base_title} · ${size}`,
      from,
      how: windows
        ? `Adds to the installed ${v.base_title} (the Windows version). GRID runs the installer silently in the game’s Wine prefix.`
        : `Adds to the installed ${v.base_title} (the Linux version). GRID unpacks its files into the game’s folder; the installer script isn’t run.`,
      already: '',
      showCompat: false,
      showFolder: false,
      joins: '',
      button: 'Install DLC',
    };
  }
  const what = `${windows ? 'Windows version' : 'Linux version'}${version ? ` · v${version}` : ''} · ${size}`;
  const how = windows
    ? `GRID runs the installer silently under ${opts.compat || 'its compatibility tool'}. If it can’t run silently, its own setup window opens: finish it there and GRID adds the game when it closes.`
    : 'GRID unpacks the game files from it. The installer script isn’t run.';
  if (v.kind === 'update') {
    return {
      kind: 'found',
      heading: `Update ${insp.title}${version ? ` to ${version}` : ''}`,
      title: insp.title,
      what,
      from,
      how,
      already: `${insp.title} is installed already (v${v.installed_version}). Installing this replaces its files with ${
        version ? `v${version}` : 'this version'
      }. Saves and settings stay.`,
      showCompat: windows,
      showFolder: false,
      joins: '',
      button: 'Update',
    };
  }
  return {
    kind: 'found',
    heading: `Install ${insp.title}`,
    title: insp.title,
    what,
    from,
    how,
    already: '',
    showCompat: windows,
    showFolder: true,
    joins: opts.gogConnected
      ? 'It joins your GOG games. Updates and cloud saves come from GOG while you’re signed in.'
      : 'It joins GOG › Installed. Sign in to GOG later for its updates and cloud saves.',
    button: 'Install',
  };
}

/** A DLC's own name without its game's ("Digital Artbook" from "Ashen Tide — Digital Artbook"). */
function dlcName(title: string, base: string): string {
  const t = title.trim();
  for (const sep of [' — ', ' - ', ': ']) {
    if (t.startsWith(base + sep)) return t.slice(base.length + sep.length);
  }
  return t || 'DLC';
}

/** The footer's space note: "Needs 13.2 GB · 412 GB free". */
export function spaceNote(needBytes: number, freeBytes: number | null): string {
  return freeBytes === null ? `Needs ${sizeGb(needBytes)}` : `Needs ${sizeGb(needBytes)} · ${sizeGb(freeBytes)} free`;
}
