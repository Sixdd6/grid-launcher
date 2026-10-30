// The Server platform header's text (design §6). Pure; Task 6 adds the
// firmware chip's label to this module.

import type { PlatformFirmwareStatus } from '../api';
import { isNativeLaunchPlatform } from '../details/cloud';
import { NO_EMULATOR_MARKER } from '../emulators/defaults';

/** "42 games · 7 installed" — the header's counts line. */
export function platformCountsLine(romCount: number, installedCount: number): string {
  const games = romCount === 1 ? '1 game' : `${romCount} games`;
  return `${games} · ${installedCount} installed`;
}

/**
 * The default-emulator chip's text.
 *
 * `get_launch_defaults` hands back `config.default_emulators` unfiltered, so
 * the name can be the reserved `<none>` marker the Emulators panel writes for
 * its "(none)" choice. A remembered "no emulator" reads the same as an absent
 * one here: the chip states a fact, it does not edit the mapping.
 */
export function emulatorChipLabel(name: string): string {
  const trimmed = name.trim();
  if (trimmed === '' || trimmed === NO_EMULATOR_MARKER) return 'No default emulator';
  return `Emulator: ${trimmed}`;
}

/**
 * Whether the Server header shows an emulator chip at all for this platform
 * (design §6: the emulator chip is omitted for native platforms). A native
 * platform's games launch as host executables, so "No default emulator" was
 * never a true statement about them — the chip is not merely relabeled, it
 * does not appear.
 */
export function showsEmulatorChip(platformLabel: string, platformSlug: string): boolean {
  return !isNativeLaunchPlatform(platformSlug, platformLabel);
}

/**
 * What the firmware chip knows: the status, `null` while the command is still
 * in flight, or `'unavailable'` when it was refused.
 */
export type FirmwareChipState = PlatformFirmwareStatus | null | 'unavailable';

/**
 * The `server-firmware-chip` text (design §6: "firmware status chip with an
 * Install action when the server offers firmware"). `null` is the state
 * before the status command answers — named rather than blank so the chip
 * does not appear and then jump. `'unavailable'` is a refused status call:
 * saying so beats sitting at "checking…" forever.
 */
export function firmwareChipLabel(status: FirmwareChipState): string {
  if (status === 'unavailable') return 'Firmware: unavailable';
  if (status === null) return 'Firmware: checking…';
  if (status.file_count === 0) return 'No server firmware';
  const files = status.file_count === 1 ? '1 file' : `${status.file_count} files`;
  if (!status.has_default_emulator) return `Firmware: ${files} — no default emulator`;
  if (!status.available) return `Firmware: ${files} · unavailable`;
  if (status.installed) return `Firmware: ${files} · installed`;
  return `Firmware: ${files}`;
}

/**
 * Where the firmware chip links: Emulators › Installed, where each emulator
 * lists its platforms' server firmware with Install (user ruling 2026-10-30),
 * or Platform defaults when the platform has no default emulator to install
 * it for. `null` (a plain chip) when the server has no firmware for it.
 */
export function firmwareChipTarget(status: FirmwareChipState): 'installed' | 'defaults' | null {
  if (status === null || status === 'unavailable' || status.file_count === 0) return null;
  return status.has_default_emulator ? 'installed' : 'defaults';
}
