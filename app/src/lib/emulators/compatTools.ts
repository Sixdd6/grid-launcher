// Pure compat-tool helpers for Settings › Systems' Windows row
// (`systems/WindowsRow.svelte`) and the launch error's umu-launcher offer:
// the Proton catalog's terminal signature, the umu-run line, and the
// Windows-host guard. No store/API imports, so vitest owns the rules.
import type { DownloadEntry, UmuRunStatus } from '../api';

const TERMINAL_STATUSES: DownloadEntry['status'][] = ['completed', 'failed', 'cancelled'];
const LIVE_STATUSES: DownloadEntry['status'][] = ['queued', 'downloading', 'installing', 'cancelling'];

/**
 * Signature of every `compat_tool`-kind drawer entry that has reached a
 * terminal status — mirrors `emulatorTerminalSignature`
 * (`installSignal.ts`), scoped to `kind === 'compat_tool'` instead of
 * `job === 'emulator'`. Read inside a `$effect` (approximate on purpose,
 * same as the emulator catalog: any terminal compat_tool entry is signal
 * enough, not just the one just installed) so a fresh terminal entry — an
 * install completing, failing, or getting cancelled — triggers a catalog
 * re-fetch, keeping the Install/Installed buttons from going stale
 * (fix round 1 finding: the catalog previously only loaded once at mount).
 */
export function compatToolTerminalSignature(entries: DownloadEntry[]): string {
  return entries
    .filter((e) => e.kind === 'compat_tool' && TERMINAL_STATUSES.includes(e.status))
    .map((e) => `${e.id}:${e.status}`)
    .join(',');
}

/**
 * The Downloads row `source_id` a managed umu-launcher install runs under
 * (grid-core `launch::umu::UMU_SOURCE_ID`). `UmuRunStatus.source_id`
 * carries the same value; the launch-error offer has no status in hand, so
 * it reads this.
 */
export const UMU_SOURCE_ID = 'Open-Wine-Components/umu-launcher';

/**
 * The Windows row's umu-run status line: the system copy and its
 * path, GRID's own copy and its release, or nothing installed.
 */
export function umuStatusLabel(status: UmuRunStatus): string {
  switch (status.source) {
    case 'system':
      return `umu-run: system — ${status.path}`;
    case 'managed':
      return status.release_tag ? `umu-run: GRID-managed ${status.release_tag}` : 'umu-run: GRID-managed';
    default:
      return 'umu-run: not installed';
  }
}

/**
 * The status line's button: Install when there is no umu-run, Reinstall
 * (which fetches the newest release) for GRID's own copy, and none when the
 * system copy is in use — a launch would never reach GRID's.
 */
export function umuActionLabel(status: UmuRunStatus): 'Install' | 'Reinstall' | null {
  if (status.source === 'missing') return 'Install';
  if (status.source === 'managed') return 'Reinstall';
  return null;
}

/** Whether a umu-launcher Downloads row is live (queued through cancelling). */
export function umuInstallBusy(entries: DownloadEntry[], sourceId: string = UMU_SOURCE_ID): boolean {
  return entries.some((e) => e.kind === 'compat_tool' && e.source_id === sourceId && LIVE_STATUSES.includes(e.status));
}

// Re-exported rather than redefined: `isWindowsHost` already exists in
// details/actions.ts (task-16-brief.md) for the same purpose (reading
// `navigator.platform`, not the server's platform name). Exporting it from
// here too lets Settings › Systems import the Windows-host guard alongside
// the other compat-tool helpers without a second implementation to keep in
// sync.
export { isWindowsHost } from '../details/actions';
