<script lang="ts">
  // A store view in the Library (TVStores README § Library; prototype
  // `libPaint` for `st:` rows and `tvstores_bk.js` for GOG › Installed):
  // the two-row header ("GOG · Installed" and its count; the chips, Backup
  // all N…, Install from installer…, Refresh and the Y Search chip, with RT
  // Select in GOG › Installed), then the covers, or a panel in their place
  // (loading, nothing installed, everything installed, not connected, login
  // expired, the store tool missing / installing / failed, an owned list
  // that failed). In GOG › Installed RT turns on select mode: A picks, X
  // selects all, Y opens the Backup page.
  import type { StoreGame, StoreSource } from '../../api';
  import type { StorePart } from '../../gamestores/rail';
  import { liveJobFor } from '../../gamestores/jobs';
  import { sortStoreGames, storeGameMatches } from '../../gamestores/view';
  import { storeBackupTitle } from '../../gamestores/backup';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { downloads } from '../../stores/downloads.svelte';
  import Glyph from '../glyphs/Glyph.svelte';
  import { registerSelectGrid, select, selKeep } from '../select.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvChip from '../ui/TvChip.svelte';
  import TvGrid from '../ui/TvGrid.svelte';
  import TvStoreCover from './TvStoreCover.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import {
    installTool,
    openInstaller,
    openStoreBackup,
    openStoreGame,
    quickStore,
    refreshStore,
    goStoreSettings,
    showAvailable,
    startLogin,
  } from './storeActions';
  import { gogSelSummary, tvChips, tvCountLine, tvHeaderActions, tvNoMatch, tvPanel, tvQuick, tvViewTitle, type PanelAct } from './storeWords';

  let { source, part, query }: { source: StoreSource; part: StorePart; query: string } = $props();

  let status = $derived(gameStores.status(source));
  let all = $derived(sortStoreGames(part === 'installed' ? gameStores.installed(source) : gameStores.available(source)));
  let games = $derived(all.filter((g) => storeGameMatches(g, query)));
  let nowSeconds = $derived.by(() => {
    void status;
    return Math.floor(Date.now() / 1000);
  });
  let panel = $derived(status ? tvPanel(status, part, all) : null);
  let acts = $derived(status ? tvHeaderActions(status, part, all.length) : null);
  let title = $derived(status ? tvViewTitle(status, part) : null);

  // ---- Select mode in GOG › Installed (user decision 2026-11-13).
  let gogInstalled = $derived(source === 'gog' && part === 'installed');
  let selGames = $derived(games.map((g) => ({ id: g.key, size: g.disk_bytes })));
  let selAll = $derived(all.map((g) => ({ id: g.key, size: g.disk_bytes })));
  $effect(() => {
    if (select.on && gogInstalled) selKeep(selAll.map((g) => g.id));
  });
  $effect(() => {
    if (!gogInstalled) return;
    return registerSelectGrid({
      games: () => selGames,
      all: () => selAll,
      drive: () => false,
      // No GOG login or server is needed to back GOG games up.
      connected: () => true,
      act: (ids) => openStoreBackup(ids, storeBackupTitle({ kind: 'games', count: ids.length })),
    });
  });
  let selBytes = $derived(selAll.filter((g) => select.ids.includes(g.id)).reduce((t, g) => t + g.size, 0));
  let allPicked = $derived(selGames.length > 0 && selGames.every((g) => select.ids.includes(g.id)));

  function panelAct(a: PanelAct) {
    switch (a) {
      case 'open_available':
        showAvailable(source);
        return;
      case 'installer':
        void openInstaller();
        return;
      case 'refresh':
        refreshStore(source);
        return;
      case 'connect':
      case 'sign_in':
        startLogin(source);
        return;
      case 'install_tool':
        installTool(source);
        return;
      case 'open_settings':
        goStoreSettings();
    }
  }

  const job = (g: StoreGame) => liveJobFor(downloads.entries, g.key);
  const coverKey = (g: StoreGame) => `s${g.key}`;
  const emptyParts = (body: string) => body.split('(Y)');
</script>

{#if status && title && acts}
  <div class="vh two" id="lib-h" data-testid="tv-store-header">
    <div class="vh-top">
      <h1 data-testid="tv-lib-name">{title.name} <span class="muted">· {title.part}</span></h1>
      <span class="cnt" data-testid="tv-lib-count">{tvCountLine(status, part, games.length, nowSeconds)}</span>
    </div>
    <div class="vh-row" data-nav="h" id="store-h-row">
      {#each tvChips(status) as c (c.key)}
        {#if c.signIn}
          <TvChip k="sh-login" warn al="Sign in again" testId="tv-store-chip-login" onA={() => startLogin(source)}>{c.text}</TvChip>
        {:else}
          <TvChip testId={`tv-store-chip-${c.key}`}>{c.text}{#if c.bold}<b>{c.bold}</b>{/if}</TvChip>
        {/if}
      {/each}
      {#if query.trim() !== ''}<TvChip testId="tv-lib-search">Search: <b>{query}</b></TvChip>{/if}
      <span class="sp"></span>
      {#if acts.backupAll > 0}
        <TvButton k="sh-bk" al="Backup" testId="tv-store-backup-all" onA={() => openStoreBackup(all.map((g) => g.key), storeBackupTitle({ kind: 'all', count: all.length }))}
          >Backup all {acts.backupAll}…</TvButton
        >
      {/if}
      {#if acts.installer}<TvButton k="sh-inst" al="Choose a file" testId="tv-store-installer" onA={() => void openInstaller()}>Install from installer…</TvButton>{/if}
      {#if acts.refresh}<TvButton k="sh-ref" al="Refresh" testId="tv-store-refresh" onA={() => refreshStore(source)}>Refresh</TvButton>{/if}
      <TvChip glyph="Y">Search</TvChip>
      {#if acts.select}<TvChip glyph="RT">Select</TvChip>{/if}
    </div>
  </div>
  {#if panel && panel.kind === 'loading'}
    <TvGrid id="lib-grid" scroll="lib-grid" testId="tv-store-loading">
      <div class="sload" style="grid-column:1/-1">
        <p class="note"><span class="sspin"></span> {panel.title}</p>
        <div class="sskel"><i></i><i></i><i></i><i></i><i></i></div>
      </div>
    </TvGrid>
  {:else if panel}
    <TvGrid id="lib-grid" scroll="lib-grid" testId="tv-store-panel">
      <div class="sstate" class:warn={panel.tone === 'warn'} class:err={panel.tone === 'err'} data-testid={panel.testId}>
        <TvStoreMark {source} lg />
        <h2>{panel.title}</h2>
        <p>{panel.body}</p>
        {#if panel.actions.length}
          <div class="btns" data-nav="h">
            {#each panel.actions as a (a.act)}
              <TvButton k={`sb-${a.act}`} primary={a.primary} disabled={a.disabled} testId={`tv-store-panel-${a.act}`} onA={() => panelAct(a.act)}>{a.label}</TvButton>
            {/each}
          </div>
        {/if}
      </div>
    </TvGrid>
  {:else}
    <TvGrid id="lib-grid" scroll="lib-grid" items={games} key={coverKey} cover={(g) => g.cover} listKey={`${source}:${part}`} testId="tv-lib-grid">
      {#snippet cell(g)}
        {@const j = job(g)}
        {@const q = tvQuick(g, status, j)}
        <TvStoreCover
          k={coverKey(g)}
          game={g}
          job={j}
          quick={q}
          sel={gogInstalled ? g.key : undefined}
          picked={gogInstalled && select.picked(g.key)}
          testId={`tv-store-cover-${g.store_id}`}
          onA={() => openStoreGame(g.key, g.title)}
          onX={() => quickStore(g, q)}
        />
      {/snippet}
      {#if games.length === 0}
        {@const e = tvNoMatch(query)}
        <div class="empty" data-testid="tv-lib-empty">
          <h2>{e.title}</h2>
          {#each emptyParts(e.body) as p, i (i)}{#if i > 0}<Glyph button="Y" />{/if}{p}{/each}
        </div>
      {/if}
    </TvGrid>
  {/if}
  {#if gogInstalled && select.on}
    <div class="selbar" data-testid="tv-selbar">
      <b data-testid="tv-selbar-summary">{gogSelSummary(select.ids.length, selBytes)}</b>
      <span class="sp"></span>
      <span class="hint" data-btn="X"><Glyph button="X" />{allPicked ? `All ${selGames.length} selected · Clear` : `Select all ${selGames.length}`}</span>
      <span class="hint" data-btn="Y"><Glyph button="Y" />Backup to drive…</span>
      <span class="hint" data-btn="RT"><Glyph button="RT" />Done</span>
    </div>
  {/if}
{/if}
