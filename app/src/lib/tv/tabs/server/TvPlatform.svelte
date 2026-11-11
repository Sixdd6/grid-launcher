<script lang="ts">
  // A Server platform (items 196–206; tvmode.js `srvPaint`): the header with
  // the name, the counts, the emulator and firmware chips (each opens the
  // system's page), the search, Backup all N… and the RT / Y chips, over the
  // grid of the platform's covers (no platform tags). Y narrows the grid as
  // you type; a new platform clears the search.
  import { savedDefaultFor } from '../../../emulators/defaults';
  import type { GameSummary } from '../../../api';
  import type { Catalog } from '../../../server/catalog.svelte';
  import { showsChips } from '../../../server/header';
  import { platformLabel } from '../../../server/platformLabel';
  import { installed, isInstalled, matchesInstalled } from '../../../stores/installed.svelte';
  import { session } from '../../../stores/session.svelte';
  import { updates } from '../../../stores/updates.svelte';
  import { openBackupPage } from '../../backupActions';
  import { openSystem } from '../../gameActions';
  import { registerSelectGrid, select, selKeep, type SelGame } from '../../select.svelte';
  import TvSelBar from '../../ui/TvSelBar.svelte';
  import { gameFromSummary, type TvGame } from '../../games';
  import TvButton from '../../ui/TvButton.svelte';
  import TvChip from '../../ui/TvChip.svelte';
  import TvGameCover from '../../ui/TvGameCover.svelte';
  import TvGrid from '../../ui/TvGrid.svelte';
  import { srv } from './server.svelte';
  import { backupAllLabel, headerChips, matchesTitle, noMatch, searching, srvCount } from './serverHeader';

  let { catalog }: { catalog: Catalog } = $props();

  // The selected rail row's platform (the catalog's own active row lags a
  // selection until its games load).
  let row = $derived(typeof srv.rail === 'number' ? (catalog.platforms.find((p) => p.id === srv.rail) ?? null) : null);
  let label = $derived(row ? platformLabel(row) : '');
  let slug = $derived(row?.slug ?? '');
  // The game list belongs to the previous platform until it loads: no
  // counts or covers for the new one until then.
  let loading = $derived(catalog.gamesLoading || catalog.activePlatform !== srv.rail);
  let all = $derived(loading ? [] : catalog.games);
  let shown = $derived(all.filter((g) => matchesTitle(g.name, srv.query)));
  let installedCount = $derived(all.filter((g) => isInstalled(g, label)).length);
  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let games = $derived(shown.map((g) => tvGame(g)));
  let chips = $derived(headerChips(label, slug, savedDefaultFor(catalog.defaultEmulators, label), catalog.firmware));

  function tvGame(g: GameSummary): TvGame {
    const row = isInstalled(g, label) ? (installed.list.find((r) => matchesInstalled(r, g, label)) ?? null) : null;
    return gameFromSummary(g, label, slug, row, updateIds);
  }

  // One firmware status read per platform shown, and again on a reconnect.
  $effect(() => {
    const id = row?.id ?? null;
    const name = label;
    const chips = showsChips(label, slug);
    catalog.firmware = null;
    if (id === null || name === '' || !chips || !session.connected) return;
    void catalog.refreshFirmwareStatus(id, name);
  });

  /** Backup all N…: every game of the platform (item 454). */
  function backupAll() {
    if (row === null || all.length === 0) return;
    openBackupPage({ kind: 'platform', platform: label, count: all.length }, all.map((g) => g.id), [row.id]);
  }

  // ---- Select mode (items 439–453): Y backs the picked games up.
  // X counts the covers shown; a search that hides a pick keeps it (the
  // bar counts and sizes the platform's whole list).
  let selGames = $derived<SelGame[]>(shown.map((g) => ({ id: String(g.id), size: g.fs_size_bytes })));
  let selAllGames = $derived<SelGame[]>(all.map((g) => ({ id: String(g.id), size: g.fs_size_bytes })));
  $effect(() => {
    if (select.on) selKeep(selAllGames.map((g) => g.id));
  });
  $effect(() =>
    registerSelectGrid({
      games: () => selGames,
      all: () => selAllGames,
      drive: () => false,
      connected: () => session.connected,
      act: (ids) => {
        const romIds = ids.map(Number).filter((n) => Number.isFinite(n));
        openBackupPage({ kind: 'games', count: romIds.length }, romIds, row ? [row.id] : []);
      },
    }),
  );
</script>

<div class="vh" data-testid="tv-srv-header">
  <h1>{label}</h1>
  {#if !loading}<span class="cnt" data-testid="tv-srv-count">{srvCount(all.length, shown.length, installedCount, searching(srv.query))}</span>{/if}
  {#if chips}
    <TvChip k="ch-emu" warn={chips.emu.warn} al="Open in Systems" testId="tv-srv-emu" onA={() => openSystem(label)}>
      {#if chips.emu.name}Emulator: <b>{chips.emu.name}</b>{:else}{chips.emu.label}{/if}
    </TvChip>
    {#if chips.fw.focusable}
      <TvChip k="ch-fw" al="Open in Systems" testId="tv-srv-fw" onA={() => openSystem(label)}>{chips.fw.label}</TvChip>
    {:else}
      <TvChip testId="tv-srv-fw">{chips.fw.label}</TvChip>
    {/if}
  {/if}
  {#if searching(srv.query)}<TvChip testId="tv-srv-search">Search: <b>{srv.query.trim()}</b></TvChip>{/if}
  <span class="sp"></span>
  {#if shown.length}
    <TvButton k="bk-all" al="Backup" testId="tv-srv-backup-all" disabled={!session.connected} why={session.connected ? undefined : 'Connect to a server to backup games.'} onA={backupAll}>{backupAllLabel(all.length)}</TvButton>
  {/if}
  <TvChip glyph="RT">Select</TvChip>
  <TvChip glyph="Y">Search</TvChip>
</div>
<TvGrid id="srv-grid" scroll="srv-grid" items={games} key={(g) => `sg${g.id}`} cover={(g) => g.cover} listKey={srv.rail}>
  {#snippet cell(g)}
    <TvGameCover k={`sg${g.id}`} game={g} selectable testId={`tv-cover-sg${g.id}`} />
  {/snippet}
  {#if searching(srv.query) && shown.length === 0 && !loading}
    <div class="empty" data-testid="tv-srv-nomatch"><h2>{noMatch(srv.query)}</h2></div>
  {/if}
</TvGrid>
<TvSelBar games={selGames} all={selAllGames} />
