// Server firmware on the Emulators › Installed list (user ruling 2026-10-30:
// firmware is installed so an emulator can run, so its Install lives with
// the emulator, not with a game or a platform). Pure, like `defaults.ts`.
//
// RomM holds firmware per platform, and GRID installs a platform's firmware
// for that platform's default emulator, so an emulator lists the platforms
// it is the default for whose server firmware has files.
import type { LaunchDefaults, Platform, PlatformFirmwareStatus } from '../api';
import { defaultEmulatorKey } from './defaults';

export type EmulatorFirmwareRow = {
  platformId: number;
  /** The platform's name as the firmware commands take it. */
  platformName: string;
  label: string;
  /** Every server file is already in place for this emulator. */
  installed: boolean;
};

/** Platforms whose firmware status the Installed list needs: those with a
 *  default emulator that is configured. */
export function firmwarePlatforms(
  platforms: Platform[],
  defaults: LaunchDefaults | null,
  emulatorNames: string[]
): Platform[] {
  if (defaults === null) return [];
  const names = new Set(emulatorNames);
  return platforms.filter((p) => names.has(defaults.default_emulators[defaultEmulatorKey(p)] ?? ''));
}

/** One line per platform this emulator is the default for whose server
 *  firmware has files, in platform order. */
export function emulatorFirmwareRows(
  emulatorName: string,
  platforms: Platform[],
  defaults: LaunchDefaults | null,
  statuses: Record<number, PlatformFirmwareStatus>
): EmulatorFirmwareRow[] {
  if (defaults === null) return [];
  const rows: EmulatorFirmwareRow[] = [];
  for (const p of platforms) {
    const key = defaultEmulatorKey(p);
    if (defaults.default_emulators[key] !== emulatorName) continue;
    const status = statuses[p.id];
    const count = status?.file_count ?? 0;
    if (count <= 0) continue;
    rows.push({
      platformId: p.id,
      platformName: key,
      label: `${key} firmware · ${count === 1 ? '1 file' : `${count} files`}`,
      installed: status?.installed === true,
    });
  }
  return rows;
}
