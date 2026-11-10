// Backup drives (plan §1.3). Holds the latest `DrivesSnapshot`: loaded once
// with `list_drives`, then replaced by every `drives-changed` event (the
// backend's 2 s scan emits only when the snapshot changes). Also turns the
// backend's `backup-notice` toasts into notification cards. `init()` runs
// while the session phase is `shell` (App.svelte), like the other stores.
//
// Reads of the snapshot are pure functions in `backups/drives.ts`; the
// getters here apply them to the live snapshot.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  BACKUP_NOTICE_EVENT,
  DRIVES_CHANGED_EVENT,
  type BackupNotice,
  type DriveInfo,
  type DrivesSnapshot,
  type RememberedDrive,
} from '../api';
import {
  driveByKey,
  drivesHoldingRom,
  EMPTY_SNAPSHOT,
  firstLibraryDriveKey,
  packDrives,
  readyDrives,
} from '../backups/drives';
import { backupNoticeCard } from '../notifications/notices';
import { notify } from '../notifications/notifier';

// `loaded` stays false until the first snapshot lands, so a pane can tell
// "no drives" from "not asked yet".
const state = $state<{ snapshot: DrivesSnapshot; loaded: boolean }>({ snapshot: EMPTY_SNAPSHOT, loaded: false });

export const drives = {
  get snapshot(): DrivesSnapshot {
    return state.snapshot;
  },
  get loaded(): boolean {
    return state.loaded;
  },
  /** Every connected drive, set up or not. */
  get connected(): DriveInfo[] {
    return state.snapshot.connected;
  },
  /** Set-up drives GRID has seen that are not connected now. */
  get remembered(): RememberedDrive[] {
    return state.snapshot.remembered;
  },
  /** Connected, set-up, writable drives: backup targets. */
  get ready(): DriveInfo[] {
    return readyDrives(state.snapshot);
  },
  /** Connected drives with a readable pack (read-only ones too): install sources, Library rows. */
  get withPack(): DriveInfo[] {
    return packDrives(state.snapshot);
  },
  /** The drive key the offline shell opens Library on, or `null`. */
  get firstLibraryDrive(): string | null {
    return firstLibraryDriveKey(state.snapshot);
  },
  byKey(key: string): DriveInfo | null {
    return driveByKey(state.snapshot, key);
  },
  /** Connected drives holding `romId` from `serverUrl` (Details' "On <pack>"). */
  holdingRom(serverUrl: string, romId: number): DriveInfo[] {
    return drivesHoldingRom(state.snapshot, serverUrl, romId);
  },
};

function apply(snapshot: DrivesSnapshot): void {
  state.snapshot = snapshot;
  state.loaded = true;
}

/** Pulls the snapshot now (after a command that changed a drive, say). */
export async function refresh(): Promise<void> {
  apply(await api.listDrives());
}

export async function init(): Promise<UnlistenFn> {
  // Listeners FIRST (same race as compatTools.svelte.ts): a scan can emit
  // while the initial pull is in flight. Once an event has arrived, its
  // snapshot is newer than the pull's, so the pull's result is dropped.
  let sawEvent = false;
  const offChanged = await listen<DrivesSnapshot>(DRIVES_CHANGED_EVENT, (e) => {
    sawEvent = true;
    apply(e.payload);
  });
  const offNotice = await listen<BackupNotice>(BACKUP_NOTICE_EVENT, (e) => {
    notify(backupNoticeCard(e.payload));
  });
  const pulled = await api.listDrives().catch(() => null);
  if (pulled !== null && !sawEvent) apply(pulled);
  return () => {
    offChanged();
    offNotice();
  };
}
