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
