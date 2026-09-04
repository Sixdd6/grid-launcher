// Pure helpers for the Emulators panel's "Per-platform defaults" select.
// No store/API imports here so this stays trivially unit-testable.
import type { LaunchDefaults } from '../api';

/** The `<select>` value that means "no default emulator for this platform". */
export const NO_DEFAULT_VALUE = '';

/**
 * The reserved `default_emulators` value the backend stores when the user
 * picks "(none)" for a platform (grid-core `launch::selection::NO_EMULATOR`).
 * It is a REMEMBERED choice, not an absent one, so it must never fall back
 * to the first compatible name.
 */
export const NO_EMULATOR_MARKER = '<none>';

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
 * [`NO_DEFAULT_VALUE`] when the saved value is [`NO_EMULATOR_MARKER`], else
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
  if (saved === NO_EMULATOR_MARKER) {
    return {
      options: compatibleNames,
      selected: NO_DEFAULT_VALUE,
      disabled: compatibleNames.length === 0,
    };
  }
  const selected = compatibleNames.includes(saved)
    ? saved
    : (compatibleNames[0] ?? NO_DEFAULT_VALUE);
  return { options: compatibleNames, selected, disabled: compatibleNames.length === 0 };
}

/** The `<select>` value that means "no core for this platform". */
export const NO_CORE_VALUE = '';

/**
 * Whether `name` is a RetroArch build. Mirrors the backend's
 * `is_retroarch_name` (crates/grid-core/src/launch/selection.rs): the name
 * contains "retroarch", case-insensitively.
 */
export function isRetroarchName(name: string): boolean {
  return name.toLowerCase().includes('retroarch');
}

/** What one platform row's core `<select>` renders. */
export type PlatformCoreSelect = {
  /** True only when the row's selected emulator is a RetroArch build. */
  visible: boolean;
  /** The installed compatible core ids, in backend order. */
  options: string[];
  /** The value shown selected — always one of `options`, or [`NO_CORE_VALUE`]. */
  selected: string;
  /** True when no compatible core is installed. */
  disabled: boolean;
};

/** The core saved for `platformName`, or `''`. Case-insensitive lookup. */
function savedCoreFor(defaults: LaunchDefaults | null, platformName: string): string {
  if (!defaults) return '';
  const folded = platformName.toLowerCase();
  const key = Object.keys(defaults.retroarch_cores).find((k) => k.toLowerCase() === folded);
  return key ? defaults.retroarch_cores[key] : '';
}

/**
 * The core select for `platformName`. `coreOptions` is the backend's
 * `retroarch_core_options` answer for that platform, so only cores actually
 * installed beside the RetroArch executable are ever offered.
 *
 * A saved core that is no longer installed shows the first option instead
 * (D-RC-5). This is DISPLAY only — falling back never writes the fallback.
 */
export function platformCoreSelect(
  defaults: LaunchDefaults | null,
  platformName: string,
  selectedEmulator: string,
  coreOptions: string[]
): PlatformCoreSelect {
  const saved = savedCoreFor(defaults, platformName);
  const selected = coreOptions.includes(saved) ? saved : (coreOptions[0] ?? NO_CORE_VALUE);
  return {
    visible: isRetroarchName(selectedEmulator),
    options: coreOptions,
    selected,
    disabled: coreOptions.length === 0,
  };
}
