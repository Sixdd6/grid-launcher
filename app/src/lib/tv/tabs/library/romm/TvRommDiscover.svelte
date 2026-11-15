<script lang="ts" module>
  /** The server's summaries the hero has asked for, by rom id. */
  const summaries = new Map<number, string>();
</script>

<script lang="ts">
  // Library › RomM › Discover (items 158–175, moved from the Server tab;
  // TVRommStorefront README § RomM › Discover; tvmode.js `discoverHTML`,
  // `hero`). While the startup probe runs it is a panel, "Connecting to
  // <host>…", with skeleton covers; installed rows RomM doesn't have never
  // show here. A hero panel of a fixed height
  // for the focused game (platform, title as a logo, its state, two lines of
  // summary) over rows that scroll sideways: Continue playing, New
  // releases, Recently added, Updates (only when there are any) and Your
  // library. Moving into a row brings its title to the top of the rows area
  // (item 108); Up from the top row and Down from the bottom one stay put,
  // and Left from a row's first cover goes to the rail (item 174).
  import { tick } from 'svelte';
  import { api, type DiscoverFeed, type GameSummary, type LaunchDefaults } from '../../../../api';
  import { isWindowsPlatform } from '../../../../details/actions';
  import { launchTargetLine } from '../../../../details/header';
  import { cachedFeed, fetchFeed, rememberFeed, watchFeedInputs } from '../../../../discover/feedStore.svelte';
  import { sortGames } from '../../../../library/sort';
  import { fromLiveServer } from '../../../../romm/platformGames';
  import { rommInstalledRows } from '../../../../romm/rail';
  import { rommConnectingPanel, rommHost } from '../../../../romm/words';
  import { romm } from '../../../../stores/romm.svelte';
  import type { Catalog } from '../../../../server/catalog.svelte';
  import { downloads } from '../../../../stores/downloads.svelte';
  import { installed, matchesInstalled } from '../../../../stores/installed.svelte';
  import { updates } from '../../../../stores/updates.svelte';
  import { focusables } from '../../../focus/dom';
  import { gameFromRow, gameFromSummary, liveInstall, type TvGame } from '../../../games';
  import { focused, scope, setFocus } from '../../../nav.svelte';
  import TvGameCover from '../../../ui/TvGameCover.svelte';
  import TvTrack from '../../../ui/TvTrack.svelte';
  import { discoverShelves, EMPTY_SHELF, heroEmulator, heroLine, heroSummary } from './discoverRows';
  import { tvRomm } from '../../../romm/tvRomm.svelte';

  let { catalog }: { catalog: Catalog } = $props();

  let connected = $derived(romm.status.connection === 'connected');
  let server = $derived(romm.status.server_url);
  let panel = $derived(rommConnectingPanel(rommHost(romm.status)));

  // The last feed shown, at once, while the fresh one loads (feedStore).
  // svelte-ignore state_referenced_locally
  const kept = cachedFeed(server);
  let feed = $state.raw<DiscoverFeed | null>(kept);
  let loaded = $state(kept !== null);
  let reload = $state(0);
  let defaults = $state.raw<LaunchDefaults | null>(null);

  $effect(() => {
    void reload;
    if (!connected) return;
    const srvUrl = server;
    let cancelled = false;
    fetchFeed()
      .then((next) => {
        if (cancelled) return;
        feed = next;
        rememberFeed(srvUrl, next);
        loaded = true;
      })
      .catch(() => {
        if (!cancelled) loaded = true;
      });
    api
      .getLaunchDefaults()
      .then((d) => {
        if (!cancelled) defaults = d;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });
  // An emulator install or first-run setup's defaults move the preferred
  // platforms (New releases) and the hero's emulator.
  $effect(() => watchFeedInputs(() => (reload += 1)));

  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));

  function fromFeed(list: GameSummary[]): TvGame[] {
    return list.map((g) => {
      const label = catalog.labelFor(g);
      const row = installed.list.find((r) => matchesInstalled(r, g, label)) ?? null;
      return gameFromSummary(g, label, catalog.platformOf(g)?.slug ?? '', row, updateIds);
    });
  }

  let shelves = $derived(
    discoverShelves({
      // RomM's own installed games: linked, from this server.
      library: sortGames(
        rommInstalledRows(installed.list).filter((r) => r.rom_id !== null && fromLiveServer(r, server)),
        'played',
      ).map((r) => gameFromRow(r, updateIds)),
      newReleases: feed ? fromFeed(feed.new_releases) : [],
      recentlyAdded: feed ? fromFeed(feed.recently_added) : [],
    }),
  );

  // The hero follows focus, and shows the first cover's game until a cover
  // takes focus (item 163).
  let heroId = $state<string | null>(null);
  let hero = $derived.by(() => {
    const all = shelves.flatMap((s) => s.games);
    return (heroId !== null ? all.find((g) => g.id === heroId) : undefined) ?? all[0] ?? null;
  });
  let live = $derived(hero ? liveInstall(downloads.entries, hero.romId) : null);
  let emulator = $derived.by(() => {
    if (!hero?.installed) return null;
    const line = launchTargetLine(defaults, hero.platform, hero.platformSlug);
    return heroEmulator(line, isWindowsPlatform(hero.platformSlug, hero.platform), hero.row?.native_compat_tool ?? '');
  });
  let line = $derived(hero ? heroLine(hero, live, emulator, Date.now() / 1000) : '');

  // Two lines of summary: the installed row's, else the server's (asked
  // once per game, after focus rests on it).
  let summary = $state('');
  $effect(() => {
    const g = hero;
    if (!g) {
      summary = '';
      return;
    }
    if (g.description.trim() !== '') {
      summary = heroSummary(g.description);
      return;
    }
    const id = g.romId;
    if (id === null || !connected) {
      summary = '';
      return;
    }
    const hit = summaries.get(id);
    if (hit !== undefined) {
      summary = hit;
      return;
    }
    summary = '';
    const timer = setTimeout(() => {
      api
        .getRomDetail(id)
        .then((d) => {
          const s = heroSummary(d.description);
          summaries.set(id, s);
          if (hero?.romId === id) summary = s;
        })
        .catch(() => {});
    }, 250);
    return () => clearTimeout(timer);
  });

  function focusedCover(g: TvGame, k: string) {
    heroId = g.id;
    tvRomm.discoverKey = k;
  }

  // Arrived before there were covers (item 417): the first to appear takes
  // focus, unless focus has moved on from the Discover row since.
  $effect(() => {
    void shelves;
    if (!tvRomm.pendingArrive) return;
    void tick().then(() => {
      if (!tvRomm.pendingArrive) return;
      const cur = focused();
      if (cur && cur.getAttribute('data-k') !== 'r-romm:discover') {
        tvRomm.pendingArrive = false;
        return;
      }
      const disc = scope()?.querySelector('#disc');
      const el = disc ? focusables(disc)[0] : undefined;
      if (el) {
        tvRomm.pendingArrive = false;
        setFocus(el, { instant: true });
      }
    });
  });
</script>

{#if !connected}
  <div class="list" data-scroll="list" data-testid="tv-romm-connecting">
    <div class="list-in">
      <div class="panel rconnecting" id="romm-connecting">
        <h2><span class="sspin"></span>{panel.title}</h2>
        <p class="note" style="margin:0">{panel.text}</p>
      </div>
      <div class="sskel" aria-hidden="true"><i></i><i></i><i></i><i></i><i></i></div>
    </div>
  </div>
{:else}
<div class="home" data-testid="tv-discover">
  <div class="hero" data-testid="tv-hero">
    {#if hero}
      <div class="hero-box">
        <div class="plat" data-testid="tv-hero-platform">{hero.platform}</div>
        <div class="logo" data-testid="tv-hero-title">{hero.title}</div>
        <div class="meta" data-testid="tv-hero-line">{line}</div>
        <p class="sum" data-testid="tv-hero-summary">{summary}</p>
      </div>
    {/if}
  </div>
  <div class="rows" id="disc" data-edge data-shelves data-rail-target data-scroll="disc">
    {#each shelves as s (s.id)}
      <section class="shelf" data-shelf data-testid={`tv-shelf-${s.id}`}>
        <h2>{s.name}{#if s.extra}<span>{s.extra}</span>{/if}</h2>
        {#if s.games.length}
          <TvTrack scroll={`disc-${s.id}`}>
            {#each s.games as g (g.id)}
              <TvGameCover k={`${s.id}${g.id}`} game={g} plat testId={`tv-cover-${s.id}${g.id}`} onFocus={() => focusedCover(g, `${s.id}${g.id}`)} />
            {/each}
          </TvTrack>
        {:else}
          <p class="empty">{loaded || (s.id !== 'n' && s.id !== 'r') ? EMPTY_SHELF : ''}</p>
        {/if}
      </section>
    {/each}
  </div>
</div>
{/if}
