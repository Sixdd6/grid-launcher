// Which Library rail entry is selected. Module scoped, like
// `appUpdate.svelte.ts`, because design §5 says the selection "persists per
// session": the Library view unmounts nothing today, but a Shell remount
// (a reconnect) must not silently throw the user back to All games.
import type { RailKey } from './rail';

const state = $state<{ key: RailKey }>({ key: 'all' });

export const librarySelection = {
  get key(): RailKey {
    return state.key;
  },
};

export function selectRail(key: RailKey): void {
  state.key = key;
}
