// Search everything (Y on Server › Discover; items 353–355, ruling 17):
// the keyboard titled "Search everything"; Done with text pushes the
// results page, Done with nothing typed just closes the keyboard.
import { pushPage } from '../nav.svelte';
import { openKeyboard } from '../overlays/overlays.svelte';
import { loadAllGames } from '../tabs/server/server.svelte';
import TvSearchAll from './TvSearchAll.svelte';
import { SEARCH_ALL_TITLE, searchPageTitle } from './searchResults';

export function openSearchAll(): void {
  // Start reading the server's games while the player types.
  loadAllGames().catch(() => {});
  void openKeyboard({
    title: SEARCH_ALL_TITLE,
    onDone: (v) => {
      if (!v.trim()) return;
      void pushPage({ title: searchPageTitle(v), component: TvSearchAll, props: { query: v } });
    },
  });
}
