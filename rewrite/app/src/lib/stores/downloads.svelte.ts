import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadEntry, type DownloadsSnapshot } from '../api';

const LIVE_STATUSES = new Set(['queued', 'downloading', 'installing', 'cancelling']);

const state = $state<{ entries: DownloadEntry[] }>({ entries: [] });

export const downloads = {
  get entries() {
    return state.entries;
  },
  get hasLive() {
    return state.entries.some((e) => LIVE_STATUSES.has(e.status));
  },
};

export async function init(): Promise<UnlistenFn> {
  const snapshot = await api.listDownloads();
  state.entries = snapshot.entries;
  return listen<DownloadsSnapshot>('downloads-changed', (e) => {
    state.entries = e.payload.entries;
  });
}
