// Install from a drive and Remove from drive (GameBackups README, "Library ›
// Backup drives"), shared by the drive grid, its select bar and Details.
// The game's own queued and installed cards come from the downloads
// snapshot like any install; this adds the lines `install_from_drive`
// answers with (the emulator and firmware it set up from the drive), and
// the refusal or removal cards.
import { api } from '../api';
import { notify } from '../notifications/notifier';
import { driveQueuedNotice, queueRefusedNotice } from '../notifications/notices';
import { refresh as refreshDrives } from '../stores/drives.svelte';
import { removedFromDriveText } from '../library/drive';

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Copies one game from the drive into the library. Answers whether it was queued. */
export async function installFromDrive(key: string, game: { rom_id: number; name: string; server: string }): Promise<boolean> {
  try {
    const outcome = await api.installFromDrive(key, game.rom_id, game.server);
    for (const line of outcome.notices) notify(driveQueuedNotice(line));
    return true;
  } catch (err) {
    notify(queueRefusedNotice(game.name, message(err)));
    return false;
  }
}

/** Installs each game in turn, so the queue and the cards keep the picked order. */
export async function installAllFromDrive(
  key: string,
  games: readonly { rom_id: number; name: string; server: string }[],
): Promise<void> {
  for (const game of games) await installFromDrive(key, game);
}

/**
 * Removes the games from the drive's pack. Games from one server go in one
 * call; the card names the games and the pack. Throws the backend's refusal
 * (a backup is writing to the drive) for the caller to show.
 */
export async function removeFromDrive(
  key: string,
  packName: string,
  games: readonly { rom_id: number; name: string; server: string }[],
): Promise<void> {
  const byServer = new Map<string, number[]>();
  for (const g of games) byServer.set(g.server, [...(byServer.get(g.server) ?? []), g.rom_id]);
  for (const [server, ids] of byServer) await api.removeFromDrive(key, ids, server);
  notify({ group: 'removed', text: removedFromDriveText(games.map((g) => g.name), packName), level: 'success' });
  // The scan picks the new count up within 2 s; pull it now so the rail and
  // the header agree with the grid at once.
  await refreshDrives().catch(() => {});
}
