// Which Library rail entry is selected. Module scoped, like
// `appUpdate.svelte.ts`, because design §5 says the selection "persists per
// session": the Library view unmounts nothing today, but a Shell remount
// (a reconnect) must not silently throw the user back to All games.
import type { RailKey } from './rail';
import { openRailGroupOf } from '../stores/uiSettings.svelte';

const state = $state<{ key: RailKey }>({ key: 'all' });

export const librarySelection = {
  get key(): RailKey {
    return state.key;
  },
};

export function selectRail(key: RailKey): void {
  state.key = key;
}

/**
 * A jump to a rail row from elsewhere (Settings' Show in Library, Open
 * Discover, Open Available, a notification): selects it and opens its group
 * when closed (RommStorefront README § Folding groups). Startup (Open GRID
 * on) and the RomM selection fallback use `selectRail`, which never opens a
 * group.
 */
export function jumpToRail(key: RailKey): void {
  openRailGroupOf(key);
  state.key = key;
}
