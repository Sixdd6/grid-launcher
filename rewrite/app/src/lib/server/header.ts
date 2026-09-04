// The Server platform header's text (design §6). Pure; Task 6 adds the
// firmware chip's label to this module.

import type { PlatformFirmwareStatus } from '../api';

/** "42 games · 7 installed" — the header's counts line. */
export function platformCountsLine(romCount: number, installedCount: number): string {
  const games = romCount === 1 ? '1 game' : `${romCount} games`;
  return `${games} · ${installedCount} installed`;
}

/** The default-emulator chip's text. */
export function emulatorChipLabel(name: string): string {
  const trimmed = name.trim();
  return trimmed === '' ? 'No default emulator' : `Emulator: ${trimmed}`;
}

/**
 * The `server-firmware-chip` text (design §6: "firmware status chip with an
 * Install action when the server offers firmware"). `null` is the state
 * before the status command answers — named rather than blank so the chip
 * does not appear and then jump.
 */
export function firmwareChipLabel(status: PlatformFirmwareStatus | null): string {
  if (status === null) return 'Firmware: checking…';
  if (status.file_count === 0) return 'No server firmware';
  const files = status.file_count === 1 ? '1 file' : `${status.file_count} files`;
  if (!status.has_default_emulator) return `Firmware: ${files} — no default emulator`;
  return `Firmware: ${files}`;
}

/** Whether the chip offers its Install action: the server has firmware AND
 *  the platform has an emulator whose profile says where it goes. */
export function firmwareInstallable(status: PlatformFirmwareStatus | null): boolean {
  return status !== null && status.file_count > 0 && status.has_default_emulator;
}
