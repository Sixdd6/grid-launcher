// Pure helpers for the Emulators panel's "Per-platform defaults" select.
// No store/API imports here so this stays trivially unit-testable.
import type { LaunchDefaults } from '../api';

/** The `<select>` value that means "no default emulator for this platform". */
export const NO_DEFAULT_VALUE = '';

/** What one platform row's `<select>` renders. */
export type PlatformDefaultSelect = {
  /** The `<option>` values, in backend (config) order. */
  options: string[];
  /** The value shown selected — always one of `options`, or [`NO_DEFAULT_VALUE`]. */
  selected: string;
  /** True when no configured emulator supports the platform. */
  disabled: boolean;
};

/**
 * The emulator name saved as `platformName`'s default, or `''`. The platform
 * lookup is case-insensitive (server platform-name casing varies).
 */
function savedDefaultFor(defaults: LaunchDefaults | null, platformName: string): string {
  if (!defaults) return '';
  const folded = platformName.toLowerCase();
  const key = Object.keys(defaults.default_emulators).find((k) => k.toLowerCase() === folded);
  return key ? defaults.default_emulators[key] : '';
}

/**
 * The option list and selected value for `platformName`'s default-emulator
 * select. `compatibleNames` is the backend's `compatible_emulators` answer
 * for that platform, so only emulators that support it are ever offered.
 *
 * The selection mirrors `default_emulator_name_for_platform` (doc 04 §2):
 * the saved default when it is still compatible, otherwise the first
 * compatible name. Names are matched verbatim against `compatibleNames`,
 * which carry the `<option>` values' exact casing. This is DISPLAY only —
 * falling back never writes the fallback to the config.
 */
export function platformDefaultSelect(
  defaults: LaunchDefaults | null,
  platformName: string,
  compatibleNames: string[]
): PlatformDefaultSelect {
  const saved = savedDefaultFor(defaults, platformName);
  const selected = compatibleNames.includes(saved)
    ? saved
    : (compatibleNames[0] ?? NO_DEFAULT_VALUE);
  return { options: compatibleNames, selected, disabled: compatibleNames.length === 0 };
}
