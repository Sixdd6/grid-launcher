// Where TV mode starts a backup (README § Game backups: Details › Backup to
// drive…, select mode's Y, Backup all N…): every way opens the one Backup
// page with the desktop's request and title (`backups/dialog.ts`).
import { backupRequest, type BackupSubject } from '../backups/dialog';
import { pushPage } from './nav.svelte';
import TvBackupPage from './pages/TvBackupPage.svelte';

/**
 * Pushes the Backup page for `romIds` (titled "Backup <title>", "Backup 3
 * games" or "Backup <Platform> · 3 games"). With RomM off it opens too
 * (RommStorefront README, "Backups without RomM"): the page copies the
 * games installed on this computer and lists the ones it can't.
 */
export function openBackupPage(subject: BackupSubject, romIds: readonly number[], platformIds: readonly number[] = []): void {
  const request = backupRequest(subject, romIds, platformIds);
  void pushPage({ title: request.title, component: TvBackupPage, props: { open: { request } } });
}
