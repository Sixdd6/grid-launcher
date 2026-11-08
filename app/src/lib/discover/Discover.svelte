<script lang="ts" module>
  import type { DiscoverFeed as Feed } from '../api';

  /** The last feed shown, so coming back to Discover (it unmounts while a
   *  platform is selected) shows it at once while the fresh one loads. */
  let lastFeed: { server: string; feed: Feed } | null = null;
</script>

<script lang="ts">
  // Server › Discover: the first Server rail row and the default view.
  // New releases (a coverflow of the newest games on the preferred
  // platforms) and Recently added (newest on the server, minus the
  // coverflow's games), with platform filter pills across its top. The lists
  // come filtered and sorted from Rust (`discover_feed`, and
  // `discover_recently_added` for one platform); this view only shows them.
  import { listen } from '@tauri-apps/api/event';
  import {
    api,
    EMULATOR_INSTALLED_EVENT,
    SETUP_DEFAULTS_EVENT,
    type DiscoverFeed,
    type GameSummary,
  } from '../api';
  import { downloads } from '../stores/downloads.svelte';
  import { isInstalled } from '../stores/installed.svelte';
  import { session } from '../stores/session.svelte';
  import Coverflow from './Coverflow.svelte';
  import RecentlyAdded from './RecentlyAdded.svelte';
  import { discoverAction, liveInstallStatus } from './feed';
  import { filterPills, keptFilter, listKey, nextFilter, shownList, type Filtered } from './filters';

  let {
    active,
    labelFor,
    onOpen,
    onPrimary,
    onOpenSystems,
  }: {
    /** Server is the front view and Discover is selected. */
    active: boolean;
    /** The platform label a game shows (and is matched as installed by). */
    labelFor: (game: GameSummary) => string;
    onOpen: (game: GameSummary) => void;
    /** Install or Play, through Server's own flow. */
    onPrimary: (game: GameSummary) => void;
    /** Opens Settings › Systems. */
    onOpenSystems: () => void;
  } = $props();

  let feed = $state.raw<DiscoverFeed | null>(
    lastFeed && lastFeed.server === session.serverUrl ? lastFeed.feed : null,
  );
  let failed = $state(false);
  let cfIndex = $state(0);
  let coverflow = $state<ReturnType<typeof Coverflow> | null>(null);
  /** Bumped to load again (the saved defaults changed). */
  let reload = $state(0);
  /** The pressed platform pill (`null`: All). */
  let selected = $state<number | null>(null);
  /** The filtered list on screen; it stays until the next one arrives. */
  let filtered = $state.raw<Filtered | null>(null);
  /** Each platform's list, fetched once per feed (a reload clears it: the
   *  coverflow it leaves out may have changed). */
  let cache = new Map<number, GameSummary[]>();
  /** The latest filter request; an older answer is only cached. */
  let request = 0;

  let pills = $derived(feed ? filterPills(feed.recent_platforms) : []);
  let recentGames = $derived(feed ? shownList(feed.recently_added, filtered) : []);

  /** Shows `id`'s list (`null`: the feed's own), from the cache or the
   *  server; the previous list stays on screen until it arrives. */
  function showFilter(id: number | null) {
    const token = ++request;
    if (id === null || feed === null) {
      filtered = null;
      return;
    }
    const hit = cache.get(id);
    if (hit) {
      filtered = { id, games: hit };
      return;
    }
    const forCache = cache;
    const exclude = feed.new_releases.map((g) => g.id);
    api
      .discoverRecentlyAdded(id, exclude)
      .then((games) => {
        forCache.set(id, games);
        if (token === request) filtered = { id, games };
      })
      .catch(() => {
        // Keep what is on screen and press its pill again.
        if (token === request) selected = filtered?.id ?? null;
      });
  }

  function choose(id: number | null) {
    selected = nextFilter(selected, id);
    showFilter(selected);
  }

  // Loads each time Discover comes to the front, on reconnect and when the
  // defaults change: the preferred platforms follow Settings › Systems.
  $effect(() => {
    void reload;
    if (!active || !session.connected) return;
    let cancelled = false;
    api
      .discoverFeed()
      .then((next) => {
        if (cancelled) return;
        const centre = feed?.new_releases[cfIndex]?.id;
        feed = next;
        lastFeed = { server: session.serverUrl, feed: next };
        failed = false;
        cache = new Map();
        selected = keptFilter(selected, next.recent_platforms);
        showFilter(selected);
        const kept = next.new_releases.findIndex((g) => g.id === centre);
        cfIndex = kept === -1 ? 0 : kept;
      })
      .catch(() => {
        if (!cancelled && feed === null) failed = true;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const offs = [
      listen(EMULATOR_INSTALLED_EVENT, () => (reload += 1)),
      listen(SETUP_DEFAULTS_EVENT, () => (reload += 1)),
    ];
    return () => {
      for (const off of offs) off.then((un) => un());
    };
  });

  function actionFor(game: GameSummary) {
    return discoverAction(isInstalled(game, labelFor(game)), liveInstallStatus(downloads.entries, game.id));
  }

  /** ← / → / Enter for the coverflow when the window has the keyboard
   *  (nothing else focused). `true` when the key was used. */
  export function handleKey(e: KeyboardEvent): boolean {
    return coverflow?.handleKey(e) ?? false;
  }
</script>

<div data-testid="discover" class="discover">
  <section data-testid="discover-new-releases" class="panel sec" aria-labelledby="discover-new-h">
    <header class="head">
      <h2 id="discover-new-h">New releases</h2>
      <p class="lede">Released in the last six months for your systems.</p>
    </header>
    {#if feed === null}
      {#if failed}
        <p data-testid="discover-error" class="note">Discover could not be loaded from the server.</p>
      {/if}
    {:else if feed.new_releases.length === 0}
      <p data-testid="discover-new-empty" class="note">
        Nothing released in the last six months for your systems.
        <button data-testid="discover-open-systems" class="link" onclick={onOpenSystems}>
          Add systems in Settings › Systems
        </button>
        to see more.
      </p>
    {:else}
      <Coverflow
        bind:this={coverflow}
        bind:index={cfIndex}
        games={feed.new_releases}
        {labelFor}
        {actionFor}
        {onOpen}
        onAction={onPrimary}
      />
    {/if}
  </section>

  <section data-testid="discover-recent" class="panel sec" aria-labelledby="discover-recent-h">
    <header class="head">
      <h2 id="discover-recent-h">Recently added</h2>
      <p class="lede">New on your server.</p>
    </header>
    {#if pills.length > 0}
      <div data-testid="discover-filters" class="filters" role="group" aria-label="Filter by platform">
        <button
          data-testid="discover-filter-all"
          class="pill"
          aria-pressed={selected === null ? 'true' : 'false'}
          onclick={() => choose(null)}
        >
          All
        </button>
        {#each pills as p (p.id)}
          <button
            data-testid={`discover-filter-${p.id}`}
            class="pill"
            aria-pressed={selected === p.id ? 'true' : 'false'}
            onclick={() => choose(p.id)}
          >
            {p.label}
          </button>
        {/each}
      </div>
    {/if}
    {#if feed !== null}
      {#if recentGames.length === 0}
        <p data-testid="discover-recent-empty" class="note">Nothing new on your server yet.</p>
      {:else}
        <RecentlyAdded
          games={recentGames}
          resetKey={listKey(filtered)}
          {labelFor}
          {actionFor}
          {onOpen}
          onAction={onPrimary}
        />
      {/if}
    {/if}
  </section>
</div>

<style>
  .discover {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--gutter);
    padding-bottom: 24px;
  }

  .sec {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: none;
  }

  .head h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-h);
  }

  .lede {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .pill {
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    padding: 4px 12px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--m-fast) ease,
      border-color var(--m-fast) ease,
      color var(--m-fast) ease;
  }

  .pill:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .pill[aria-pressed='true'] {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .pill[aria-pressed='true']:hover {
    background: var(--primary-hover);
    border-color: var(--primary-hover);
  }

  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
  }

  .note {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .link {
    font: inherit;
    padding: 0;
    border: none;
    background: none;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
  }
</style>
