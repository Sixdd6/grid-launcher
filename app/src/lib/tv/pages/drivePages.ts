// A Backup drive's pages in TV mode (checklist items 499–513; tvmode.js
// `drivePage`, `osPage`, `setupPage`; rulings 9 and 14): the drive page, the
// "Computers this pack is for" page and Set up for GRID. Pure, over the
// desktop's drive rules (`backups/drives.ts`); the pages run the shared
// drive commands (`backups/driveActions.ts`).
import type { DriveInfo, PackOs } from '../../api';
import { sizeText } from '../../backups/dialog';
import { emulatorNames, gridpackPath, osNames, packFacts, PACK_OSES, setupError, usageSegments } from '../../backups/drives';
import { formatBytes } from '../../settings/library';
import { plural, type PageMain, type PageRow } from './pageRows';

export const AT_LEAST_ONE = 'At least one computer stays ticked';

/** The drive page's line under its title (item 499). */
export function drivePageSub(d: DriveInfo): string {
  return `${d.label} · ${d.fs} · ${formatBytes(d.total_bytes)} · ${d.status === 'ready' ? 'Ready for backups' : 'Read-only'}`;
}

/** "Other files 812 GB · GRID 126.6 GB · 61.4 GB free" (item 501). */
export function spaceLine(d: DriveInfo): string {
  const [other, grid, free] = usageSegments(d.total_bytes, d.free_bytes, d.pack?.gridpack_bytes ?? 0);
  return `Other files ${sizeText(other.bytes)} · GRID ${sizeText(grid.bytes)} · ${sizeText(free.bytes)} free`;
}

export type DrivePageActions = {
  changeOs: () => void;
  showInLibrary: () => void;
  openFolder: () => void;
  rename: () => void;
  removeFiles: () => void;
};

export const OPEN_FOLDER_SUB = 'Opens the file manager in desktop mode';
export const REMOVE_FILES_SUB = 'Deletes only the gridpack folder; games installed on this computer stay';

/** The drive page's rows (item 500). A read-only drive can't be changed. */
export function drivePageRows(d: DriveInfo, act: DrivePageActions): PageRow[] {
  const pack = d.pack;
  if (!pack) return [];
  const editable = d.status === 'ready' && !d.read_only;
  const [other, grid] = usageSegments(d.total_bytes, d.free_bytes, pack.gridpack_bytes);
  const facts = packFacts(d);
  const fact = (label: string) => facts.find((f) => f.label === label)?.value ?? '';
  const emus = emulatorNames(pack.emulators);
  const R: PageRow[] = [];
  const forLine = `For ${osNames(pack.os) || 'no computer'}`;
  if (editable) R.push({ kind: 'row', key: 'dp-os', label: forLine, value: 'Change…', al: 'Change', run: act.changeOs });
  else R.push({ kind: 'info', key: 'dp-os', label: forLine });
  R.push({
    kind: 'info',
    key: 'dp-space',
    label: 'Space',
    bar: {
      wide: true,
      segs: [
        { kind: 'used', pct: other.pct },
        { kind: 'grid', pct: grid.pct },
      ],
      label: spaceLine(d),
      after: spaceLine(d),
    },
  });
  R.push({ kind: 'info', key: 'dp-games', label: 'Games', value: String(pack.game_count) });
  R.push({ kind: 'info', key: 'dp-emus', label: 'Emulators', value: emus.length ? emus.join(', ') : 'None' });
  R.push({ kind: 'info', key: 'dp-fw', label: 'Firmware', value: plural(pack.firmware.length, 'system') });
  R.push({ kind: 'info', key: 'dp-last', label: 'Last backup', value: fact('Last backup') });
  R.push({ kind: 'info', key: 'dp-folder', label: 'Folder', value: gridpackPath(d.mount) });
  R.push({ kind: 'row', key: 'dp-show', label: 'Show in Library', run: act.showInLibrary });
  R.push({ kind: 'row', key: 'dp-open', label: 'Open folder', sub: OPEN_FOLDER_SUB, run: act.openFolder });
  if (editable) {
    R.push({ kind: 'row', key: 'dp-rename', label: 'Rename…', value: pack.name, al: 'Rename', run: act.rename });
    R.push({ kind: 'row', key: 'dp-remove', label: 'Remove GRID files…', danger: true, sub: REMOVE_FILES_SUB, run: act.removeFiles });
  }
  return R;
}

/** Remove GRID files… asks twice (item 508), then says what it freed (509). */
export function removeFilesWords(pack: string, label: string, bytes: number) {
  const size = sizeText(bytes);
  return {
    first: {
      title: `Remove GRID files from ${pack}?`,
      body: `The gridpack folder (${size}) is deleted from ${label}. Nothing else on the drive is touched, and the games installed on this computer stay.`,
      ok: 'Continue',
    },
    second: { title: `Delete ${size} from ${label}?`, body: 'This can’t be undone.', ok: 'Remove GRID files' },
    freed: `${size} freed`,
  };
}

export const RENAME_TITLE = 'Pack name';
export const renamedText = (name: string) => `Renamed to ${name}`;

// ── Computers this pack is for (items 502–504; ruling 9)

export const OS_PAGE_TITLE = 'Computers this pack is for';
export const OS_PAGE_SUB = 'Emulators go on the drive built for each one. At least one stays ticked.';

/** The OS list after flipping `os`, or the refusal when it would leave none. */
export function flipOs(current: readonly PackOs[], os: PackOs): { next: PackOs[] } | { refused: string } {
  const on = current.includes(os);
  const next = on ? current.filter((o) => o !== os) : PACK_OSES.map((p) => p.os).filter((o) => o === os || current.includes(o));
  if (next.length === 0) return { refused: AT_LEAST_ONE };
  return { next };
}

/** Turning a computer off deletes its emulator builds: it asks first (ruling 9). */
export function osOffConfirm(os: PackOs, pack: string): { title: string; body: string; ok: string } {
  const name = osNames([os]);
  return {
    title: `Remove the ${name} builds from ${pack}?`,
    body: `The ${name} builds of the emulators on ${pack} are deleted from the drive. Games and firmware stay.`,
    ok: `Remove ${name} builds`,
  };
}

export function osPageRows(os: readonly PackOs[], flip: (os: PackOs) => void): PageRow[] {
  return PACK_OSES.map((p) => ({ kind: 'row' as const, key: `os-${p.os}`, toggle: os.includes(p.os), label: p.label, run: () => flip(p.os) }));
}

// ── Set up for GRID (items 510–513; ruling 14)

export type SetupForm = { name: string; os: PackOs[] };

export const setupTitle = (label: string) => `Set up ${label} for GRID`;
export const setupSub = (d: DriveInfo) => `${d.fs} · ${formatBytes(d.total_bytes)} · Not set up`;
export const FAT32_NOTE = 'This drive is FAT32: files over 4 GB can’t go on it. exFAT avoids that. GRID never formats a drive.';
export const GRIDPACK_NOTE = 'GRID makes a gridpack folder at the drive’s root and never touches anything outside it.';
export const COMPUTERS_HEADING = 'Computers this pack is for';

/** The form's start: the volume label, and this computer's OS ticked (ruling 14). */
export function setupStart(d: Pick<DriveInfo, 'label' | 'mount'>, host: PackOs | null, defaultName: string): SetupForm {
  return { name: defaultName, os: host ? [host] : [] };
}

export function setupRows(d: DriveInfo, f: SetupForm, act: { editName: () => void; flip: (os: PackOs) => void }): PageRow[] {
  const R: PageRow[] = [
    { kind: 'row', key: 'su-name', label: 'Pack name', value: f.name, al: 'Edit', run: act.editName },
    { kind: 'heading', key: 'su-h-os', text: COMPUTERS_HEADING },
    ...PACK_OSES.map((p) => ({ kind: 'row' as const, key: `su-${p.os}`, toggle: f.os.includes(p.os), label: p.label, run: () => act.flip(p.os) })),
  ];
  if (d.fat32 || d.fs.toUpperCase() === 'FAT32') R.push({ kind: 'note', key: 'su-fat32', text: FAT32_NOTE, warn: true });
  R.push({ kind: 'note', key: 'su-note', text: GRIDPACK_NOTE });
  return R;
}

/** The setup page's main button (item 513): disabled with the desktop's reason. */
export function setupMain(f: SetupForm, busy: boolean, press: () => void): PageMain {
  const err = setupError(f.name, f.os);
  if (busy) return { label: 'Setting up…', summary: '', disabled: true, why: 'Setting the drive up.', hint: 'Set up', press };
  return { label: 'Set up for GRID', summary: '', disabled: err !== null, why: err ?? undefined, hint: 'Set up', press };
}

/** The card after setup (item 513). */
export function setupDone(name: string, d: DriveInfo): { text: string; sub: string } {
  return { text: `${name.trim()} is ready for backups`, sub: `${d.label} · ${d.fs}` };
}
