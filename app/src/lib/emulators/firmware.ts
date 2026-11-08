// Server firmware under an emulator in Settings › Systems (user ruling
// 2026-10-30: firmware is installed so an emulator can run, so its Install
// lives with the emulator, not with a game). Pure, like `defaults.ts`.
//
// RomM holds firmware per platform. An emulator lists the platforms it
// supports (the backend's `compatible_emulators` answer, the same list a
// system's picker offers) whose server firmware has files, whether
// or not it is that platform's default: firmware is what lets the emulator
// run, and a platform set to "(none)" still has its emulators installed.
// Only the default emulator is told "Unavailable"; any other emulator lists
// a platform only when the firmware can land in its folders, so a hand-added
// entry with no profile (which "supports" every platform) lists nothing.
import type { LaunchDefaults, Platform, PlatformFirmwareStatus } from '../api';
import { defaultEmulatorKey, isEmulatorsShelf, needsEmulator, savedDefaultFor } from './defaults';

export type EmulatorFirmwareRow = {
  /** [`firmwareKey`] for this emulator and platform. */
  key: string;
  platformId: number;
  /** The platform's name as the firmware commands take it. */
  platformName: string;
  label: string;
  /** Every server file is already in place for this emulator. */
  installed: boolean;
  /** Some server file can land in this emulator's folders. */
  available: boolean;
};

/** Where one emulator's firmware status for one platform is kept. */
export function firmwareKey(emulatorName: string, platformId: number): string {
  return `${platformId}:${emulatorName}`;
}

/** The platforms `emulatorName` supports that can hold firmware for it:
 *  never a native platform or the Emulators shelf. `compatible` is keyed by
 *  the platform's raw `name`. */
function supportedPlatforms(
  emulatorName: string,
  platforms: Platform[],
  compatible: Record<string, string[]>
): Platform[] {
  return platforms.filter(
    (p) => needsEmulator(p) && !isEmulatorsShelf(p) && (compatible[p.name] ?? []).includes(emulatorName)
  );
}

export type FirmwareRequest = { key: string; emulator: string; platform: Platform };

/** One line per platform this emulator supports whose server firmware has
 *  files and either can land in its folders or is its to fail (it is the
 *  platform's default), in platform order. */
export function emulatorFirmwareRows(
  emulatorName: string,
  platforms: Platform[],
  compatible: Record<string, string[]>,
  defaults: LaunchDefaults | null,
  statuses: Record<string, PlatformFirmwareStatus>
): EmulatorFirmwareRow[] {
  const rows: EmulatorFirmwareRow[] = [];
  for (const p of supportedPlatforms(emulatorName, platforms, compatible)) {
    const key = firmwareKey(emulatorName, p.id);
    const status = statuses[key];
    const count = status?.file_count ?? 0;
    if (count <= 0) continue;
    const name = defaultEmulatorKey(p);
    const isDefault = savedDefaultFor(defaults?.default_emulators, name) === emulatorName;
    if (!isDefault && status?.available !== true && status?.installed !== true) continue;
    rows.push({
      key,
      platformId: p.id,
      platformName: name,
      label: `${name} firmware · ${count === 1 ? '1 file' : `${count} files`}`,
      installed: status?.installed === true,
      available: status?.available === true,
    });
  }
  return rows;
}

/**
 * What a firmware line offers. `requested` while an install runs,
 * `checking` while a click on Not needed re-reads the server, `installed`
 * once every file is in place, `unavailable` when nothing on the server can
 * land in this emulator's folders (Install would place nothing), else
 * `install`.
 */
export type FirmwareAction = 'install' | 'requested' | 'checking' | 'installed' | 'unavailable';

export function firmwareAction(row: EmulatorFirmwareRow, requested: boolean, checking: boolean): FirmwareAction {
  if (requested) return 'requested';
  if (checking) return 'checking';
  if (!row.available) return 'unavailable';
  return row.installed ? 'installed' : 'install';
}

/** The toast an Install click shows at once. */
export function firmwareStartedToast(platformName: string, emulatorName: string): string {
  return `Installing ${platformName} firmware for ${emulatorName}…`;
}

/**
 * The toast that ends an Install click, from the backend's pass-finished
 * event. `failed` counts the files the pass could not place (one warning
 * each); `ok` false with nothing failed means the pass never ran.
 */
export function firmwareFinishedToast(
  platformName: string,
  emulatorName: string,
  ok: boolean,
  failed: number
): { text: string; level: 'success' | 'error' } {
  if (ok) return { text: `${platformName} firmware installed for ${emulatorName}`, level: 'success' };
  if (failed > 0) {
    const files = failed === 1 ? '1 file' : `${failed} files`;
    return {
      text: `${platformName} firmware for ${emulatorName}: ${files} could not be installed`,
      level: 'error',
    };
  }
  return { text: `Could not install ${platformName} firmware for ${emulatorName}`, level: 'error' };
}
