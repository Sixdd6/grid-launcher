// Every game action both shells offer, as one effectful call each: the
// command, the card it announces, and the registry refresh after it. Each
// answers an outcome instead of throwing, so the desktop and TV each show
// their own prompt for it (the umu-launcher offer, the library-folder
// banner, an error line). Cards that announce the action or its refusal are
// sent here (through `notifier.ts`), never by the caller.
import { api, type ContentKind } from '../api';
import { installFromDrive as copyFromDrive } from '../backups/install';
import { isUmuMissing } from '../launchError';
import { needsLibraryFolder } from '../library/installRefusal';
import { installGame, launchGame, notify } from '../notifications/notifier';
import { removedFromLibraryNotice, uninstalledNotice } from '../notifications/notices';
import { refresh as refreshInstalled } from '../stores/installed.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { SKIPPED_TOAST_MS, skippedRemovalText } from '../uninstallSkips';

export type RunOutcome =
  | { kind: 'ok' }
  /** A Proton launch found no umu-run: offer the umu-launcher install. */
  | { kind: 'needs-umu'; reason: string }
  /** No library folder is set: offer the library-folder prompt. */
  | { kind: 'needs-library-folder'; reason: string }
  /** Any other refusal, with the backend's reason. */
  | { kind: 'refused'; reason: string };

const OK: RunOutcome = { kind: 'ok' };

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Reads a failed command into the outcome its screen reacts to. */
export function outcomeOf(err: unknown): RunOutcome {
  const reason = message(err);
  if (isUmuMissing(err)) return { kind: 'needs-umu', reason };
  if (needsLibraryFolder(reason)) return { kind: 'needs-library-folder', reason };
  return { kind: 'refused', reason };
}

/** Runs `step`, answering `ok` or the outcome of its failure. */
async function attempt(step: () => Promise<unknown>): Promise<RunOutcome> {
  try {
    await step();
    return OK;
  } catch (err) {
    return outcomeOf(err);
  }
}

/** Launches an installed game. "Launching <title>…" and a refusal are cards;
 *  with `platform`, a refusal for want of an emulator offers that system's
 *  page on the card (TV mode). */
export function playInstalled(romId: number, title: string, platform?: string): Promise<RunOutcome> {
  return attempt(() => launchGame(romId, title, platform));
}

/** Queues a server game's install. The queued card comes from the downloads
 *  snapshot; a refusal is a card. */
export function installServerGame(romId: number, title: string): Promise<RunOutcome> {
  return attempt(() => installGame(romId, title));
}

/** Copies a game from a backup drive into the library. The drive's own
 *  lines and a refusal are cards (`backups/install.ts`). */
export async function installFromDrive(
  key: string,
  game: { rom_id: number; name: string; server: string },
): Promise<RunOutcome> {
  const queued = await copyFromDrive(key, game);
  return queued ? OK : { kind: 'refused', reason: '' };
}

/** Cancels the live install for a rom. An install already extracting is
 *  left alone by the backend. */
export function cancelInstall(romId: number): Promise<RunOutcome> {
  return attempt(() => api.cancelDownloadForRom(romId));
}

/** Queues the server's update for an installed game. Its cards come from
 *  the downloads snapshot. */
export function updateGame(romId: number): Promise<RunOutcome> {
  return attempt(() => api.updateGame(romId));
}

/** Queues a PS4 / Xbox 360 update or DLC for an installed game. */
export function installContent(romId: number, kind: ContentKind): Promise<RunOutcome> {
  return attempt(() => api.installContent(romId, kind));
}

/** Uninstalls a game: the card, a card for folders kept on disk, then the
 *  registry refresh. */
export function uninstallGame(romId: number, title: string): Promise<RunOutcome> {
  return attempt(async () => {
    const report = await api.uninstallGame(romId);
    notify(uninstalledNotice(title));
    pushToast(skippedRemovalText(report?.skipped), 'error', SKIPPED_TOAST_MS);
    await refreshInstalled();
  });
}

/** Drops a registry row and keeps its files: the card, then the refresh. */
export function removeFromLibrary(title: string, platform: string): Promise<RunOutcome> {
  return attempt(async () => {
    await api.removeFromLibrary(title, platform);
    notify(removedFromLibraryNotice(title));
    await refreshInstalled();
  });
}

/** Points a missing row at the folder that holds its files. */
export function locateGame(title: string, platform: string, folder: string): Promise<RunOutcome> {
  return attempt(async () => {
    await api.locateInstalledRow(title, platform, folder.trim());
    await refreshInstalled();
  });
}

/** Links an installed row with no server id to a server game. The opener
 *  sees the row gain its id in the refreshed registry. */
export function linkGame(title: string, platform: string, romId: number): Promise<RunOutcome> {
  return attempt(async () => {
    await api.linkInstalledRow(title, platform, romId);
    await refreshInstalled();
  });
}

/** Stops a running game's session. */
export function stopGame(sessionId: number): Promise<RunOutcome> {
  return attempt(() => api.stopGame(sessionId));
}
