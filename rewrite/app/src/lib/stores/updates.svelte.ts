// Server-update set (doc 10). Mirrors compatTools.svelte.ts: a `$state`
// snapshot behind getters, `refresh()` via the command, `init()` = refresh
// then listen. The event payload IS the new row list, so the listener
// applies it directly instead of re-fetching.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, UPDATES_CHANGED_EVENT, type UpdateRow } from '../api';

const state = $state<{ rows: UpdateRow[] }>({ rows: [] });

/** Pure, exported for tests: the button label for `romId`, or null when it has no update. */
export function labelFor(rows: UpdateRow[], romId: number | null): string | null {
  if (romId === null) return null;
  return rows.find((r) => r.rom_id === romId)?.label ?? null;
}

export const updates = {
  get rows() {
    return state.rows;
  },
  labelFor(romId: number | null): string | null {
    return labelFor(state.rows, romId);
  },
};

export async function refresh(): Promise<void> {
  state.rows = await api.listUpdates();
}

export async function init(): Promise<UnlistenFn> {
  await refresh().catch(() => {});
  return listen<UpdateRow[]>(UPDATES_CHANGED_EVENT, (e) => {
    state.rows = e.payload;
  });
}
