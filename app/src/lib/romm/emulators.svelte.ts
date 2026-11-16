// Library › RomM › Emulators: the server's Emulators platform list, shared
// by the desktop and TV Libraries (the rail row's count and the view), and
// the view's actions. The list loads while RomM is connected and the
// server has an Emulators platform; it reloads when an install from it
// finishes and when the view opens.
import { api, type DownloadEntry, type RommEmulator, type RommOtherFile } from '../api';
import { onSnapshot } from '../stores/downloads.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { expectInstalledCard, expectQueued } from '../notifications/notifier';
import {
  emulatorInstalledCard,
  latestEntry,
  otherFileInstalledCard,
  otherFileSourceId,
  type EmulatorsLoad,
} from './emulators';

const state = $state<{ load: EmulatorsLoad | null; server: string }>({ load: null, server: '' });

let inflight: Promise<void> | null = null;
let again = false;

export const rommEmulators = {
  /** `null` before a load started (and after `reset`). */
  get load(): EmulatorsLoad | null {
    return state.load;
  },
};

async function run(): Promise<void> {
  const server = state.server;
  try {
    const data = await api.rommEmulators();
    if (state.server === server) state.load = { phase: 'ready', data };
  } catch (err) {
    if (state.server === server) state.load = { phase: 'error', message: err instanceof Error ? err.message : String(err) };
  }
}

/**
 * Loads (or reloads) the list for `server`. A first load, or one for another
 * server, shows Loading; a reload keeps the list on show until it answers.
 * `showLoading` puts Loading up for a reload too (the view's Retry).
 */
export function loadEmulators(server: string, showLoading = false): Promise<void> {
  if (state.server !== server || state.load === null || showLoading) {
    state.server = server;
    state.load = { phase: 'loading' };
  }
  if (inflight) {
    again = true;
    return inflight;
  }
  inflight = (async () => {
    do {
      again = false;
      await run();
    } while (again);
    inflight = null;
  })();
  return inflight;
}

/** Loads once for `server` (nothing when that server's list is on show or loading). */
export function ensureEmulators(server: string): void {
  if (state.server === server && state.load !== null) return;
  void loadEmulators(server);
}

/** RomM dropped, or the server has no Emulators platform: forget the list. */
export function resetEmulators(): void {
  state.load = null;
  state.server = '';
}

// An install or update from the list finished: its answer changed.
const seen = new Map<number, string>();
onSnapshot((entries: DownloadEntry[], first: boolean) => {
  let changed = false;
  for (const e of entries) {
    const from = seen.get(e.id);
    seen.set(e.id, e.status);
    // The first snapshot is a baseline; a row first seen already finished
    // (a fast install between two snapshots) still counts.
    if (first || from === e.status) continue;
    if (e.status === 'completed' && (e.source_id.startsWith('romm:') || e.source_id.startsWith('romm-file:'))) changed = true;
  }
  if (changed && state.load !== null && state.server !== '') void loadEmulators(state.server);
});

// ---- actions -----------------------------------------------------------------

function errText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Install or Update an emulator row. A refusal (nothing queued) is a card. */
export async function installEmulator(e: RommEmulator, entries: readonly DownloadEntry[]): Promise<void> {
  const update = e.entry !== null;
  const card = emulatorInstalledCard(e, update);
  expectQueued(e.source_id, { label: e.name, quietQueued: true, update });
  expectInstalledCard(e.entry ?? e.install_as, { group: update ? 'tool-updated' : 'tool-installed', level: 'success', ...card });
  const failed = latestEntry(entries, e.source_id);
  try {
    if (failed?.status === 'failed') await api.retryInstall(failed.id);
    else await api.rommEmulatorInstall(e);
  } catch (err) {
    pushToast(`Couldn't install ${e.name}: ${errText(err)}`, 'error');
  }
}

export async function installOtherFile(f: RommOtherFile, entries: readonly DownloadEntry[]): Promise<void> {
  const sourceId = otherFileSourceId(f);
  const card = otherFileInstalledCard(f);
  expectQueued(sourceId, { label: f.file_name, quietQueued: true, done: card.text, doneTv: card.tvText, doneSub: card.sub });
  const failed = latestEntry(entries, sourceId);
  try {
    if (failed?.status === 'failed') await api.retryInstall(failed.id);
    else await api.rommEmulatorFileInstall(f);
  } catch (err) {
    pushToast(`Couldn't install ${f.file_name}: ${errText(err)}`, 'error');
  }
}

/** Cancel a queued or downloading row ("Cancelled PCSX2" is the queue's card). */
export async function cancelRow(sourceId: string, label: string, entries: readonly DownloadEntry[]): Promise<void> {
  const live = latestEntry(entries, sourceId);
  if (live === null) return;
  expectQueued(sourceId, { label, quietQueued: true });
  try {
    await api.cancelInstall(live.id);
  } catch (err) {
    pushToast(`Couldn't cancel ${label}: ${errText(err)}`, 'error');
  }
}

/** Uninstall an emulator: Settings › Systems' delete, then the list again. */
export async function uninstallEmulator(e: RommEmulator, server: string): Promise<void> {
  if (e.entry === null) return;
  try {
    await api.deleteEmulator(e.entry);
    pushToast(`${e.name} uninstalled`);
  } catch (err) {
    pushToast(`Couldn't uninstall ${e.name}: ${errText(err)}`, 'error');
  }
  await loadEmulators(server);
}

export async function uninstallOtherFile(f: RommOtherFile, server: string): Promise<void> {
  try {
    await api.rommEmulatorFileUninstall(f);
    pushToast(`${f.file_name} uninstalled`);
  } catch (err) {
    pushToast(`Couldn't uninstall ${f.file_name}: ${errText(err)}`, 'error');
  }
  await loadEmulators(server);
}

export function openPackagingGuide(): void {
  api.openPackagingGuide().catch((err) => pushToast(`Couldn't open the guide: ${errText(err)}`, 'error'));
}

export function openEmulatorsFolder(): void {
  api.openEmulatorsFolder().catch((err) => pushToast(`Couldn't open the Emulators folder: ${errText(err)}`, 'error'));
}

/** Tests only. */
export function resetEmulatorsForTest(): void {
  state.load = null;
  state.server = '';
  inflight = null;
  again = false;
  seen.clear();
}
