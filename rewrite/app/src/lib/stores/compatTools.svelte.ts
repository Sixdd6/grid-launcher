// Compatibility-tools registry (task-17-brief.md). Mirrors installed.svelte.ts's
// shape: a plain `$state` snapshot behind getters, an exported `refresh()`,
// an `init()` that listens for server-pushed changes and then loads once, and
// a module-scoped `$effect.root` that refreshes on its own whenever a
// `compat_tool`-kind drawer entry completes (a managed compat-tool install
// finishing) — so nothing outside this module has to remember to wire that
// up. `COMPAT_TOOLS_CHANGED_EVENT` (api.ts) already covers the same install
// finishing in the background AND `setDefaultCompatTool`, so the two refresh
// paths overlap on purpose rather than one replacing the other.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  COMPAT_TOOLS_CHANGED_EVENT,
  type CompatTool,
  type CompatToolsDto,
  type DownloadEntry,
} from '../api';
import { downloads } from './downloads.svelte';

const state = $state<{ tools: CompatTool[]; defaultTool: string }>({ tools: [], defaultTool: '' });

export const compatTools = {
  get tools() {
    return state.tools;
  },
  get defaultTool() {
    return state.defaultTool;
  },
};

function apply(dto: CompatToolsDto): void {
  state.tools = dto.tools;
  state.defaultTool = dto.default_tool;
}

export async function refresh(): Promise<void> {
  apply(await api.listCompatTools());
}

export async function init(): Promise<UnlistenFn> {
  // Listener FIRST (same race as updates.svelte.ts): a managed compat-tool
  // install finishing in the background can emit while the initial pull is
  // still in flight, and a refresh-then-listen order drops it. A redundant
  // refresh is harmless; a missed event is not.
  let sawEvent = false;
  const unlisten = await listen(COMPAT_TOOLS_CHANGED_EVENT, () => {
    sawEvent = true;
    refresh();
  });
  // ...and the initial pull can RESOLVE after an event's own refresh has
  // already landed, which would put the older snapshot back. Once an event
  // has arrived, its refresh wins and this result is dropped.
  const pulled = await api.listCompatTools().catch(() => null);
  if (pulled !== null && !sawEvent) apply(pulled);
  return unlisten;
}

const previousStatuses = new Map<number, DownloadEntry['status']>();

/**
 * Exported so the transition-watching logic itself is a plain, callable
 * function (mirrors installed.svelte.ts's `watchDownloads`): refreshes the
 * compat-tools registry whenever a `compat_tool`-kind entry in the given
 * snapshot just transitioned into 'completed'.
 */
export function watchDownloads(entries: DownloadEntry[]): void {
  let justCompleted = false;
  const seen = new Set<number>();
  for (const entry of entries) {
    seen.add(entry.id);
    const prev = previousStatuses.get(entry.id);
    if (entry.kind === 'compat_tool' && entry.status === 'completed' && prev !== 'completed') {
      justCompleted = true;
    }
    previousStatuses.set(entry.id, entry.status);
  }
  for (const id of [...previousStatuses.keys()]) {
    if (!seen.has(id)) previousStatuses.delete(id);
  }
  if (justCompleted) refresh();
}

// Module-scoped root effect: keeps the compat-tools registry in sync with the
// downloads store for the lifetime of the app, without requiring App.svelte
// or CompatTools.svelte to remember to wire it up.
$effect.root(() => {
  $effect(() => {
    watchDownloads(downloads.entries);
  });
});
