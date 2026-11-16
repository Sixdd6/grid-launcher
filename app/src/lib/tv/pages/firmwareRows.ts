// The firmware block on a TV System page (TVRommStorefront README, "The
// firmware block (a System page)"): one information row per expected file,
// a note, then Install from RomM and Locate files… as action rows. A row
// that can't run is dimmed and A says why. Pure: TvSystemPage renders the
// rows and runs the actions.
import type { FirmwareExpectedFile, RommConnection } from '../../api';
import { OPTIONAL_SHOWN, type FirmwareBlockView } from '../../systems/firmwareBlock';
import { shortName } from '../../systems/rows';
import type { Row } from '../rows';

export type FirmwareRowsInput = {
  system: string;
  /** The system's emulator (profile name), `''` for none. */
  emulator: string;
  /** The RomM host ("romm.home.lan"), for the Install from RomM line. */
  host: string;
  connection: RommConnection;
  files: FirmwareExpectedFile[];
  view: FirmwareBlockView;
};

export type FirmwareRowsActions = { romm: () => void; locate: () => void };

/** "512 KB", "4 MB", "197 MB". */
export function sizeText(bytes: number): string {
  if (bytes <= 0) return '';
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  const mb = bytes / (1024 * 1024);
  return `${mb < 10 ? Math.round(mb * 10) / 10 : Math.round(mb)} MB`;
}

/** The line under a file's name. */
export function fileSub(f: FirmwareExpectedFile): string {
  const parts: string[] = [];
  const size = sizeText(f.size);
  if (f.md5 === '') parts.push("Different on every console, so GRID can't check it");
  else parts.push(`${size ? `${size} · ` : ''}${f.required ? 'required' : 'optional'}`);
  if (f.installed && f.verified_by === 'romm') parts.push('checked against RomM');
  if (f.installed && f.verified_by === 'grid') parts.push('checked by GRID');
  return parts.join(' · ');
}

/** A file's value. */
export function fileValue(f: FirmwareExpectedFile): string {
  if (f.installed) return f.verified_by === 'none' ? 'Installed · not verified' : 'Installed';
  return f.required ? 'Missing · required' : 'Missing';
}

/** The note under the files. */
export function firmwareNote(i: Pick<FirmwareRowsInput, 'system' | 'emulator' | 'files' | 'view'>): string {
  if (i.emulator === '') return `Install an emulator for ${i.system} first. GRID puts firmware in its folders.`;
  if (i.view.pup) return 'RPCS3 installs the PlayStation 3 system update file, PS3UPDAT.PUP. GRID checks it first.';
  if (i.files.some((f) => f.md5 === '')) {
    return 'GRID checks every file it knows by size and MD5. Key files are different on every console, so they install as not verified.';
  }
  return `GRID checks each file’s size and MD5 before it goes into ${shortName(i.emulator)}’s folders.`;
}

/** Why Install from RomM is dimmed: its value and what A says. */
export function rommDimmed(i: FirmwareRowsInput): { value: string; why: string } | null {
  const { view, system } = i;
  if (view.romm.label === 'Installing…') return null;
  switch (view.romm.why) {
    case 'no-emulator':
      return { value: 'Needs an emulator first', why: `Install an emulator for ${system} first. GRID puts firmware in its folders.` };
    case 'needs-romm':
      return {
        value: 'Needs RomM',
        why:
          i.connection === 'expired'
            ? 'Your RomM sign-in expired. Sign in again in Settings › Game stores.'
            : i.connection === 'unreachable'
              ? 'Can’t reach RomM right now. GRID tries again when you press Retry.'
              : 'Install from RomM needs RomM. Connect RomM in Settings › Game stores.',
      };
    case 'not-on-server':
      return { value: 'Not on your RomM', why: `${system} isn’t on your RomM server.` };
    case 'no-server-firmware':
      return { value: 'Your RomM has no firmware for it', why: `Your RomM has no ${system} firmware.` };
    default:
      break;
  }
  if (i.files.length > 0 && i.files.every((f) => f.installed)) {
    return { value: 'Every file is installed', why: `Every ${system} firmware file is installed.` };
  }
  return null;
}

/** The block's rows. */
export function firmwareRows(i: FirmwareRowsInput, a: FirmwareRowsActions): Row[] {
  const rows: Row[] = [];
  const optionalMissing = i.files.filter((f) => !f.installed && !f.required);
  const shownOptional = optionalMissing.length <= OPTIONAL_SHOWN + 1 ? optionalMissing : optionalMissing.slice(0, OPTIONAL_SHOWN);
  const shown = [...i.files.filter((f) => f.installed || f.required), ...shownOptional];
  for (const f of shown) {
    rows.push({ type: 'info', key: `fw-file-${f.name}`, label: f.name, sub: fileSub(f), value: fileValue(f) });
  }
  const hidden = optionalMissing.length - shownOptional.length;
  if (hidden > 0) {
    rows.push({ type: 'info', key: 'fw-more', label: `${hidden} more optional files GRID knows`, value: '' });
  }
  rows.push({ type: 'info', key: 'fw-note', label: firmwareNote(i), value: '' });
  const dim = rommDimmed(i);
  const count = i.files.filter((f) => !f.installed).length || i.files.length;
  rows.push({
    type: 'action',
    key: 'fw-romm',
    label: 'Install from RomM',
    sub: `Downloads ${count === 1 ? '1 file' : `${count} files`} from ${i.host || 'RomM'} and checks each one`,
    ...(i.view.romm.label === 'Installing…' ? { value: 'Installing…', disabled: true, why: 'Still in progress. Try again when it finishes.' } : {}),
    ...(dim ? { value: dim.value, disabled: true, why: dim.why } : {}),
    run: a.romm,
  });
  rows.push({
    type: 'action',
    key: 'fw-locate',
    label: i.view.locate.label,
    sub: i.view.pup ? 'Pick the update file on this computer or a drive' : 'Pick files on this computer or a drive. GRID installs the ones it knows.',
    ...(i.emulator === ''
      ? { value: 'Needs an emulator first', disabled: true, why: `Install an emulator for ${i.system} first. GRID puts firmware in its folders.` }
      : {}),
    run: a.locate,
  });
  return rows;
}
