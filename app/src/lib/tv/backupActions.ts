// Where TV mode starts a backup (README § Game backups: Details › Backup to
// drive…, select mode's Y, Backup all N…): every way opens the one Backup
// page with the desktop's request and title (`backups/dialog.ts`).
import { backupRequest, type BackupSubject } from '../backups/dialog';
import { session } from '../stores/session.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { pushPage } from './nav.svelte';
import TvBackupPage from './pages/TvBackupPage.svelte';
import { BACKUP_NEEDS_SERVER } from './select.svelte';

/**
 * Pushes the Backup page for `romIds` (titled "Backup <title>", "Backup 3
 * games" or "Backup <Platform> · 3 games"). Offline it says "Backing up
 * needs a server." instead (items 437, 448, 456).
 */
export function openBackupPage(subject: BackupSubject, romIds: readonly number[], platformIds: readonly number[] = []): void {
  if (!session.connected) {
    pushToast(BACKUP_NEEDS_SERVER, 'error');
    return;
  }
  const request = backupRequest(subject, romIds, platformIds);
  void pushPage({ title: request.title, component: TvBackupPage, props: { open: { request } } });
}
