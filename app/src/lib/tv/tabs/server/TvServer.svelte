<script lang="ts">
  // The Server tab (TV mode plan § P7; tvmode.js `TABS.server`): the rail
  // with Discover first, a divider, "PLATFORMS" and the server's platforms
  // with their game counts (items 157, 196), beside Discover or the
  // platform's grid. Unreachable, one panel offers Try again and Open
  // Library (207–208); used without a server, one offers Connect to a
  // server… and Open Library (518–519).
  import { tick, untrack } from 'svelte';
  import { refresh as refreshInstalled } from '../../../stores/installed.svelte';
  import { connectToServer, session } from '../../../stores/session.svelte';
  import { platformLabel } from '../../../server/platformLabel';
  import { focusables } from '../../focus/dom';
  import { focused, scope, setFocus, showTab } from '../../nav.svelte';
  import { reconnect } from '../../reconnect';
  import { useScreen } from '../../screen.svelte';
  import { openSearchAll } from '../../pages/searchAll';
  import TvButton from '../../ui/TvButton.svelte';
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
  import TvDiscover from './TvDiscover.svelte';
  import TvPlatform from './TvPlatform.svelte';
  import { selectRail, serverCatalog, srv } from './server.svelte';
  import { searchTitle } from './serverHeader';
  import { openKeyboard } from '../../overlays/overlays.svelte';

  const catalog = serverCatalog();

  /** online: connected; offline: the server can't be reached; none: used without a server. */
  let mode = $derived(session.connected ? 'online' : session.offline ? 'none' : 'offline');

  // Fresh platforms, installed rows and launch defaults each time the tab
  // shows, and again on a reconnect.
  $effect(() => {
    if (!session.connected) return;
    catalog.loadPlatforms().catch(() => {});
    refreshInstalled().catch(() => {});
    void catalog.loadLaunchDefaults();
  });
  $effect(() => (session.connected ? catalog.watch() : undefined));

  // The selected platform's games (kept across tab switches by the catalog).
  $effect(() => {
    const r = srv.rail;
    if (r === 'discover' || !session.connected) return;
    if (untrack(() => catalog.activePlatform) !== r) void catalog.selectPlatform(r);
  });

  /**
   * Arriving on Server while Discover is selected focuses Discover's
   * content, the cover last focused there, else the first, not the rail
   * (items 104, 417; tvmode.js `render`). Before Discover has covers, the
   * first one to appear takes focus.
   */
  function arrive() {
    if (mode !== 'online' || srv.rail !== 'discover') return;
    const cur = focused();
    if (cur && cur.isConnected && !cur.closest('[data-rail]')) return;
    const disc = scope()?.querySelector('#disc');
    const covers = disc ? focusables(disc) : [];
    const el = covers.find((e) => e.getAttribute('data-k') === srv.discoverKey) ?? covers[0];
    if (el) setFocus(el, { instant: true });
    else srv.pendingArrive = true;
  }

  // Back online (Try again, Menu › Reconnect): the tab is drawn again, and
  // focus goes into Discover as on arriving (tvmode.js `reconnect` → `render`).
  let wasOnline: boolean | null = null;
  $effect(() => {
    const online = mode === 'online';
    if (online && wasOnline === false) void tick().then(arrive);
    wasOnline = online;
  });

  function platformName(): string {
    const p = catalog.platforms.find((x) => x.id === srv.rail);
    return p ? platformLabel(p) : '';
  }

  useScreen({
    hints: () =>
      mode !== 'online'
        ? []
        : srv.rail === 'discover'
          ? [{ button: 'Y', label: 'Search' }]
          : [
              { button: 'Y', label: 'Search' },
              { button: 'RT', label: 'Select' },
            ],
    get y() {
      if (mode !== 'online') return undefined;
      return () => {
        // Y on Discover searches everything (item 172); on a platform it
        // narrows the grid as you type (item 204).
        if (srv.rail === 'discover') {
          openSearchAll();
          return;
        }
        const before = srv.query;
        void openKeyboard({
          title: searchTitle(platformName()),
          value: srv.query,
          onInput: (v) => (srv.query = v),
          onDone: (v) => (srv.query = v),
          // B: the field keeps the value it had (item 385).
          onCancel: () => (srv.query = before),
        });
      };
    },
    get rt() {
      if (mode !== 'online' || srv.rail === 'discover') return undefined;
      // TODO(P13): select mode on the platform's grid (items 470–480).
      return () => {};
    },
    arrive,
  });
</script>

{#if mode === 'none'}
  <div class="list" data-testid="tv-server-none">
    <div class="list-in">
      <div class="panel empty" id="srv-panel">
        <h2>Not connected to a server</h2>
        Your Library and Backup drives work without one.
        <div class="btns" data-nav="h" style="margin-top:1.4rem">
          <TvButton k="srv-connect" primary testId="tv-srv-connect" onA={connectToServer}>Connect to a server…</TvButton>
          <TvButton k="srv-library" testId="tv-srv-library" onA={() => void showTab('library')}>Open Library</TvButton>
        </div>
      </div>
    </div>
  </div>
{:else if mode === 'offline'}
  <div class="list" data-testid="tv-server-offline">
    <div class="list-in">
      <div class="panel empty" id="srv-panel">
        <h2>Can't reach your server</h2>
        {session.serverUrl} isn't answering. Your installed games still play from the Library.
        <div class="btns" data-nav="h" style="margin-top:1.4rem">
          <TvButton k="srv-retry" primary testId="tv-srv-retry" onA={() => void reconnect()}>Try again</TvButton>
          <TvButton k="srv-library" testId="tv-srv-library" onA={() => void showTab('library')}>Open Library</TvButton>
        </div>
      </div>
    </div>
  </div>
{:else}
  <div class="split" data-testid="tv-server">
    <TvRail label="Server" scroll="srv-rail">
      <TvRailRow k="r-discover" selected={srv.rail === 'discover'} testId="tv-srv-rail-discover" onSelect={() => selectRail('discover')}>Discover</TvRailRow>
      <hr />
      <h3>PLATFORMS</h3>
      {#each catalog.platforms as p (p.id)}
        <TvRailRow k={`r-${p.id}`} selected={srv.rail === p.id} count={p.rom_count} testId={`tv-srv-rail-${p.id}`} onSelect={() => selectRail(p.id)}>{platformLabel(p)}</TvRailRow>
      {/each}
    </TvRail>
    <div class="main" id="srv-main">
      {#if srv.rail === 'discover'}
        <TvDiscover {catalog} />
      {:else}
        <TvPlatform {catalog} />
      {/if}
    </div>
  </div>
{/if}
