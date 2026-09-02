// Pure helper for the Emulators panel's "Per-platform defaults" select
// (task-9-brief.md item 3). No store/API imports here so this stays
// trivially unit-testable.
import type { EmulatorEntry, LaunchDefaults } from '../api';

/** The `<select>` value that means "no default emulator for this platform". */
export const NO_DEFAULT_VALUE = '';

/**
 * The value to show selected in the per-platform default-emulator
 * `<select>` for `platformName`. The platform lookup in
 * `defaults.default_emulators` is case-insensitive (server platform-name
 * casing varies); the emulator name it resolves to is then matched against
 * `emulators` verbatim — exact, case-sensitive — since that's the identity
 * the `<option>` values use. A saved default that doesn't name a currently
 * configured emulator (deleted, renamed, or a stale casing) resolves to
 * [`NO_DEFAULT_VALUE`] rather than a value with no matching `<option>`.
 */
export function resolveDefaultEmulatorValue(
  defaults: LaunchDefaults | null,
  platformName: string,
  emulators: EmulatorEntry[]
): string {
  if (!defaults) return NO_DEFAULT_VALUE;

  const folded = platformName.toLowerCase();
  const key = Object.keys(defaults.default_emulators).find((k) => k.toLowerCase() === folded);
  const saved = key ? defaults.default_emulators[key] : '';
  if (!saved) return NO_DEFAULT_VALUE;

  return emulators.some((e) => e.name === saved) ? saved : NO_DEFAULT_VALUE;
}
