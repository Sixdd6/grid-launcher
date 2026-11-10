<script lang="ts">
  // Search everything's results (items 353–355, ruling 17): "<n> results
  // for “<text>”" over a grid of covers with platform tags, or "Nothing
  // matches. Try fewer letters." Y reopens the keyboard, and from then on
  // the results follow every key.
  import { installed, isInstalled, matchesInstalled } from '../../stores/installed.svelte';
  import { updates } from '../../stores/updates.svelte';
  import { gameFromSummary } from '../games';
  import { nav, setPageTitle } from '../nav.svelte';
  import { openKeyboard } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import { loadAllGames, type LabelledGame } from '../tabs/server/server.svelte';
  import TvGameCover from '../ui/TvGameCover.svelte';
  import { NO_RESULTS, resultsHeading, SEARCH_ALL_TITLE, searchGames, searchPageTitle } from './searchResults';

  let { query }: { query: string } = $props();

  const pageId = nav.topPage?.id ?? null;
  // svelte-ignore state_referenced_locally
  let q = $state(query);
  let all = $state.raw<LabelledGame[] | null>(null);
  loadAllGames()
    .then((list) => (all = list))
    .catch(() => (all = []));

  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let results = $derived(
    searchGames(all ?? [], q).map(({ game, platform, slug }) => {
      const row = isInstalled(game, platform) ? (installed.list.find((r) => matchesInstalled(r, game, platform)) ?? null) : null;
      return gameFromSummary(game, platform, slug, row, updateIds);
    }),
  );

  function setQuery(v: string) {
    // An emptied field keeps the last results (Done with nothing typed
    // changes nothing, item 354).
    if (!v.trim()) return;
    q = v;
    if (pageId !== null) setPageTitle(pageId, searchPageTitle(v));
  }

  useScreen({
    hints: () => [{ button: 'Y', label: 'Search' }],
    y: () =>
      void openKeyboard({
        title: SEARCH_ALL_TITLE,
        value: q,
        onInput: setQuery,
        onDone: setQuery,
      }),
  });
</script>

<div class="list" data-scroll="list" data-testid="tv-search-all">
  <div class="list-in">
    {#if all !== null}
      <div class="panel" id="sr-panel">
        <h2 data-testid="tv-search-heading">{resultsHeading(results.length, q)}</h2>
        <div class="grid" data-nav="grid">
          {#each results as g (g.id)}
            <TvGameCover k={`sr${g.id}`} game={g} plat testId={`tv-cover-sr${g.id}`} />
          {:else}
            <p class="note" data-testid="tv-search-none">{NO_RESULTS}</p>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
