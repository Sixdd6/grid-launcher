// Pure helpers for the Emulators panel's RetroAchievements block
// (task-12-brief.md). No store/API imports here so this stays trivially
// unit-testable.
import type { RaFanOutRow, RaStatus } from '../api';

/** Both fields non-blank after trim — the frontend's save-button gate. */
export function canSubmit(username: string, token: string): boolean {
  return username.trim() !== '' && token.trim() !== '';
}

/** The status line: `Not set`, or `Set for <username>` once a token exists. */
export function statusLabel(status: RaStatus | null): string {
  if (!status || !status.token_present) return 'Not set';
  return `Set for ${status.username}`;
}

/**
 * The fan-out result line: the names of the rows the backend actually
 * changed, comma-joined (`Updated: RetroArch, PPSSPP`), or `No changes`
 * when none did.
 */
export function fanOutSummary(rows: RaFanOutRow[]): string {
  const changed = rows.filter((r) => r.changed).map((r) => r.emulator);
  return changed.length === 0 ? 'No changes' : `Updated: ${changed.join(', ')}`;
}

/**
 * `_ra_login_clicked`'s gate (grid-launcher.py:2708-2712): a non-blank
 * username and a non-empty password. The password is checked for emptiness,
 * not blankness — Python reads `text()` without stripping it, and a password
 * made of spaces is a legal password.
 */
export function canLogin(username: string, password: string): boolean {
  return username.trim() !== '' && password !== '';
}

/** grid-launcher.py:2711, verbatim. */
export const LOGIN_MISSING_FIELDS_TOAST = 'Enter both username and password.';

/** grid-launcher.py:2767, verbatim. */
export const CREDENTIALS_CLEARED_TOAST = 'RetroAchievements credentials cleared.';

/** grid-launcher.py:2750, verbatim. Takes the server-reported account name. */
export function loginToast(username: string): string {
  return `Logged in as ${username}`;
}

/** grid-launcher.py:2736, verbatim. */
export function loginFailedToast(error: string): string {
  return `RA login failed: ${error}`;
}
