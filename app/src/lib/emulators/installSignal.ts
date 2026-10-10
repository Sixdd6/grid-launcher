import type { DownloadEntry } from '../api';

const TERMINAL_STATUSES: DownloadEntry['status'][] = ['completed', 'failed', 'cancelled'];

/**
 * Signature of every emulator install that has reached a terminal status.
 * The Emulators view reads it inside its `$effect`s: a new terminal entry
 * re-reads the entry list, the defaults and the catalog. Approximate on
 * purpose (task-7-brief.md): any terminal emulator entry is signal enough.
 *
 * Two rows count: a catalog install (`job === 'emulator'`) and a server
 * "Emulators"-platform package, which installs as a `game` job carrying
 * `kind === 'emulator'` and registers an emulator entry when it finishes.
 */
export function emulatorTerminalSignature(entries: DownloadEntry[]): string {
  return entries
    .filter((e) => (e.job === 'emulator' || e.kind === 'emulator') && TERMINAL_STATUSES.includes(e.status))
    .map((e) => `${e.id}:${e.status}`)
    .join(',');
}
