// The commands on one backup drive (GameBackups README, "Settings › Backup
// drives"): set up, rename, change its computers, measure and remove the
// GRID files, open its folder, forget a remembered one. The desktop
// DriveCard and the TV drive pages both call these. Each sends its card,
// then pulls the drives store again so every screen re-renders from the new
// snapshot. A refusal is thrown for the caller's error line.
import { api, type DriveInfo, type PackOs, type RememberedDrive } from '../api';
import { expectOsChangeBatch } from '../notifications/notifier';
import { refresh } from '../stores/drives.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { addedOses, osChanged, osChangeToasts } from './drives';

function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Makes `drive` a GRID pack named `name` for the given computers. */
export async function setUpDrive(drive: DriveInfo, name: string, oses: PackOs[]): Promise<void> {
  const trimmed = name.trim();
  await api.setupDrive(drive.mount, trimmed, oses);
  pushToast(`${trimmed} is ready for backups`);
  await refresh();
}

/** Renames the pack. Answers false (nothing sent) when the name is unchanged. */
export async function renameDrive(drive: DriveInfo, name: string): Promise<boolean> {
  const trimmed = name.trim();
  if (trimmed === drive.pack?.name) return false;
  await api.renameDrive(drive.key, trimmed);
  await refresh();
  return true;
}

/**
 * Changes which computers the pack serves. Answers false (nothing sent) when
 * the list is unchanged. Adding a computer needs a server; the backend's
 * refusal is thrown. The queued builds are announced by the backup batch's
 * own cards; the change itself by `osChangeToasts`.
 */
export async function changeDriveOs(drive: DriveInfo, next: PackOs[]): Promise<boolean> {
  const pack = drive.pack;
  if (!pack || !osChanged(pack.os, next)) return false;
  const before = [...pack.os];
  const after = [...next];
  const outcome = await api.setDriveOs(drive.key, after);
  expectOsChangeBatch(outcome, addedOses(before, after));
  for (const line of osChangeToasts(pack.name, before, after, outcome)) pushToast(line);
  await refresh();
  return true;
}

/** The bytes "Remove GRID files" frees, measured on the drive; the pack's
 *  own figure when the measurement fails. */
export function gridFilesSize(drive: DriveInfo): Promise<number> {
  return api.removeGridFilesSize(drive.key).catch(() => drive.pack?.gridpack_bytes ?? 0);
}

/** Removes the GRID files from the drive. */
export async function removeGridFiles(drive: DriveInfo): Promise<void> {
  await api.removeGridFiles(drive.key);
  pushToast(`GRID files removed from ${drive.label}`);
  await refresh();
}

/** Opens the drive's folder in the file manager; a refusal is a card. */
export function openDriveFolder(drive: DriveInfo): void {
  api.openDriveFolder(drive.key).catch((err) => pushToast(errorText(err), 'error'));
}

/** Forgets a remembered drive. */
export async function forgetDrive(remembered: RememberedDrive): Promise<void> {
  await api.forgetDrive(remembered.mount);
  await refresh();
}
