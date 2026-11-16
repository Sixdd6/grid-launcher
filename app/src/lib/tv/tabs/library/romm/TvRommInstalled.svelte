<script lang="ts">
  // Library › RomM › Installed (TVRommStorefront README § RomM › Installed;
  // prototype `rPaintInstalled`): the header ("RomM · Installed" with the
  // mark, the count), the one-line banner for Can't reach and Sign-in
  // expired (focusable, Up from the grid reaches it), the chips (the
  // sign-in, Backup all N…, RT Select, LT Sort, Y Search), then the installed
  // RomM games with the Library's sort and search, with platform tags.
  // Select mode backs the picked games up (Backups work without RomM).
  import type { InstalledGame } from '../../../../api';
  import { rommInstalledRows } from '../../../../romm/rail';
  import { rommBanner } from '../../../../romm/words';
  import { sortLabel } from '../../../../library/sort';
  import { installed } from '../../../../stores/installed.svelte';
  import { romm } from '../../../../stores/romm.svelte';
  import { session } from '../../../../stores/session.svelte';
  import { updates } from '../../../../stores/updates.svelte';
  import { openBackupPage } from '../../../backupActions';
  import Glyph from '../../../glyphs/Glyph.svelte';
  import { gameFromRow, type TvGame } from '../../../games';
  import { openSignIn, retryRomm } from '../../../romm/rommActions';
  import { BUSY_WHY, installedChip, installedCountLine, installedEmpty } from '../../../romm/rommTv';
  import TvRommMark from '../../../romm/TvRommMark.svelte';
  import { tvRomm } from '../../../romm/tvRomm.svelte';
  import { registerSelectGrid, select, selKeep, type SelGame } from '../../../select.svelte';
  import TvButton from '../../../ui/TvButton.svelte';
  import TvChip from '../../../ui/TvChip.svelte';
  import TvGameCover from '../../../ui/TvGameCover.svelte';
  import TvGrid from '../../../ui/TvGrid.svelte';
  import TvSelBar from '../../../ui/TvSelBar.svelte';
  import { libraryGrid, searching } from '../libraryHeader';
  import { lib } from '../library.svelte';

  let status = $derived(romm.status);
  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let rows = $derived(rommInstalledRows(installed.list));
  const grid = (query: string): TvGame[] => libraryGrid(rows, query, lib.sort).map((r: InstalledGame) => gameFromRow(r, updateIds));
  let games = $derived(grid(tvRomm.query));
  let allGames = $derived(searching(tvRomm.query) ? grid('') : games);
  let banner = $derived(rommBanner(status));
  let chip = $derived(installedChip(status));
  let backupIds = $derived(rows.flatMap((r) => (r.rom_id !== null ? [r.rom_id] : [])));
  let empty = $derived(games.length === 0 ? installedEmpty(status, tvRomm.query) : null);

  function backupAll() {
    openBackupPage({ kind: 'games', count: backupIds.length }, backupIds);
  }

  function bannerAct() {
    if (!banner) return;
    if (banner.kind === 'unreachable') void retryRomm();
    else openSignIn('again');
  }

  // ---- Select mode: Y backs the picked games up (items 439–453).
  const selGame = (g: TvGame): SelGame => ({ id: g.id, size: g.row?.filesize_bytes ?? 0 });
  let selGames = $derived<SelGame[]>(games.map(selGame));
  let selAllGames = $derived<SelGame[]>(allGames.map(selGame));
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
        const romIds = allGames.filter((g) => ids.includes(g.id)).flatMap((g) => (g.romId !== null ? [g.romId] : []));
        openBackupPage({ kind: 'games', count: romIds.length }, romIds);
      },
    }),
  );

  const emptyParts = (body: string) => body.split('(Y)');
</script>

<div class="vh two" id="lib-h" data-testid="tv-romm-installed-header">
  <div class="vh-top">
    <h1 data-testid="tv-lib-name"><TvRommMark />RomM <span class="muted">· Installed</span></h1>
    <span class="cnt" data-testid="tv-lib-count">{installedCountLine(rows.length, games.length, searching(tvRomm.query))}</span>
  </div>
  {#if banner}
    <div class="rbanner" data-nav="h" id="romm-banner" data-testid="tv-romm-banner">
      <span class="rbx">!</span>
      <span class="rbt"><b>{banner.strong}</b> · {banner.text}</span>
      <TvButton
        k="rb-act"
        al={banner.kind === 'unreachable' ? 'Retry' : 'Sign in again'}
        disabled={banner.kind === 'unreachable' && status.busy}
        why={banner.kind === 'unreachable' && status.busy ? BUSY_WHY : undefined}
        testId="tv-romm-banner-act"
        onA={bannerAct}
      >
        {#if banner.kind === 'unreachable' && status.busy}<span class="sspin"></span>{banner.busyAction}{:else}{banner.action}{/if}
      </TvButton>
    </div>
  {/if}
  <div class="vh-row" id="romm-hrow" data-nav="h">
    {#if chip}<TvChip testId="tv-romm-chip">{chip}</TvChip>{/if}
    {#if searching(tvRomm.query)}<TvChip testId="tv-lib-search">Search: <b>{tvRomm.query.trim()}</b></TvChip>{/if}
    <span class="sp"></span>
    {#if rows.length}
      <TvButton k="rh-bk" al="Backup" testId="tv-romm-backup-all" onA={backupAll}>Backup all {rows.length}…</TvButton>
    {/if}
    <TvChip glyph="RT">Select</TvChip>
    <TvChip glyph="LT" testId="tv-lib-sort">Sort: <b>{sortLabel(lib.sort)}</b></TvChip>
    <TvChip glyph="Y">Search</TvChip>
  </div>
</div>
<TvGrid id="lib-grid" scroll="lib-grid" items={games} key={(g) => `rg${g.id}`} cover={(g) => g.cover} listKey="romm:installed" testId="tv-lib-grid">
  {#snippet cell(g)}
    <TvGameCover k={`rg${g.id}`} game={g} plat selectable testId={`tv-cover-rg${g.id}`} />
  {/snippet}
  {#if empty}
    <div class="empty" data-testid="tv-lib-empty">
      <h2>{empty.title}</h2>
      {#each emptyParts(empty.body) as part, i (i)}{#if i > 0}<Glyph button="Y" />{/if}{part}{/each}
    </div>
  {/if}
</TvGrid>
<TvSelBar games={selGames} all={selAllGames} />
