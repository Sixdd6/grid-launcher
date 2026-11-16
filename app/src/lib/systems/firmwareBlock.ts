// A system's firmware block in Settings › Systems (RommStorefront README,
// "Firmware block"; TVRommStorefront "The firmware block"): the files the
// emulator needs (`firmware_expected`), and the two ways to get them —
// Install from RomM and Locate files…. Pure: the desktop block and the TV
// System page render what this answers.
import type { FirmwareExpectedFile, RommConnection } from '../api';
import { shortName } from './rows';

/** What the block is about. */
export type FirmwareBlockInput = {
  system: string;
  slug: string;
  /** The system's emulator, `''` when none is chosen. */
  emulator: string;
  /** `firmware_expected`'s files; `null` before the answer. */
  files: FirmwareExpectedFile[] | null;
  connection: RommConnection;
  /** RomM has the platform (`SystemPlatform.server_id`). */
  onServer: boolean;
  /** RomM has firmware for the platform that can land in the emulator's
   *  folders; `null` while that is unknown (not asked, or RomM off). */
  serverFirmware: boolean | null;
  /** An Install from RomM pass is running. */
  installing: boolean;
};

/** Why Install from RomM can't run, or `null` when it can. */
export type RommBlock = 'no-emulator' | 'needs-romm' | 'not-on-server' | 'no-server-firmware' | 'all-installed';

export type FirmwareFileLine = {
  name: string;
  /** "Required" / "Optional", plus " · no fixed checksum" for key files. */
  meta: string;
  state: 'verified' | 'unverified' | 'missing' | 'optional';
  /** "Installed · verified", "Installed · not verified", "Missing",
   *  "Missing · optional". */
  status: string;
  /** Who checked it, once installed: "checked against RomM" / "checked by GRID". */
  checkedBy: string;
};

export type FirmwareBlockView = {
  /** "3 files · 2 installed" / "3 files · all installed". */
  count: string;
  /** A required file is missing: the row says "Firmware needed". */
  needed: boolean;
  romm: { label: string; disabled: boolean; primary: boolean; why: RommBlock | null; title: string };
  locate: { label: string; disabled: boolean; primary: boolean };
  /** The reason line under the buttons, or `null`. */
  reason: 'no-emulator' | 'needs-romm' | 'not-on-server' | null;
  /** PlayStation 3: the PUP picker stands in for Locate files. */
  pup: boolean;
  /** Installed and required files, then the optional missing ones. */
  files: FirmwareFileLine[];
  /** Missing optional files beyond the first few, folded under a count. */
  moreOptional: FirmwareFileLine[];
};

/** How many missing optional files show before the rest fold away. */
export const OPTIONAL_SHOWN = 4;

function plural(n: number, one: string): string {
  return `${n} ${one}${n === 1 ? '' : 's'}`;
}

/** Whether `slug` installs its firmware as RPCS3's system update. */
export function isPupSystem(slug: string): boolean {
  return slug === 'ps3';
}

function lineOf(f: FirmwareExpectedFile): FirmwareFileLine {
  const noHash = f.md5 === '';
  const meta = `${f.required ? 'Required' : 'Optional'}${noHash ? ' · no fixed checksum' : ''}`;
  if (f.installed) {
    const verified = f.verified_by !== 'none';
    return {
      name: f.name,
      meta,
      state: verified ? 'verified' : 'unverified',
      status: verified ? 'Installed · verified' : 'Installed · not verified',
      checkedBy: f.verified_by === 'romm' ? 'checked against RomM' : f.verified_by === 'grid' ? 'checked by GRID' : '',
    };
  }
  return {
    name: f.name,
    meta,
    state: f.required ? 'missing' : 'optional',
    status: f.required ? 'Missing' : 'Missing · optional',
    checkedBy: '',
  };
}

/** The block for `input`. */
export function firmwareBlock(input: FirmwareBlockInput): FirmwareBlockView {
  const files = input.files ?? [];
  const installed = files.filter((f) => f.installed).length;
  const needed = files.some((f) => f.required && !f.installed);
  const pup = isPupSystem(input.slug);
  const why: RommBlock | null =
    input.emulator === ''
      ? 'no-emulator'
      : input.connection !== 'connected'
        ? 'needs-romm'
        : !input.onServer
          ? 'not-on-server'
          : input.serverFirmware === false
            ? 'no-server-firmware'
            : null;
  const canRomm = why === null;
  const title =
    why === 'needs-romm'
      ? 'Install from RomM needs RomM'
      : why === 'not-on-server'
        ? 'Not on your RomM server'
        : why === 'no-server-firmware'
          ? `Your RomM has no ${input.system} firmware`
          : why === 'no-emulator'
            ? 'Install an emulator first'
            : '';
  const lines = files.map(lineOf);
  const head = lines.filter((l) => l.state !== 'optional');
  const optional = lines.filter((l) => l.state === 'optional');
  const shownOptional = optional.length <= OPTIONAL_SHOWN + 1 ? optional : optional.slice(0, OPTIONAL_SHOWN);
  return {
    count: `${plural(files.length, 'file')} · ${files.length > 0 && installed === files.length ? 'all installed' : `${installed} installed`}`,
    needed: input.emulator !== '' && needed,
    romm: {
      label: input.installing ? 'Installing…' : 'Install from RomM',
      disabled: !canRomm || input.installing,
      primary: canRomm && needed,
      why,
      title,
    },
    locate: {
      label: pup ? 'Locate PS3UPDAT.PUP…' : 'Locate files…',
      disabled: input.emulator === '' || input.installing,
      primary: input.emulator !== '' && !canRomm && needed,
    },
    reason:
      why === 'no-emulator'
        ? 'no-emulator'
        : why === 'needs-romm'
          ? 'needs-romm'
          : why === 'not-on-server' || why === 'no-server-firmware'
            ? 'not-on-server'
            : null,
    pup,
    files: [...head, ...shownOptional],
    moreOptional: optional.slice(shownOptional.length),
  };
}

/** The reason line's words (desktop). `needs-romm` has a link in the
 *  middle: the view renders `before`, the link, then `after`. */
export const REASON_WORDS = {
  'no-emulator': 'Install an emulator first. GRID puts firmware into its folders.',
  'needs-romm': { before: 'Install from RomM needs RomM · ', link: 'Connect RomM…', after: ' · or locate files you have.' },
  'not-on-server': 'Not on your RomM server · locate files you have.',
} as const;

/** The Locate dialog's title. */
export function locateTitle(system: string, pup: boolean): string {
  return pup ? 'Locate PS3UPDAT.PUP' : `Locate ${system} firmware`;
}

/** The Locate dialog's lede. */
export function locateLede(emulator: string, pup: boolean): string {
  const emu = shortName(emulator);
  return pup
    ? `Pick the PS3 system software update file you downloaded. GRID checks it, then ${emu} installs it.`
    : `Pick firmware files you have, or a folder. GRID checks each file’s size and MD5, then puts the ones it knows into ${emu}’s folders.`;
}
