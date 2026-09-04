// The Server platform header's text (design §6). Pure; Task 6 adds the
// firmware chip's label to this module.

import type { PlatformFirmwareStatus } from '../api';
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
  return `Firmware: ${files}`;
}

/**
 * Whether the chip offers its Install action: the server lists firmware for
 * the platform AND the platform resolves to a configured emulator.
 *
 * `has_default_emulator` is exactly that and no more (`commands.rs`'s
 * `platform_firmware_status` → `default_entry_for_platform`): a config entry
 * is found for the platform's default emulator name. It does NOT say the
 * emulator's profile declares firmware targets, so a pass started from this
 * button can still end with nothing installed — which is why the button
 * reads "Requested…" rather than claiming progress, and re-enables on
 * `FIRMWARE_PASS_FINISHED_EVENT` whether or not anything was fetched.
 */
export function firmwareInstallable(status: FirmwareChipState): boolean {
  return (
    status !== null &&
    status !== 'unavailable' &&
    status.file_count > 0 &&
    status.has_default_emulator
  );
}
