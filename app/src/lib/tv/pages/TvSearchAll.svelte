<script lang="ts">
  // Search everything's results (items 353–355, ruling 17): "<n> results
  // for “<text>”" over a grid of covers with platform tags, or "Nothing
  // matches. Try fewer letters." Y reopens the keyboard, and from then on
  // the results follow every key. While every system is being asked the
  // heading reads "Searching for “<text>”…" over an empty grid; when nothing
  // could be searched, the offline panel's words and Try again; when only
  // some systems answered, a warning line and Try again above the results
  // (consultant rulings). Failures are inline, never cards.
  import { installed, isInstalled, matchesInstalled } from '../../stores/installed.svelte';
  import { session } from '../../stores/session.svelte';
  import { updates } from '../../stores/updates.svelte';
  import { gameFromSummary } from '../games';
  import { nav, setPageTitle } from '../nav.svelte';
  import { openKeyboard } from '../overlays/overlays.svelte';
  import { useScreen } from '../screen.svelte';
  import { loadAllGames, type AllGames } from '../tabs/server/server.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvGameCover from '../ui/TvGameCover.svelte';
  import TvGrid from '../ui/TvGrid.svelte';
  import {
    LOAD_FAILED_TITLE,
    loadFailedLine,
    NO_RESULTS,
    PARTIAL_NOTE,
    RETRY,
    resultsHeading,
    SEARCH_ALL_TITLE,
    searchGames,
    searchingHeading,
    searchPageTitle,
  } from './searchResults';

  let { query }: { query: string } = $props();

  const pageId = nav.topPage?.id ?? null;
  // svelte-ignore state_referenced_locally
  let q = $state(query);
  let phase = $state.raw<'loading' | 'failed' | AllGames>('loading');

  function load() {
    phase = 'loading';
    loadAllGames().then(
      (all) => (phase = all),
      () => (phase = 'failed'),
    );
  }
  load();

  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let results = $derived(
    typeof phase === 'string'
      ? []
      : searchGames(phase.games, q).map(({ game, platform, slug }) => {
          const row = isInstalled(game, platform) ? (installed.list.find((r) => matchesInstalled(r, game, platform)) ?? null) : null;
          return gameFromSummary(game, platform, slug, row, updateIds);
        }),
  );

  function setQuery(v: string) {
    // On the results page an emptied field is "nothing typed": the results,
    // heading and breadcrumb keep the last text, and Done just closes the
    // keyboard (consultant ruling; item 354).
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
    <div class="panel" id="sr-panel">
      {#if phase === 'failed'}
        <div class="empty" data-testid="tv-search-failed">
          <h2>{LOAD_FAILED_TITLE}</h2>
          <p>{loadFailedLine(session.serverUrl)}</p>
          <div class="btns"><TvButton k="sr-retry" primary autofocus testId="tv-search-retry" onA={load}>{RETRY}</TvButton></div>
        </div>
      {:else}
        <h2 data-testid="tv-search-heading">{phase === 'loading' ? searchingHeading(q) : resultsHeading(results.length, q)}</h2>
        {#if typeof phase !== 'string' && phase.partial}
          <div class="bar-h" data-testid="tv-search-partial">
            <p class="note warn sr-warn">{PARTIAL_NOTE}</p>
            <TvButton k="sr-retry" testId="tv-search-retry" onA={load}>{RETRY}</TvButton>
          </div>
        {/if}
        <TvGrid id="sr-grid" items={results} key={(g) => `sr${g.id}`} cover={(g) => g.cover} listKey={q}>
          {#snippet cell(g)}
            <TvGameCover k={`sr${g.id}`} game={g} plat testId={`tv-cover-sr${g.id}`} />
          {/snippet}
          {#if results.length === 0 && phase !== 'loading'}<p class="note" data-testid="tv-search-none">{NO_RESULTS}</p>{/if}
        </TvGrid>
      {/if}
    </div>
  </div>
</div>
