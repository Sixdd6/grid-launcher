// The one Backup dialog (GameBackups README, "Three ways to start a
// backup, all opening the same dialog"). Details, the select bar and a
// platform header call `openBackup`; `Shell.svelte` hosts the dialog once,
// above every view and over Details, and routes "Set up a drive…" to
// Settings › Backup drives.
import type { BackupRequest } from '../api';

export type BackupDialogOpen = {
  request: BackupRequest;
  /** Runs once the backup is queued: the select bar clears its selection. */
  onQueued?: () => void;
  /** Runs when the dialog sends the user elsewhere ("Set up a drive…"):
   *  Details closes, so it is not left open behind Settings. */
  onLeave?: () => void;
};

const state = $state<{ current: BackupDialogOpen | null }>({ current: null });

export const backupDialog = {
  get current(): BackupDialogOpen | null {
    return state.current;
  },
};

export function openBackup(open: BackupDialogOpen): void {
  state.current = open;
}

export function closeBackup(): void {
  state.current = null;
}
