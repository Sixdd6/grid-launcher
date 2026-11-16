<script lang="ts">
  // Library › RomM › a platform (items 196–206, moved from the Server tab;
  // TVRommStorefront README § RomM › a platform; prototype `rPaintPlatform`):
  // the header with the name and the counts on its first row ("3 games on
  // RomM · 1 installed · 1 only on this computer"), and on its second the
  // emulator and firmware chips (each opens the system's page), the search,
  // Backup all N… and the RT / Y chips, over the merged grid: RomM's games
  // (installed ones Play, the rest Install) and the installed rows RomM
  // doesn't have, tagged NOT LINKED or NOT ON ROMM (decisions 14 and 15).
  import { untrack } from 'svelte';
  import { savedDefaultFor } from '../../../../emulators/defaults';
  import type { RommRailRow } from '../../../../romm/rail';
  import { installedOnPlatform, localTagLabel, platformGameCounts, platformGames, type RommGridItem } from '../../../../romm/platformGames';
  import type { Catalog } from '../../../../server/catalog.svelte';
  import { showsChips } from '../../../../server/header';
  import { installed } from '../../../../stores/installed.svelte';
  import { romm } from '../../../../stores/romm.svelte';
  import { session } from '../../../../stores/session.svelte';
  import { updates } from '../../../../stores/updates.svelte';
  import { openBackupPage } from '../../../backupActions';
  import { openSystem } from '../../../gameActions';
  import { gameFromRow, gameFromSummary, type TvGame } from '../../../games';
  import { tvRomm } from '../../../romm/tvRomm.svelte';
  import { registerSelectGrid, select, selKeep, type SelGame } from '../../../select.svelte';
  import TvButton from '../../../ui/TvButton.svelte';
  import TvChip from '../../../ui/TvChip.svelte';
  import TvGameCover from '../../../ui/TvGameCover.svelte';
  import TvGrid from '../../../ui/TvGrid.svelte';
  import TvSelBar from '../../../ui/TvSelBar.svelte';
  import { backupAllLabel, headerChips, matchesTitle, noMatch, platformCount, searching } from './platformHeader';

  let { catalog, row }: { catalog: Catalog; row: RommRailRow } = $props();

  let platform = $derived(row.platform);
  let label = $derived(row.label);
  let slug = $derived(row.slug ?? '');
  let connected = $derived(romm.status.connection === 'connected');

  // The selected platform's games (kept across tab switches by the catalog).
  $effect(() => {
    const id = platform?.id ?? null;
    if (id === null || !connected) return;
    if (untrack(() => catalog.activePlatform) !== id) void catalog.selectPlatform(id);
  });

  // The game list belongs to the previous platform until it loads: no
  // counts or covers for the new one until then.
  let loading = $derived(platform !== null && (catalog.gamesLoading || catalog.activePlatform !== platform.id));
  let serverGames = $derived(platform === null || loading ? [] : catalog.games);
  let localRows = $derived(installedOnPlatform(installed.list, slug, catalog.platforms));
  let items = $derived(platformGames(serverGames, localRows, romm.status.server_url));
  let counts = $derived(platformGameCounts(items));
  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));

  type Cell = { item: RommGridItem; game: TvGame; tag: string | null };
  const cellOf = (item: RommGridItem): Cell =>
    item.kind === 'server'
      ? { item, game: gameFromSummary(item.game, label, platform?.slug ?? slug, item.installed, updateIds), tag: null }
      : { item, game: gameFromRow(item.row, updateIds), tag: localTagLabel(item.tag) };
  let all = $derived(items.map(cellOf));
  let shown = $derived(searching(tvRomm.query) ? all.filter((c) => matchesTitle(c.game.title, tvRomm.query)) : all);
  let chips = $derived(headerChips(label, platform?.slug ?? slug, savedDefaultFor(catalog.defaultEmulators, label), catalog.firmware));

  // One firmware status read per platform shown, and again on a reconnect.
  $effect(() => {
    const id = platform?.id ?? null;
    const name = label;
    const withChips = showsChips(label, platform?.slug ?? slug);
    catalog.firmware = null;
    if (id === null || name === '' || !withChips || !connected) return;
    void catalog.refreshFirmwareStatus(id, name);
  });

  const romIds = (cells: readonly Cell[]) => cells.flatMap((c) => (c.game.romId !== null ? [c.game.romId] : []));

  /** Backup all N…: every game of the platform (item 454). */
  function backupAll() {
    if (all.length === 0) return;
    openBackupPage({ kind: 'platform', platform: label, count: all.length }, romIds(all), platform ? [platform.id] : []);
  }

  // ---- Select mode (items 439–453): Y backs the picked games up. A search
  // that hides a pick keeps it (the bar counts the platform's whole list).
  const selGame = (c: Cell): SelGame => ({ id: c.game.id, size: c.item.kind === 'server' ? c.item.game.fs_size_bytes : (c.item.row.filesize_bytes ?? 0) });
  let selGames = $derived<SelGame[]>(shown.map(selGame));
  let selAllGames = $derived<SelGame[]>(all.map(selGame));
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
        const picked = all.filter((c) => ids.includes(c.game.id));
        openBackupPage({ kind: 'games', count: picked.length }, romIds(picked), platform ? [platform.id] : []);
      },
    }),
  );
</script>

<!-- Two rows (user decision 2026-11-11): the name and its counts, then
     everything that acts on the platform. -->
<div class="vh two" id="lib-h" data-testid="tv-romm-platform-header">
  <div class="vh-top">
    <h1 data-testid="tv-lib-name">{label}</h1>
    {#if !loading}<span class="cnt" data-testid="tv-lib-count">{platformCount(counts, shown.length, searching(tvRomm.query))}</span>{/if}
  </div>
  <div class="vh-row" id="romm-prow" data-nav="h">
    {#if chips}
      <TvChip k="ch-emu" warn={chips.emu.warn} al="Open in Systems" testId="tv-romm-emu" onA={() => openSystem(label)}>
        {#if chips.emu.name}Emulator: <b>{chips.emu.name}</b>{:else}{chips.emu.label}{/if}
      </TvChip>
      {#if chips.fw.focusable}
        <TvChip k="ch-fw" al="Open in Systems" testId="tv-romm-fw" onA={() => openSystem(label)}>{chips.fw.label}</TvChip>
      {:else}
        <TvChip testId="tv-romm-fw">{chips.fw.label}</TvChip>
      {/if}
    {/if}
    {#if searching(tvRomm.query)}<TvChip testId="tv-lib-search">Search: <b>{tvRomm.query.trim()}</b></TvChip>{/if}
    <span class="sp"></span>
    {#if all.length}
      <TvButton k="bk-all" al="Backup" testId="tv-romm-backup-all" onA={backupAll}>{backupAllLabel(all.length)}</TvButton>
    {/if}
    <TvChip glyph="RT">Select</TvChip>
    <TvChip glyph="Y">Search</TvChip>
  </div>
</div>
<TvGrid id="lib-grid" scroll="lib-grid" items={shown} key={(c) => `pg${c.game.id}`} cover={(c) => c.game.cover} listKey={row.key} testId="tv-lib-grid">
  {#snippet cell(c)}
    <TvGameCover k={`pg${c.game.id}`} game={c.game} tag={c.tag} selectable testId={`tv-cover-pg${c.game.id}`} />
  {/snippet}
  {#if searching(tvRomm.query) && shown.length === 0 && !loading}
    <div class="empty" data-testid="tv-lib-empty"><h2>{noMatch(tvRomm.query)}</h2></div>
  {/if}
</TvGrid>
<TvSelBar games={selGames} all={selAllGames} />
