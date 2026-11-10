/**
 * Fake removable drives for the `backups` stage group.
 *
 * scripts/e2e.sh points the app's `GRID_LAUNCHER_E2E_DRIVE_ROOTS` at
 * `<data dir>/media` (exported to specs as `E2E_DRIVE_ROOTS`). The e2e build
 * then treats every child folder there as a mounted drive, keyed by its path,
 * and skips dot-folders (crates/grid-core/src/backup/drives.rs). So plugging
 * in is a `mkdir`, and unplugging is a rename to `.<name>`: the drive's files
 * survive for the next `plugIn`, under the same mount path. An optional
 * `<drive>/.e2e-drive.json` fakes the label and the file system.
 */
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

function required(name: string): string {
  const value = process.env[name];
  if (!value || value.trim() === '') {
    throw new Error(`${name} is not set. Run the backups group through scripts/e2e.sh.`);
  }
  return value;
}

/** The folder whose child folders are the mounted drives. */
export const driveRoot = (): string => required('E2E_DRIVE_ROOTS');

/** The mount path (and drive key) of the drive named `label`. */
export const mountOf = (label: string): string => path.join(driveRoot(), label);

/** Where an unplugged drive's files wait. */
const awayOf = (label: string): string => path.join(driveRoot(), `.${label}`);

/** The drive's `gridpack/` folder. */
export const packDir = (label: string): string => path.join(mountOf(label), 'gridpack');

/** The file the fake drive probe reads (not part of what GRID writes). */
export const FAKE_DRIVE_FILE = '.e2e-drive.json';

export type PlugOptions = {
  /** The raw file system name the drive reports: `exfat` (default), `vfat`… */
  fs?: string;
  total_bytes?: number;
  free_bytes?: number;
  read_only?: boolean;
};

/**
 * Plugs in the drive `label`: the files it held when it was unplugged come
 * back, or a new empty drive appears. The app notices within one poll.
 */
export function plugIn(label: string, options: PlugOptions = {}): string {
  const mount = mountOf(label);
  mkdirSync(driveRoot(), { recursive: true });
  if (!existsSync(mount) && existsSync(awayOf(label))) {
    renameSync(awayOf(label), mount);
  }
  mkdirSync(mount, { recursive: true });
  const fake = path.join(mount, FAKE_DRIVE_FILE);
  if (!existsSync(fake) || Object.keys(options).length > 0) {
    writeFileSync(fake, JSON.stringify({ label, fs: options.fs ?? 'exfat', ...options }));
  }
  return mount;
}

/** Unplugs the drive `label`, keeping its files for a later `plugIn`. */
export function unplug(label: string): void {
  rmSync(awayOf(label), { recursive: true, force: true });
  renameSync(mountOf(label), awayOf(label));
}

/** Removes the drive `label` and everything on it. */
export function discard(label: string): void {
  rmSync(mountOf(label), { recursive: true, force: true });
  rmSync(awayOf(label), { recursive: true, force: true });
}

/** A file entry of a `gridpack.json` game. */
export type PackFile = { path: string; size: number; server_file_id: number; name: string; category: string };

/** A game's `installed` entry: a copy of the install it was backed up from. */
export type PackInstalled = {
  archive: string;
  layout: 'extracted' | 'native' | 'multi' | 'file' | string;
  name: string;
  parts: { path: string; size: number; md5: string }[];
  tree_bytes: number;
  tree_files: number;
  launch: string;
  rom_file_name: string;
  server_updated_at: string;
  outdated: boolean;
  row: Record<string, unknown>;
};

/** The parts of `gridpack.json` the specs read. */
export type PackIndex = {
  format: number;
  name: string;
  os: string[];
  games: {
    platform: string;
    platform_name: string;
    name: string;
    source: { server: string; rom_id: number };
    files: PackFile[];
    assets: Record<string, unknown>;
    installed?: PackInstalled;
  }[];
  emulators: { name: string; os: string; path: string; source_id: string }[];
  firmware: { platform: string; files: { name: string; path: string; size: number }[] }[];
};

/** Reads and parses the drive's `gridpack/gridpack.json` from Node. */
export function readPack(label: string): PackIndex {
  return JSON.parse(readFileSync(path.join(packDir(label), 'gridpack.json'), 'utf-8')) as PackIndex;
}

/** The drive's pack, or `null` while it has none or it is mid-write. */
export function tryReadPack(label: string): PackIndex | null {
  try {
    return readPack(label);
  } catch {
    return null;
  }
}
