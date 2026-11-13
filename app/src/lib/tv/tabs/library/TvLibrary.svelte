<script lang="ts">
  // The Library tab (TV mode plan § P8; tvmode.js `TABS.library`,
  // `libRailHTML`, `libPaint`, `ONF.lib`): the filter rail (All games,
  // Recent, Updates, a divider, then the Platforms and Backup drives
  // folders, items 176–184, 420) beside the header and a windowed grid of
  // large covers (185–195; drives 421–424). Moving through the rail filters
  // at once and clears the search; A or Right goes into the grid. Y searches
  // live, LT sorts, RT is select mode (items 439–453): on a drive Y installs
  // or removes the picked games, elsewhere it backs them up.
  import { api, type DriveGame, type InstalledGame } from '../../../api';
  import { visibleLibraryGames } from '../../../library';
  import { driveKeyOf, matchesRail, type RailKey } from '../../../library/rail';
  import { librarySelection, selectRail } from '../../../library/selection.svelte';
  import { sortLabel } from '../../../library/sort';
  import { driveGrid, driveSortLabel } from '../../../library/drive';
  import { drives } from '../../../stores/drives.svelte';
  import { installed, refresh as refreshInstalled } from '../../../stores/installed.svelte';
  import { session } from '../../../stores/session.svelte';
  import { installAllFromDrive, removeFromDrive } from '../../../backups/install';
  import { removedFromDriveConfirm } from './libraryHeader';
  import { updates } from '../../../stores/updates.svelte';
  import Glyph from '../../glyphs/Glyph.svelte';
  import { gameFromDrive, gameFromRow, liveInstall, type TvGame } from '../../games';
  import { downloads } from '../../../stores/downloads.svelte';
  import { openBackupPage } from '../../backupActions';
  import { confirmBox, openKeyboard, openList } from '../../overlays/overlays.svelte';
  import { pushToast } from '../../../stores/toasts.svelte';
  import { registerSelectGrid, select, selEnd, selKeep, selToggle, type SelGame } from '../../select.svelte';
  import TvSelBar from '../../ui/TvSelBar.svelte';
  import { useScreen } from '../../screen.svelte';
  import TvChip from '../../ui/TvChip.svelte';
  import TvGameCover from '../../ui/TvGameCover.svelte';
  import TvGrid from '../../ui/TvGrid.svelte';
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
  import { driveGamesCache, lib, selectLibRail, selectLibStore, toggleFold } from './library.svelte';
  import type { DriveStoreGame, StoreGame } from '../../../api';
  import { isStoreRow } from '../../../gameKey';
  import { liveJobFor } from '../../../gamestores/jobs';
  import { storeGameMatches } from '../../../gamestores/view';
  import { gameStores } from '../../../stores/gameStores.svelte';
  import TvStoreCover from '../../stores/TvStoreCover.svelte';
  import TvStoreRail from '../../stores/TvStoreRail.svelte';
  import TvStoreView from '../../stores/TvStoreView.svelte';
  import { openStoreGame, playStore, quickStore, restoreFromDrive } from '../../stores/storeActions';
  import { driveStoreCache, tvStores } from '../../stores/storeState.svelte';
  import { mixedStoreGames, storeGameFromDrive } from '../../stores/libraryStores';
  import { tvDriveQuick, tvQuick } from '../../stores/storeWords';
  import { driveChips, driveCountLine, driveEmptyState, driveSortItems, emptyState, libraryGrid, countLine, searching, SEARCH_TITLE, SORT_TITLE, TV_SORTS, type EmptyState } from './libraryHeader';
  import { backupAllLabel, FOLDER_LABELS, platformBackupIds, folderHint, folderKey, folderOf, libraryRail, NO_DRIVES, railEntry, railRowKey, type FolderId } from './libraryRail';

  // "Now" once per rail recompute, as the desktop's Library reads it.
  let nowSeconds = $derived.by(() => {
    void installed.list;
    void updates.rows;
    return Math.floor(Date.now() / 1000);
  });
  let updateIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  let rail = $derived(libraryRail(installed.list, updateIds, nowSeconds, drives.withPack));
  let selected = $derived(railEntry(rail, librarySelection.key));
  /** A store's Installed or Available on show (game stores), else null. */
  let storeView = $derived(tvStores.view);

  // A row that went away (an unplugged drive, a platform's last game
  // uninstalled) falls back to All games, written back to the store.
  $effect(() => {
    if (selected.key !== librarySelection.key) {
      // The drive went away (item 452): select mode ends with it.
      selEnd();
      selectRail(selected.key);
      lib.query = '';
    }
  });

  // The registry again each time the tab shows: Recent and Recently played
  // read `last_played_at`, which a launch stamps with no event.
  $effect(() => {
    refreshInstalled().catch(() => {});
  });

  let driveKey = $derived(driveKeyOf(selected.key));
  let drive = $derived(driveKey === null ? null : drives.byKey(driveKey));
  let driveGames = $state.raw<DriveGame[] | null>(null);
  let driveGamesFor = $state<string | null>(null);

  // The drive's games, again whenever its pack changes or the registry does
  // (a finished install flips a cover to Play). Kept per drive for the
  // session, so coming back from Details draws the grid at once.
  $effect(() => {
    const key = driveKey;
    void drive?.pack?.updated;
    void drive?.pack?.game_count;
    void installed.list;
    if (key === null) return;
    const cached = driveGamesCache.get(key);
    if (driveGamesFor !== key) {
      driveGames = cached ?? null;
      driveGamesFor = key;
    }
    let cancelled = false;
    api
      .driveGames(key)
      .then((games) => {
        if (cancelled) return;
        driveGamesCache.set(key, games);
        driveGames = games;
        driveGamesFor = key;
      })
      .catch(() => {
        // The grid keeps what it had; an unreadable pack has no rail row.
      });
    return () => {
      cancelled = true;
    };
  });

  function rowFor(g: DriveGame): InstalledGame | null {
    return installed.list.find((r) => r.rom_id === g.rom_id) ?? null;
  }

  /** All games, Recent and Updates mix platforms: their covers carry tags (item 186). */
  let mixed = $derived(selected.key === 'all' || selected.key === 'recent' || selected.key === 'updates');

  /** The selected row's covers under a search (`''`: the row's whole list). */
  function gridFor(query: string): TvGame[] {
    if (driveKey !== null) {
      const list = driveGamesFor === driveKey && driveGames ? driveGames : [];
      return driveGrid(list, query, lib.driveSort).map((g) => gameFromDrive(driveKey!, g, rowFor(g), updateIds));
    }
    // Store games have their own covers (game stores): the RomM grid leaves their rows out.
    const rows = visibleLibraryGames(installed.list).filter((r) => !isStoreRow(r) && matchesRail(r, selected.key, updateIds, nowSeconds));
    return libraryGrid(rows, query, lib.sort).map((r) => gameFromRow(r, updateIds));
  }
  let games = $derived(gridFor(lib.query));
  /** The row's whole list, for select mode: picks a search hides are kept. */
  let allGames = $derived(lib.query.trim() === '' ? games : gridFor(''));
  let driveLoading = $derived(driveKey !== null && drive !== null && (driveGamesFor !== driveKey || driveGames === null));

  // ---- Game stores in the Library's own rows (TVStores README § Library):
  // All games, Recent and Updates hold the installed store games too, with
  // the store's tag; a drive's grid holds its GOG games after its RomM ones.
  let storeExtra = $derived.by((): StoreGame[] => {
    if (storeView || driveKey !== null) return [];
    if (selected.key !== 'all' && selected.key !== 'recent' && selected.key !== 'updates') return [];
    return mixedStoreGames(gameStores.installedGames, installed.list, selected.key, nowSeconds).filter((g) => storeGameMatches(g, lib.query));
  });
  // Each drive's GOG games, read again when its pack changes or a restore
  // or a backup ends.
  let driveStore = $state<Record<string, DriveStoreGame[]>>({});
  $effect(() => {
    void tvStores.driveVersion;
    void installed.list;
    const keys = drives.withPack.map((d) => `${d.key}\u0000${d.pack?.updated ?? ''}`);
    let cancelled = false;
    for (const k of keys) {
      const key = k.split('\u0000')[0];
      const cached = driveStoreCache.get(key);
      if (cached) driveStore[key] = cached;
      api
        .driveStoreGames(key)
        .then((list) => {
          if (cancelled) return;
          driveStoreCache.set(key, list);
          driveStore[key] = list;
        })
        .catch(() => {});
    }
    return () => {
      cancelled = true;
    };
  });
  let driveGog = $derived(
    driveKey === null ? [] : (driveStore[driveKey] ?? []).filter((g) => lib.query.trim() === '' || g.name.toLowerCase().includes(lib.query.trim().toLowerCase())),
  );
  type Item = { kind: 'romm'; g: TvGame } | { kind: 'store'; g: StoreGame } | { kind: 'drive'; g: DriveStoreGame };
  let items = $derived<Item[]>([
    ...games.map((g) => ({ kind: 'romm' as const, g })),
    ...storeExtra.map((g) => ({ kind: 'store' as const, g })),
    ...driveGog.map((g) => ({ kind: 'drive' as const, g })),
  ]);
  const itemKey = (it: Item) => (it.kind === 'romm' ? coverKey(it.g) : it.kind === 'store' ? `ls${it.g.key}` : `dsg${it.g.key}`);
  const itemCover = (it: Item) => (it.kind === 'romm' ? it.g.cover : it.g.cover || null);
  /** A drive's GOG game as a store game: the owned library's, else the drive's record. */
  const driveStoreGame = (dg: DriveStoreGame): StoreGame => gameStores.game(dg.key) ?? storeGameFromDrive(dg);
  /** A drive row's count includes its GOG games. */
  const rowCount = (key: RailKey, n: number) => {
    const d = driveKeyOf(key);
    return d === null ? n : n + (driveStore[d]?.length ?? 0);
  };

  let empty = $derived.by((): EmptyState | null => {
    if (items.length > 0 || driveLoading) return null;
    if (driveKey !== null) return driveEmptyState(selected.label, drive !== null, lib.query);
    return emptyState(selected.key, lib.query);
  });
  let chips = $derived(drive ? driveChips(drive) : null);

  // Each platform row's "Backup all N…" counts what its Backup page backs up.
  let backupIds = $derived(new Map(rail.platforms.map((e) => [e.key, platformBackupIds(installed.list, e.key, updateIds, nowSeconds)])));

  /** X on a Library platform row: "Backup all N…", its installed games the
   *  server knows (item 455; offline it says why, 456). */
  function backupPlatform(key: RailKey, label: string) {
    const ids = backupIds.get(key) ?? platformBackupIds(installed.list, key, updateIds, nowSeconds);
    if (ids.length === 0 && session.connected) return;
    openBackupPage({ kind: 'platform', platform: label, count: ids.length }, ids);
  }

  // ---- Select mode (items 439–453).
  const selGame = (g: TvGame): SelGame => ({ id: g.id, size: g.drive ? g.drive.game.size : (g.row?.filesize_bytes ?? 0) });
  let selGames = $derived<SelGame[]>(games.map(selGame));
  let selAllGames = $derived<SelGame[]>(allGames.map(selGame));
  // A pick the row no longer lists goes; a search that hides one keeps it
  // (the bar counts and sizes it, Y acts on it).
  $effect(() => {
    // A store view keeps its own picks (TvStoreView).
    if (select.on && !storeView) selKeep(selAllGames.map((g) => g.id));
  });

  function pickedGames(ids: readonly string[]): TvGame[] {
    return allGames.filter((g) => ids.includes(g.id));
  }

  /** Y on a drive: "<n> games on <pack>" with Install and Remove (items 449–451). */
  function driveAct(ids: string[]) {
    if (driveKey === null || drive === null) return;
    const key = driveKey;
    const pack = drive.pack?.name ?? drive.label;
    const label = drive.label;
    const picked = pickedGames(ids).flatMap((g) => (g.drive ? [{ game: g.drive.game, installed: g.installed }] : []));
    const n = picked.length;
    const word = (k: number) => `${k} game${k === 1 ? '' : 's'}`;
    void openList({
      title: `${word(n)} on ${pack}`,
      items: [
        {
          label: `Install ${word(n)}`,
          testId: 'tv-drive-install-all',
          onPick: () => {
            const todo = picked.filter((p) => !p.installed && liveInstall(downloads.entries, p.game.rom_id) === null).map((p) => p.game);
            selEnd();
            void installAllFromDrive(key, todo, { label });
          },
        },
        {
          label: `Remove ${n} from drive…`,
          danger: true,
          testId: 'tv-drive-remove-all',
          onPick: () => {
            void confirmBox({
              ...removedFromDriveConfirm(n, pack),
              danger: true,
              onOk: () => {
                selEnd();
                removeFromDrive(key, pack, picked.map((p) => p.game)).catch((err: unknown) =>
                  pushToast(err instanceof Error ? err.message : String(err), 'error'),
                );
              },
            });
          },
        },
      ],
    });
  }

  /** Y elsewhere: the Backup page for the picked games (item 443). */
  function backupAct(ids: string[]) {
    const romIds = pickedGames(ids).flatMap((g) => (g.romId !== null ? [g.romId] : []));
    openBackupPage({ kind: 'games', count: romIds.length }, romIds);
  }

  $effect(() => {
    // A store view registers its own (GOG › Installed).
    if (storeView) return;
    return registerSelectGrid({
      games: () => selGames,
      all: () => selAllGames,
      drive: () => driveKey !== null,
      connected: () => session.connected,
      act: (ids) => (driveKey !== null ? driveAct(ids) : backupAct(ids)),
    });
  });

  function search() {
    const before = lib.query;
    void openKeyboard({
      title: SEARCH_TITLE,
      value: lib.query,
      onInput: (v) => (lib.query = v),
      onDone: (v) => (lib.query = v),
      // B: the field keeps the value it had (item 385).
      onCancel: () => (lib.query = before),
    });
  }

  function sortList() {
    if (driveKey !== null) {
      void openList({
        title: SORT_TITLE,
        items: driveSortItems(lib.driveSort).map((i) => ({ label: i.label, checked: i.checked, testId: `tv-drive-sort-${i.sort}`, onPick: () => (lib.driveSort = i.sort) })),
      });
      return;
    }
    void openList({
      title: SORT_TITLE,
      items: TV_SORTS.map((s) => ({ label: sortLabel(s), checked: lib.sort === s, testId: `tv-lib-sort-${s}`, onPick: () => (lib.sort = s) })),
    });
  }

  /** GOG › Installed: the one store view with select mode (user decision 2026-11-13). */
  const gogInstalled = () => tvStores.view?.source === 'gog' && tvStores.view.part === 'installed';

  useScreen({
    // A store view has Y Search, and RT Select in GOG › Installed
    // (tvstores.js `TABS.library.hints`); no sort.
    hints: () => {
      if (tvStores.view) return gogInstalled() ? [{ button: 'Y', label: 'Search' }, { button: 'RT', label: 'Select' }] : [{ button: 'Y', label: 'Search' }];
      return [
        { button: 'Y', label: 'Search' },
        { button: 'LT', label: 'Sort' },
        { button: 'RT', label: 'Select' },
      ];
    },
    y: search,
    get lt() {
      return tvStores.view ? undefined : sortList;
    },
    get rt() {
      return tvStores.view && !gogInstalled() ? undefined : selToggle;
    },
  });

  const tid = (testId: string) => testId.replace(/^library-rail-/, 'tv-lib-rail-');
  const holds = (id: FolderId) => !storeView && !lib.folds[id] && folderOf(selected.key) === id;
  const coverKey = (g: TvGame) => `${driveKey !== null ? 'dg' : 'lg'}${g.id}`;
  const emptyParts = (body: string) => body.split('(Y)');
</script>

<div class="split" data-testid="tv-library">
  <TvRail label="Library" scroll="lib-rail">
    {#each rail.filters as e (e.key)}
      <TvRailRow k={railRowKey(e.key)} selected={!storeView && selected.key === e.key} count={e.count} testId={tid(e.testId)} onSelect={() => selectLibRail(e.key)}>{e.label}</TvRailRow>
    {/each}
    <hr />
    <TvRailRow k={folderKey('plat')} fold={lib.folds.plat} holdsSel={holds('plat')} testId="tv-lib-fold-plat" onToggle={() => toggleFold('plat')}>{FOLDER_LABELS.plat}</TvRailRow>
    {#if lib.folds.plat}
      {#each rail.platforms as e (e.key)}
        <TvRailRow
          k={railRowKey(e.key)}
          sub
          selected={!storeView && selected.key === e.key}
          count={e.count}
          xl={backupAllLabel(backupIds.get(e.key)?.length ?? 0)}
          testId={tid(e.testId)}
          onSelect={() => selectLibRail(e.key)}
          onX={() => backupPlatform(e.key, e.label)}>{e.label}</TvRailRow
        >
      {/each}
    {/if}
    <TvRailRow k={folderKey('drives')} fold={lib.folds.drives} holdsSel={holds('drives')} testId="tv-lib-fold-drives" onToggle={() => toggleFold('drives')}>{FOLDER_LABELS.drives}</TvRailRow>
    {#if lib.folds.drives}
      {#each rail.drives as e (e.key)}
        <TvRailRow k={railRowKey(e.key)} sub selected={!storeView && selected.key === e.key} count={rowCount(e.key, e.count)} testId={tid(e.testId)} onSelect={() => selectLibRail(e.key)}>{e.label}</TvRailRow>
      {:else}
        <div class="ri-empty" data-testid="tv-lib-no-drives">{NO_DRIVES}</div>
      {/each}
    {/if}
    <TvStoreRail onSelect={selectLibStore} />
  </TvRail>
  <div class="main">
    {#if storeView}
      {#key `${storeView.source}:${storeView.part}`}
        <TvStoreView source={storeView.source} part={storeView.part} query={lib.query} />
      {/key}
    {:else}
    {#if driveKey !== null}
      <div class="vh" id="lib-h" data-testid="tv-lib-header">
        <h1 data-testid="tv-lib-name">{selected.label}</h1>
        {#if drive}<span class="cnt" data-testid="tv-lib-count">{driveCountLine(drive, [...games.map((g) => g.drive!.game), ...driveGog])}</span>{/if}
        {#if chips}
          <TvChip testId="tv-lib-emus">Emulators: <b>{chips.emulators}</b></TvChip>
          <TvChip testId="tv-lib-fw">Firmware: <b>{chips.firmware}</b></TvChip>
        {/if}
        {#if (driveStore[driveKey] ?? []).length > 0}
          {@const n = (driveStore[driveKey] ?? []).length}
          <TvChip testId="tv-lib-gog">GOG: <b>{n} game{n === 1 ? '' : 's'}</b></TvChip>
        {/if}
        <span class="sp"></span>
        <TvChip glyph="RT">Select</TvChip>
        <TvChip glyph="LT" testId="tv-lib-sort">Sort: <b>{driveSortLabel(lib.driveSort)}</b></TvChip>
      </div>
    {:else}
      <div class="vh" id="lib-h" data-testid="tv-lib-header">
        <h1 data-testid="tv-lib-name">{selected.label}</h1>
        <span class="cnt" data-testid="tv-lib-count">{countLine(items.length)}</span>
        {#if searching(lib.query)}<TvChip testId="tv-lib-search">Search: <b>{lib.query}</b></TvChip>{/if}
        <span class="sp"></span>
        <TvChip glyph="RT">Select</TvChip>
        <TvChip glyph="LT" testId="tv-lib-sort">Sort: <b>{sortLabel(lib.sort)}</b></TvChip>
        <TvChip glyph="Y">Search</TvChip>
      </div>
    {/if}
    <TvGrid id="lib-grid" scroll="lib-grid" {items} key={itemKey} cover={itemCover} listKey={selected.key} testId="tv-lib-grid">
      {#snippet cell(it)}
        {#if it.kind === 'romm'}
          <TvGameCover k={coverKey(it.g)} game={it.g} plat={mixed || driveKey !== null} selectable testId={`tv-cover-${coverKey(it.g)}`} />
        {:else if it.kind === 'store'}
          {@const sg = it.g}
          {@const j = liveJobFor(downloads.entries, sg.key)}
          {@const q = tvQuick(sg, gameStores.status(sg.source), j)}
          <TvStoreCover k={itemKey(it)} game={sg} job={j} quick={q} mixed testId={`tv-cover-ls-${sg.store_id}`} onA={() => openStoreGame(sg.key, sg.title)} onX={() => quickStore(sg, q)} />
        {:else}
          {@const dg = it.g}
          {@const sg = driveStoreGame(dg)}
          {@const j = liveJobFor(downloads.entries, dg.key)}
          {@const here = gameStores.game(dg.key)?.installed ?? false}
          {@const q = tvDriveQuick(here, j)}
          <TvStoreCover
            k={itemKey(it)}
            game={{ ...sg, installed: here }}
            job={j}
            quick={q}
            mixed
            testId={`tv-cover-dsg-${sg.store_id}`}
            onA={() => drive && openStoreGame(dg.key, dg.name, { key: drive.key, name: drive.pack?.name ?? drive.label })}
            onX={() => {
              if (q === 'Play') void playStore(sg);
              else if (q && drive) void restoreFromDrive(drive.key, dg.key);
            }}
          />
        {/if}
      {/snippet}
      {#if empty}
        <div class="empty" data-testid="tv-lib-empty">
          <h2>{empty.title}</h2>
          {#each emptyParts(empty.body) as part, i (i)}{#if i > 0}<Glyph button="Y" />{/if}{part}{/each}
        </div>
      {/if}
    </TvGrid>
    <TvSelBar games={selGames} all={selAllGames} drive={driveKey !== null} />
    {/if}
  </div>
</div>
