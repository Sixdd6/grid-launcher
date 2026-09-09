// The "Update from Source" texts, verbatim from the reference's version
// check dialogs (`emulator_ui_mixin.py:1323-1370`) and its post-install
// toast (`install_mixin.py:1690-1716`). Pure, so vitest owns the wording.
import type { VersionCheck } from '../api';

/** The Yes/No question the row's second click confirms (`:1360-1366`). */
export function updateConfirmText(name: string, check: VersionCheck): string {
  return `Update ${name}?\n\nInstalled: ${check.installed_display}\nAvailable: ${check.available_display}`;
}

/** Shown when the installed pin already equals the newest release (`:1352`). */
export function upToDateText(check: VersionCheck): string {
  return `Already up to date (${check.available_display}).`;
}

/** The completion toast: an update over an existing entry reads differently. */
export function installedToastText(name: string, fresh: boolean): string {
  return fresh
    ? `Installed emulator '${name}' from source.`
    : `Updated emulator '${name}' from source.`;
}
