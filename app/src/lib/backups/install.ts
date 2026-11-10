// Install from a drive and Remove from drive (GameBackups README, "Library ›
// Backup drives"), shared by the drive grid, its select bar and Details.
// The game's own queued and installed cards come from the downloads
// snapshot like any install; this adds the lines `install_from_drive`
// answers with (the emulator and firmware it set up from the drive), and
// the refusal or removal cards.
import { api } from '../api';
import { notify, quietQueued, unquiet } from '../notifications/notifier';
import { driveOutcomeNotice, queueRefusedNotice, summaryText, type Notice } from '../notifications/notices';
import { refresh as refreshDrives } from '../stores/drives.svelte';
import { removedFromDriveText } from '../library/drive';

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

type DriveGame = { rom_id: number; name: string; server: string };

/** Copies one game from the drive into the library. Answers whether it was queued. */
export async function installFromDrive(key: string, game: DriveGame): Promise<boolean> {
  try {
    const outcome = await api.installFromDrive(key, game.rom_id, game.server);
    for (const line of outcome.notices) notify(driveOutcomeNotice(line));
    return true;
  } catch (err) {
    notify(queueRefusedNotice(game.name, message(err)));
    return false;
  }
}

/**
 * The one card for a batch install from a drive (item 450): "<n> games added
 * to download queue" ("<title> added…" for one), "Copying from <label>".
 */
export function batchQueuedNotice(titles: readonly string[], label: string): Notice {
  const text = titles.length === 1 ? `${titles[0]} added to download queue` : summaryText('game-queued', titles.length);
  return { group: 'game-queued', text, level: 'success', action: 'downloads', sub: `Copying from ${label}` };
}

/**
 * Installs each game in turn, so the queue keeps the picked order. With
 * `oneCard` (TV mode's select mode, item 450) the games' own queued cards
 * are held back and one card stands for those that queued, with the drive's
 * label, however slowly each call returns; without it each game's card
 * comes from the queue as usual (merged when they arrive together).
 */
export async function installAllFromDrive(key: string, games: readonly DriveGame[], oneCard?: { label: string }): Promise<void> {
  if (!oneCard) {
    for (const game of games) await installFromDrive(key, game);
    return;
  }
  quietQueued(games.map((g) => g.rom_id));
  const queued: string[] = [];
  for (const game of games) {
    if (await installFromDrive(key, game)) queued.push(game.name);
    else unquiet(game.rom_id);
  }
  if (queued.length > 0) notify(batchQueuedNotice(queued, oneCard.label));
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
