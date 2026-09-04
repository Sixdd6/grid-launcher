// Server-update set (doc 10). Mirrors compatTools.svelte.ts: a `$state`
// snapshot behind getters, `refresh()` via the command, `init()` = listen
// then refresh. The event payload IS the new row list, so the listener
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

async function fetchRows(): Promise<UpdateRow[]> {
  return await api.listUpdates();
}

export async function refresh(): Promise<void> {
  state.rows = await fetchRows();
}

export async function init(): Promise<UnlistenFn> {
  // Listener FIRST: `connect` spawns the update pass before it returns, so a
  // refresh-then-listen order drops the `updates-changed` the pass emits in
  // between and the store stays empty for the rest of the process. A
  // redundant refresh is harmless; a missed event is not.
  let sawEvent = false;
  const unlisten = await listen<UpdateRow[]>(UPDATES_CHANGED_EVENT, (e) => {
    sawEvent = true;
    state.rows = e.payload;
  });
  // ...but the pull can also RESOLVE after an event that landed while the
  // command was in flight, which would replace the pushed rows with the older
  // snapshot. Once an event has arrived, its payload wins and the pull result
  // is dropped.
  const pulled = await fetchRows().catch(() => null);
  if (pulled !== null && !sawEvent) state.rows = pulled;
  return unlisten;
}
