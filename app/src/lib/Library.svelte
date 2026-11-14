<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type DriveGame, type InstalledGame } from './api';
  import UmuInstallOffer from './UmuInstallOffer.svelte';
  import { quickAction } from './games/quick';
  import { installFromDrive, playInstalled, type RunOutcome } from './games/run';
  import { installed, refresh as refreshInstalled } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { installedKey, visibleLibraryGames } from './library';
  import { isMissing } from './library/missing';
  import { driveKeyOf, emptyText, entryForKey, matchesRail, railEntries, type RailEntry, type RailKey } from './library/rail';
  import {
    DRIVE_SORTS,
    driveChips,
    driveCountLine,
    driveEmptyText,
    driveGameKey,
    driveGrid,
    driveSortLabel,
    installSelectionLabel,
    normalizeDriveSort,
    type DriveSort,
  } from './library/drive';
  import { drives } from './stores/drives.svelte';
  import { session } from './stores/session.svelte';
  import { driveAssetUrl } from './backups/driveArt';
  import { installAllFromDrive, removeFromDrive } from './backups/install';
  import SelectionBar from './backups/SelectionBar.svelte';
  import { openBackup } from './backups/dialogState.svelte';
  import { backupRequest, sizeText } from './backups/dialog';
  import { backupAction } from './details/actions';
  import {
    allPicked,
    EMPTY_SELECTION,
    isPicked,
    keepOnly,
    leavesSelectMode,
    pick,
    selectAll,
    totalBytes,
    type Selection,
  } from './backups/selection';
  import { librarySelection, selectRail } from './library/selection.svelte';
  import { LIBRARY_SORTS, normalizeSort, sortGames, sortLabel, titleContains, type LibrarySort } from './library/sort';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { fromDriveGame, fromInstalled, type DetailsSubject } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import GameCard from './GameCard.svelte';
  import { coverPair } from './cards/cover';
  import type { Rect } from './cards/layout';
  import CardGrid from './CardGrid.svelte';
  import FoldingHeader from './FoldingHeader.svelte';
  import { gameCountLine, librarySearchPlaceholder } from './views/fold';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import Details from './Details.svelte';
  import { chordBlocked, chordContext, shouldFocusSearch } from './views/searchKeys';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { HOVER_DELAY_MS, PREFETCH_DELAY_MS, subjectFromInstalled } from './background';
  import { createHoverViewed } from './lastViewedHover';
  import { dropPendingWarms } from './backgroundPrefetch';
  import { attachScrollGate, createVisibleWarmer, scrollParent } from './visibleWarm';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { inputMode, noteDirectional } from './stores/inputMode.svelte';
  import type { DriveStoreGame, StoreGame } from './api';
  import { isStoreRow } from './gameKey';
  import { gameStores } from './stores/gameStores.svelte';
  import { downloads } from './stores/downloads.svelte';
  import { parseStoreRailKey, storeRailGroups } from './gamestores/rail';
  import { coverFaded, coverPrimary, coverProgress, coverProgressTone, coverTags, showCoverCloud } from './gamestores/cover';
  import { liveJobFor } from './gamestores/jobs';
  import { noMatchText, storeGameMatches, storePanel, type PanelAct } from './gamestores/view';
  import { mixedCounts, mixedStoreGames, selectedToWrite, sortStoreView } from './gamestores/library';
  import { driveGogChip, driveStoreGrid, driveStorePrimary, storeBackupTitle } from './gamestores/backup';
  import { canBackup, storeMark, storeName } from './gamestores/marks';
  import StoreViewHeader from './gamestores/desktop/StoreViewHeader.svelte';
  import StorePanel from './gamestores/desktop/StorePanel.svelte';
  import {
    installTool,
    openInstall,
    openInstaller,
    openStoreBackup,
    openStoreDetails,
    playStore,
    refreshStore,
    showCard,
    signIn,
    steamRecheck,
  } from './gamestores/desktop/ui.svelte';
  import { navigation } from './stores/navigation.svelte';
  import { refusedNotice } from './gamestores/notices';

  let { active }: { active: boolean } = $props();

  let search = $state('');
  let sort = $state<LibrarySort>('title');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let scrollEl = $state<HTMLElement | null>(null);
  let folded = $state(false);
  let subject = $state<DetailsSubject | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  let launchError = $state<string | null>(null);
  // The launch error that offers the umu-launcher install (a Proton launch
  // with no umu-run); `null` for any other.
  let umuOfferFor = $state<string | null>(null);
  let cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());

  // `Date.now()` is read once per rail recompute rather than per row, so
  // every entry and every predicate in one render agrees on "now".
  let nowSeconds = $derived.by(() => {
    // Depend on the two inputs that can change the rail, so the timestamp
    // refreshes whenever the rail does instead of freezing at mount.
    void installed.list;
    void updates.rows;
    return Math.floor(Date.now() / 1000);
  });

  let updateRomIds = $derived(new Set(updates.rows.map((r) => r.rom_id)));
  // Store rows live in their store group (GameStores README): the RomM
  // rail and grids read only RomM rows, and the store games are mixed into
  // All games, Recent and Updates with their store's tag.
  let romRows = $derived(installed.list.filter((r) => !isStoreRow(r)));
  let storeMix = $derived(mixedCounts(gameStores.installedGames, installed.list, nowSeconds));
  let storeGroups = $derived(storeRailGroups(gameStores.statuses, (s) => gameStores.library(s)));
  let entries = $derived([
    ...railEntries(romRows, updateRomIds, nowSeconds, drives.withPack).map((e) =>
      e.key === 'all' || e.key === 'recent' || e.key === 'updates' ? { ...e, count: e.count + storeMix[e.key] } : e,
    ),
    ...storeGroups.flatMap((g) =>
      g.rows.map((r): RailEntry => ({ key: r.key, testId: r.testId, label: r.label, count: r.count.kind === 'number' ? r.count.n : 0 })),
    ),
  ]);
  let selected = $derived(entryForKey(entries, librarySelection.key));

  // A store view (Library › GOG › Installed / Available).
  let storeSel = $derived(parseStoreRailKey(selected.key));
  let storeStatus = $derived(storeSel ? gameStores.status(storeSel.source) : null);
  let storeAll = $derived(
    storeSel ? (storeSel.part === 'installed' ? gameStores.installed(storeSel.source) : gameStores.available(storeSel.source)) : [],
  );
  let storeList = $derived(sortStoreView(storeAll.filter((g) => storeGameMatches(g, search)), sort, installed.list));
  let storePanelView = $derived(storeStatus && storeSel ? storePanel(storeStatus, storeSel.part, storeAll) : null);
  /** A backup store's Installed (GOG › Installed) is the one store view with Select (GOG backups); Epic Games has none. */
  let storeSelectable = $derived(storeSel !== null && canBackup(storeSel.source) && storeSel.part === 'installed' && storeAll.length > 0);
  let storePicks = $state<string[]>([]);

  let rows = $derived(
    sortGames(
      visibleLibraryGames(romRows).filter(
        (row) =>
          matchesRail(row, selected.key, updateRomIds, nowSeconds) && titleContains(row.title, search),
      ),
      sort,
    ),
  );

  // All games, Recent and Updates: the RomM rows, then the installed store games.
  let mixed = $derived(mixedStoreGames(gameStores.installedGames, installed.list, selected.key, search, nowSeconds));
  type LibItem = { kind: 'romm'; row: InstalledGame } | { kind: 'store'; game: StoreGame };
  let items = $derived<LibItem[]>([
    ...rows.map((row): LibItem => ({ kind: 'romm', row })),
    ...mixed.map((game): LibItem => ({ kind: 'store', game })),
  ]);

  // Library › Backup drives (GameBackups README): a drive row shows the
  // drive's own games. Unplugging the drive drops its row, and `entryForKey`
  // falls back to All games.
  let driveKey = $derived(driveKeyOf(selected.key));
  let drive = $derived(driveKey === null ? null : drives.byKey(driveKey));
  let driveGames = $state.raw<DriveGame[]>([]);
  /** The drive key `driveGames` belongs to; `null` while the first answer for this drive is pending. */
  let driveGamesFor = $state<string | null>(null);
  let driveError = $state<string | null>(null);
  let driveSort = $state<DriveSort>('title');
  let driveRows = $derived(driveKey !== null && driveGamesFor === driveKey ? driveGrid(driveGames, search, driveSort) : []);
  // Its GOG games follow its RomM games, each with a GOG tag (GameStores README).
  let driveStore = $state.raw<DriveStoreGame[]>([]);
  let driveStoreRows = $derived(driveKey !== null && driveGamesFor === driveKey ? driveStoreGrid(driveStore, search) : []);
  type DriveItem = { kind: 'romm'; game: DriveGame } | { kind: 'gog'; game: DriveStoreGame };
  let driveItems = $derived<DriveItem[]>([
    ...driveRows.map((game): DriveItem => ({ kind: 'romm', game })),
    ...driveStoreRows.map((game): DriveItem => ({ kind: 'gog', game })),
  ]);
  let gogChip = $derived(driveGogChip(driveStore));
  let driveLoading = $derived(driveKey !== null && driveGamesFor !== driveKey && driveError === null);
  let chips = $derived(drive === null ? null : driveChips(drive));

  // The drive's games, again whenever its pack changes (a backup or a
  // removal moves `updated` and the count) or the registry does (a finished
  // install flips a card from Install to Play).
  $effect(() => {
    const key = driveKey;
    void drive?.pack?.updated;
    void drive?.pack?.game_count;
    void installed.list;
    if (key === null) return;
    let cancelled = false;
    driveError = null;
    Promise.all([api.driveGames(key), api.driveStoreGames(key).catch(() => [] as DriveStoreGame[])])
      .then(([games, storeGames]) => {
        if (cancelled) return;
        driveGames = games;
        driveStore = storeGames;
        driveGamesFor = key;
      })
      .catch((err) => {
        if (cancelled) return;
        driveError = err instanceof Error ? err.message : String(err);
      });
    return () => {
      cancelled = true;
    };
  });

  /** The cards on screen, whichever grid: the empty state and the focus clamp read it. */
  let gridCount = $derived(driveKey !== null ? driveItems.length : storeSel !== null ? storeList.length : items.length);

  // Select mode (GameBackups README): on installed games it backs them up;
  // on a drive it installs or removes them. Changing the rail leaves it.
  let selecting = $state(false);
  let selection = $state.raw<Selection>(EMPTY_SELECTION);
  let confirmRemove = $state(false);
  let removeError = $state<string | null>(null);
  let visibleIds = $derived(
    driveKey !== null ? driveRows.map((g) => g.rom_id) : rows.flatMap((r) => (r.rom_id === null ? [] : [r.rom_id])),
  );
  let pickedBytes = $derived.by(() => {
    const sizes = new Map<number, number>(
      driveKey !== null
        ? driveGames.map((g) => [g.rom_id, g.size])
        : installed.list.flatMap((r) => (r.rom_id === null ? [] : [[r.rom_id, r.filesize_bytes] as [number, number]])),
    );
    return totalBytes(selection, (id) => sizes.get(id) ?? 0);
  });
  let backup = $derived(backupAction(session.connected));

  function leaveSelectMode() {
    selecting = false;
    selection = EMPTY_SELECTION;
    storePicks = [];
    confirmRemove = false;
    removeError = null;
  }

  function toggleSelectMode() {
    if (selecting) leaveSelectMode();
    else selecting = true;
  }

  /** "Backup to drive…" on GOG › Installed's select bar. */
  function backUpStoreSelection() {
    if (storePicks.length === 0) return;
    openStoreBackup([...storePicks], storeBackupTitle({ kind: 'games', count: storePicks.length }), leaveSelectMode);
  }

  /** "Backup all N…" in the GOG › Installed header. */
  function backUpStoreAll() {
    const keys = storeAll.map((g) => g.key);
    if (keys.length > 0) openStoreBackup(keys, storeBackupTitle({ kind: 'all', count: keys.length }));
  }

  /** A store state panel's action. */
  function storePanelAct(act: PanelAct) {
    if (!storeSel) return;
    const source = storeSel.source;
    switch (act) {
      case 'open_available':
        onRailSelect(`store:${source}:available`);
        break;
      case 'installer':
        openInstaller();
        break;
      case 'refresh':
        void refreshStore(source);
        break;
      case 'connect':
      case 'sign_in':
        void signIn(source);
        break;
      case 'install_tool':
        void installTool(source);
        break;
      case 'open_settings':
        navigation.goSettings('stores');
        break;
      case 'recheck':
        void steamRecheck();
        break;
      default:
        break;
    }
  }

  /** A store cover's hover action. */
  function storePrimary(game: StoreGame) {
    const p = coverPrimary(game, gameStores.status(game.source), liveJobFor(downloads.entries, game.key));
    if (p.act === 'play') void playStore(game);
    else if (p.act === 'install') openInstall(game);
    else openStoreDetails(game.key);
  }

  function openDriveStoreDetails(g: DriveStoreGame) {
    if (drive === null) return;
    openStoreDetails(g.key, 'overview', { key: drive.key, name: drive.pack?.name ?? drive.label, label: drive.label, game: g });
  }

  async function driveStoreAct(g: DriveStoreGame) {
    const busy = liveJobFor(downloads.entries, g.key) !== null;
    const p = driveStorePrimary(g, busy);
    if (p.act === 'play') {
      const game = gameStores.game(g.key);
      if (game) void playStore(game);
      return;
    }
    if (p.act === 'details' || driveKey === null) {
      openDriveStoreDetails(g);
      return;
    }
    try {
      await api.installStoreFromDrive(driveKey, g.key);
    } catch (err) {
      showCard(refusedNotice(`Couldn’t install ${g.name}`, err instanceof Error ? err.message : String(err)));
    }
  }

  function onRailSelect(key: RailKey) {
    if (key !== librarySelection.key) leaveSelectMode();
    selectRail(key);
  }

  // A refresh can drop a game: the selection never counts one off screen.
  $effect(() => {
    const ids = driveKey !== null ? driveGames.map((g) => g.rom_id) : installed.list.flatMap((r) => (r.rom_id === null ? [] : [r.rom_id]));
    const next = keepOnly(untrack(() => selection), ids);
    if (next !== untrack(() => selection)) selection = next;
  });

  // A change to the picks takes back a pending "Remove N from drive".
  $effect(() => {
    void selection;
    confirmRemove = false;
  });

  /** Picked games that are not installed yet: what "Install N games" copies. */
  let toInstall = $derived(driveGames.filter((g) => !g.installed && selection.ids.includes(g.rom_id)).length);

  function pickedDriveGames(): DriveGame[] {
    return driveGames.filter((g) => selection.ids.includes(g.rom_id));
  }

  /** The select bar's "Install N games": copies each picked game that is not installed yet. */
  async function installSelection() {
    if (driveKey === null) return;
    const key = driveKey;
    const todo = pickedDriveGames().filter((g) => !g.installed);
    leaveSelectMode();
    await installAllFromDrive(key, todo);
  }

  /** "Remove from drive…", then "Remove N from <pack>". */
  async function removeSelection() {
    if (driveKey === null || drive === null) return;
    if (!confirmRemove) {
      confirmRemove = true;
      return;
    }
    removeError = null;
    try {
      await removeFromDrive(driveKey, drive.pack?.name ?? drive.label, pickedDriveGames());
      leaveSelectMode();
    } catch (err) {
      confirmRemove = false;
      removeError = err instanceof Error ? err.message : String(err);
    }
  }

  /** "Backup to drive…" on the select bar of installed games. */
  function backUpSelection() {
    if (selection.ids.length === 0 || backup.disabled) return;
    openBackup({
      request: backupRequest({ kind: 'games', count: selection.ids.length }, selection.ids),
      onQueued: leaveSelectMode,
    });
  }

  // "Backup all N…" on a Library platform row: the installed games of
  // that platform the server knows (a row with no server id can't be
  // downloaded again).
  let platformBackupIds = $derived(
    selected.key.startsWith('platform:')
      ? visibleLibraryGames(installed.list).flatMap((r) =>
          matchesRail(r, selected.key, updateRomIds, nowSeconds) && r.rom_id !== null ? [r.rom_id] : [],
        )
      : [],
  );

  function backUpPlatform() {
    if (platformBackupIds.length === 0 || backup.disabled) return;
    openBackup({
      request: backupRequest(
        { kind: 'platform', platform: selected.label, count: platformBackupIds.length },
        platformBackupIds,
      ),
    });
  }

  function driveCover(game: DriveGame) {
    const url = driveKey === null ? null : driveAssetUrl(driveKey, game.cover);
    return coverPair(null, url);
  }

  /** A failed launch's reason shows here only beside the umu-launcher
   *  install it needs; every other refusal is a card (games/run.ts). */
  function showLaunchOutcome(outcome: RunOutcome) {
    if (outcome.kind === 'ok') return;
    const missing = outcome.kind === 'needs-umu';
    launchError = missing ? outcome.reason : null;
    umuOfferFor = missing ? launchError : null;
  }

  async function drivePrimary(game: DriveGame) {
    const quick = quickAction({ installed: game.installed, romId: game.rom_id, fromDrive: true });
    if (quick.kind === 'play') {
      launchError = null;
      showLaunchOutcome(await playInstalled(game.rom_id, game.name));
      return;
    }
    if (driveKey !== null) await installFromDrive(driveKey, game);
  }

  function openDriveDetails(game: DriveGame, mode: CloudMode = 'overview') {
    if (drive === null) return;
    detailsCloudMode = mode;
    // No `noteViewed`: the drive's art is not on the server the background
    // layer builds from.
    subject = fromDriveGame(game, drive);
  }

  // Details from a drive closes when that drive goes away: its Install and
  // Remove act on a drive that is no longer there.
  $effect(() => {
    const current = subject;
    if (current?.drive !== undefined && drives.byKey(current.drive.key) === null) closeDetails();
  });

  /** The rail key without its `platform:` prefix — the dash slug the
   *  §11 `library-rail-platform-<slug>` id already carries, so the count
   *  id beside it never grows a colon. */
  const slugOf = (key: RailKey): string =>
    key.startsWith('platform:')
      ? key.slice('platform:'.length)
      : key.startsWith('store:')
        ? `store-${key.slice('store:'.length).replace(/:/g, '-')}`
        : key;

  let firstPlatform = $derived(entries.findIndex((e) => e.key.startsWith('platform:')));
  let firstDrive = $derived(entries.findIndex((e) => e.key.startsWith('drive:')));

  /** Each store group's heading, mark and badge, keyed by its first row. */
  let storeHeads = $derived(
    new Map(storeGroups.map((g) => [g.rows[0].key as string, { heading: g.heading, mark: storeMark(g.source), flag: g.flag }])),
  );
  let storeCounts = $derived(new Map(storeGroups.flatMap((g) => g.rows.map((r) => [r.key as string, r.count.kind] as const))));

  let railRows = $derived(
    entries.map(
      (entry, i): RailPaneEntry<RailKey> => ({
        key: entry.key,
        testId: entry.testId,
        countTestId: `library-rail-count-${slugOf(entry.key)}`,
        dataRail: entry.testId,
        label: entry.label,
        count: entry.count,
        selected: entry.key === selected.key,
        // The three fixed entries come first, then the platforms, then the
        // backup drives; each group's heading sits on its first row.
        heading:
          i === firstPlatform ? 'PLATFORMS' : i === firstDrive ? 'BACKUP DRIVES' : storeHeads.get(entry.key)?.heading,
        icon: entry.key.startsWith('drive:') ? 'drive' : undefined,
        title: entry.title,
        ...(entry.key.startsWith('store:')
          ? {
              indent: true,
              countKind: storeCounts.get(entry.key) ?? 'number',
              headingMark: storeHeads.get(entry.key)?.mark,
              headingFlag: storeHeads.get(entry.key)?.flag,
            }
          : {}),
      }),
    ),
  );

  // Which platforms have a default emulator, for the cards' cloud badge
  // (see `cloudPlatformSet`). One call per mount; Settings › Systems is
  // where defaults change, and switching back re-runs this effect.
  $effect(() => {
    if (!active) return;
    // The registry row's `last_played_at` is stamped by the backend after a
    // launch with no event to announce it, and the rail's Recent entry and
    // the "Recently played" sort both read it. Re-read the registry whenever
    // this view comes forward, so a game played from Details or from the
    // Server grid is ordered by when it was actually played.
    refreshInstalled().catch(() => {
      // The list already on screen stays. A failed re-read is not worth an
      // error line over the grid.
    });
    api
      .getLaunchDefaults()
      .then((defaults) => (cloudPlatforms = cloudPlatformSet(defaults.default_emulators)))
      .catch(() => {
        // No defaults readable: no cloud badges. A missing hint badge is
        // not worth an error line over the grid.
      });
  });

  // `entryForKey` falls back to All games when the selected key has gone
  // away (the last game on a platform was uninstalled). Write that fallback
  // back into the store, so the rail's stored key matches what is on screen
  // and reinstalling the platform does not snap the view somewhere else.
  $effect(() => {
    if (selected.key !== librarySelection.key) selectRail(selected.key);
  });

  // A filter change can leave the focus index past the end of the new list.
  $effect(() => {
    if (focusIndex > gridCount - 1) focusIndex = Math.max(0, gridCount - 1);
  });

  function openDetails(row: InstalledGame, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromInstalled(row);
    noteViewed(subjectFromInstalled(row));
  }

  // Q4: the open Details shows a row with no server id, and the row gains
  // one — from the Link picker or from the relink pass after a connect.
  // Re-open Details on the linked row (`{#key subject.romId}` remounts it),
  // so it shows the server-backed actions instead of a stale "unlinked" view.
  $effect(() => {
    const current = subject;
    if (current === null || current.source !== 'installed' || current.romId !== null) return;
    const key = installedKey({ title: current.name, platform: current.platformName } as InstalledGame);
    const row = installed.list.find((r) => installedKey(r) === key);
    if (row !== undefined && row.rom_id !== null) subject = fromInstalled(row);
  });

  function closeDetails() {
    subject = null;
    detailsCloudMode = 'overview';
  }

  async function play(row: InstalledGame) {
    if (row.rom_id === null) return;
    launchError = null;
    // "Launching <title>…" and a refusal are cards (games/run.ts).
    showLaunchOutcome(await playInstalled(row.rom_id, row.title));
  }

  /** An installed card's button: Play, or Locate… (Details) for missing files. */
  function rowPrimary(row: InstalledGame) {
    const quick = quickAction({ installed: true, romId: row.rom_id, missing: isMissing(row) });
    if (quick.kind === 'locate') openDetails(row);
    else void play(row);
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than 120ms — counted from its last move, so
  // browsing across cards does not cross-fade the art on each one.
  const hover = createHoverViewed(
    HOVER_DELAY_MS,
    PREFETCH_DELAY_MS,
    typeof window === 'undefined' ? null : window,
  );

  // Keyboard selection feeds the background through the SAME 120ms
  // dwell as the pointer, so holding an arrow key across the grid does not
  // start a fetch per card. A separate timer from `hover`: sharing one would
  // let a mouse move cancel a keyboard selection's pending swap.
  const focusDwell = createHoverViewed();

  // Precedence is details > focus > hover, enforced rather than left to
  // whichever timer fires last: the overlay blocks both writers while it is
  // open, and a selection change cancels any hover dwell still in flight.
  $effect(() => {
    // Read FIRST and inside the effect, so switching to the keyboard arms the
    // dwell for the current index and switching back to the pointer tears it
    // down. Pointer users never have a keyboard selection; the grow and the
    // art follow the hover instead (user ruling 2026-09-05).
    const directional = inputMode.directional;
    const index = focusIndex;
    if (!active || subject !== null || !directional) return;
    // `rows` is a fresh array on every `installed.list` refresh (replenish,
    // download and native-settings events all publish one). Tracking it here
    // would re-arm this dwell on a background refresh and, 120ms later, snap
    // the art to whatever sits at the current index. Only the SELECTION is a
    // reason to change the background, so the row is read untracked.
    const selected = untrack(() => {
      if (driveKey !== null) return null; // a drive's art is not the background's
      const row = rows[index];
      return row === undefined ? null : subjectFromInstalled(row);
    });
    if (selected === null) return;
    hover.end();
    focusDwell.start(selected);
    return () => focusDwell.end();
  });

  /** The scroll gate's detach, held for the life of the component. */
  let gateDetach: (() => void) | null = null;

  // No reads, so this effect runs once and its teardown fires only when the
  // view unmounts — the one moment the gate should be released.
  $effect(() => () => {
    gateDetach?.();
    gateDetach = null;
  });

  // Design §3: a card builds its background art as it scrolls into view, so
  // the first hover of a game the user has never opened is not the thing
  // that pays for the download, decode and blur. The queue in
  // `backgroundPrefetch.ts` runs one queue for warming and hovering together,
  // three builds at a time, with a hovered card jumping to the front — so
  // speculative art never crowds out the covers the grid is still fetching.
  const warmer = createVisibleWarmer((index) => {
    if (driveKey !== null) return null;
    const row = rows[index];
    return row === undefined ? null : subjectFromInstalled(row);
  });

  $effect(() => {
    // `rows` is read as a dependency, not for its value: a filter, a sort or
    // a refresh replaces the grid's children, and the new ones have never
    // been observed. `warmBackground` de-duplicates by URL, so re-observing
    // a card already warmed costs no request.
    void rows;
    void driveRows;
    const el = grid?.element();
    if (!el) return;
    warmer.observe(el);
    // The scroll gate is attached ONCE, the first time the grid element
    // exists, and lives until the view unmounts. It deliberately does not
    // follow this effect: the effect re-runs whenever the rows change — a
    // finished install, a filter — and that happens while the user is
    // scrolling, which is the one moment the pause must hold.
    gateDetach ??= attachScrollGate(scrollParent(el) ?? window);
    return () => {
      warmer.disconnect();
      // Stop watching AND drop what is still queued for this grid: the view
      // was left, or its rows were replaced, so the cards those warms were
      // for are not the cards anyone is looking at now.
      dropPendingWarms();
    };
  });

  $effect(() => {
    // The windowed grid renders only the rows near the visible part, so a
    // scroll swaps its children too. Re-observing picks up the new cards and
    // lets go of the unmounted ones. No teardown: what is queued is still
    // wanted, and the effect above owns the warmer's lifetime.
    const el = grid?.element();
    if (!el) return;
    void grid?.range();
    warmer.observe(el);
  });

  /** The pointer only feeds the background while the overlay is closed —
   *  the details popup owns the art for as long as it is open — and only
   *  while the pointer IS the active input method: a card scrolling under a
   *  stationary cursor during keyboard navigation must not take the art from
   *  the selected card (user ruling 2026-09-05). A real mouse move switches
   *  the mode back first (`notePointerAt`), so a mouse user is unaffected.
   *  `hoverEnd` stays unconditional — ending a dwell that was never armed is
   *  a no-op, and that is the safe direction. */
  function hoverStart(row: InstalledGame) {
    if (subject !== null || inputMode.directional) return;
    hover.start(subjectFromInstalled(row));
  }

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    searchEl?.focus();
    searchEl?.select();
  }

  /** Arrow-key grid movement. */
  function moveSelection(action: NavDirection) {
    if (subject) return; // grid navigation is suspended while the overlay is open
    focusIndex = grid ? grid.neighbor(focusIndex, action) : moveFocus(focusIndex, action, 1, gridCount);
    // The windowed grid may not have the card rendered; it scrolls to its row.
    grid?.scrollToIndex(focusIndex);
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    // One read of the document, shared by both branches below.
    const ctx = chordContext(document);
    if (leavesSelectMode(e.key, selecting, chordBlocked(ctx) || subject !== null)) {
      e.preventDefault();
      leaveSelectMode();
      return;
    }
    if (shouldFocusSearch(e, ctx)) {
      e.preventDefault();
      focusSearch();
      return;
    }
    // The search box and the sort/size selects own their own arrow keys —
    // and so does an open dialog. Taking them for grid movement would stop
    // `library-sort` and `library-size` changing with Arrow Up/Down.
    if (chordBlocked(ctx)) return;
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      noteDirectional('keyboard');
      moveSelection(action);
    }
  }

  /** Design §3: Escape leaves the search box, so the arrow keys drive the
   *  grid again (the input owns them while it has focus). The text stays —
   *  Escape gives the keyboard back, it does not undo the search. */
  function onSearchKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLInputElement).blur();
  }

  function onSortChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value;
    if (driveKey !== null) driveSort = normalizeDriveSort(value);
    else sort = normalizeSort(value);
  }

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    setCardSize('library', size).catch(() => {
      // Applied for this session; a failed save is not worth an error line.
    });
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="library-section" class="library">
  <RailPane
    entries={railRows}
    testId="library-rail"
    ariaLabel="Library filters"
    onSelect={onRailSelect}
  />

  <div class="body">
    <FoldingHeader testId="library-head" scroller={scrollEl} resetKey={selected.key} bind:folded>
      {#snippet title()}
        {#if storeSel !== null && storeStatus !== null}
          <!-- GameStores README: "GOG · Installed", its count line, chips and actions. -->
          <StoreViewHeader
            status={storeStatus}
            part={storeSel.part}
            games={storeAll}
            installedCount={gameStores.installed(storeSel.source).length}
            now={nowSeconds}
            onBackupAll={backUpStoreAll}
          />
        {:else}
        <header data-testid="library-header" class="title-row">
          <h2 data-testid="library-title">{selected.label}</h2>
          {#if drive !== null}
            <p data-testid="library-count" class="counts">{driveCountLine(drive)}</p>
            {#if chips !== null}
              <!-- GameBackups README: chips for the drive's emulators and firmware;
                   GameStores README: "GOG: 2 games". -->
              <div class="chips" data-testid="library-drive-chips">
                <span class="chip" data-testid="library-drive-emulators">{chips.emulators}</span>
                <span class="chip" data-testid="library-drive-firmware">{chips.firmware}</span>
                {#if gogChip}<span class="chip" data-testid="library-drive-gog">{gogChip}</span>{/if}
              </div>
            {/if}
          {:else}
            <p data-testid="library-count" class="counts">{gameCountLine(items.length)}</p>
            {#if platformBackupIds.length > 0}
              <!-- GameBackups README: "Backup all N…" on a Library platform row, installed games only. -->
              <div class="chips">
                <button
                  data-testid="library-backup-platform"
                  class="chip link backup-all"
                  disabled={backup.disabled}
                  title={backup.reason || undefined}
                  onclick={backUpPlatform}
                >
                  Backup all {platformBackupIds.length}…
                </button>
              </div>
            {/if}
          {/if}
        </header>
        {/if}
      {/snippet}
      {#snippet toolbar()}
        <div class="toolbar">
          <input
            data-testid="library-search"
            class="search"
            type="search"
            placeholder={storeSel !== null ? 'Search by title' : librarySearchPlaceholder(folded, selected.key, selected.label)}
            aria-label={drive !== null ? 'Search games on this drive' : 'Search installed games'}
            bind:this={searchEl}
            bind:value={search}
            onkeydown={onSearchKey}
          />
          <label class="control">
            <span>Sort</span>
            {#if driveKey !== null}
              <select data-testid="library-sort" value={driveSort} onchange={onSortChange}>
                {#each DRIVE_SORTS as option (option)}
                  <option value={option}>{driveSortLabel(option)}</option>
                {/each}
              </select>
            {:else}
              <select data-testid="library-sort" value={sort} onchange={onSortChange}>
                {#each LIBRARY_SORTS as option (option)}
                  <option value={option}>{sortLabel(option)}</option>
                {/each}
              </select>
            {/if}
          </label>
          <label class="control">
            <span>Size</span>
            <select data-testid="library-size" value={uiSettings.cardSizeLibrary} onchange={onSizeChange}>
              {#each CARD_SIZES as option (option)}
                <option value={option}>{cardSizeLabel(option)}</option>
              {/each}
            </select>
          </label>
          <!-- Select: every RomM view; of the store views, GOG › Installed only (GOG backups). -->
          {#if storeSel === null || storeSelectable}
            <button
              data-testid="library-select"
              class="select-btn"
              class:active={selecting}
              aria-pressed={selecting}
              onclick={toggleSelectMode}
            >
              {selecting ? 'Done' : 'Select'}
            </button>
          {/if}
        </div>
      {/snippet}
    </FoldingHeader>

    {#snippet storeCard(game: StoreGame, i: number, rect: Rect, mixedTag: boolean)}
      {@const job = liveJobFor(downloads.entries, game.key)}
      {@const primary = coverPrimary(game, gameStores.status(game.source), job)}
      {@const mark = storeMark(game.source)}
      <GameCard
        testId={`library-store-card-${game.key.replace(':', '-')}`}
        index={i}
        badgeId={game.key.replace(':', '-')}
        title={game.title}
        platform={storeName(game.source)}
        cover={coverPair(null, game.cover || null)}
        installed={game.installed}
        updateLabel={null}
        cloudPlatforms={new Set<string>()}
        labels={uiSettings.cardLabels}
        {rect}
        showPlatform={false}
        focused={i === focusIndex && inputMode.directional}
        selecting={selecting && storeSel !== null}
        selected={storePicks.includes(game.key)}
        onSelect={() => {
          focusIndex = i;
          storePicks = storePicks.includes(game.key) ? storePicks.filter((k) => k !== game.key) : [...storePicks, game.key];
        }}
        onOpen={() => {
          focusIndex = i;
          openStoreDetails(game.key);
        }}
        onPrimary={() => storePrimary(game)}
        onCloud={() => openStoreDetails(game.key, 'cloud')}
        onHoverStart={() => {}}
        onHoverEnd={hover.end}
        extraTags={coverTags(game, job)}
        progress={coverProgress(job)}
        progressTone={coverProgressTone(job)}
        primaryLabel={primary.label}
        primaryDisabled={primary.disabled ?? false}
        primaryTitle={primary.title}
        storeTag={mixedTag ? { name: mark.name, mono: mark.mono, hue: mark.hue } : null}
        showCloud={showCoverCloud(game)}
        faded={coverFaded(game)}
      />
    {/snippet}

    <div class="scroller" bind:this={scrollEl}>
    {#if launchError}
      <p data-testid="library-launch-error" class="error panel" role="alert">{launchError}</p>
      {#if launchError === umuOfferFor}<UmuInstallOffer />{/if}
    {/if}

    {#if driveKey !== null}
      {#if driveError !== null}
        <p data-testid="library-drive-error" class="error panel" role="alert">{driveError}</p>
      {:else if driveLoading}
        <!-- The drive's index is being read; "nothing on this drive" would be a guess. -->
      {:else if driveItems.length === 0}
        <p data-testid="library-empty" class="empty panel">{driveEmptyText(search)}</p>
      {:else}
        <CardGrid
          bind:this={grid}
          gridId="library-grid"
          labels={uiSettings.cardLabels}
          display={uiSettings.cardDisplay}
          cover={(item) => (item.kind === 'romm' ? driveCover(item.game) : coverPair(null, item.game.cover || null))}
          size={uiSettings.cardSizeLibrary}
          items={driveItems}
          key={(item) => (item.kind === 'romm' ? driveGameKey(item.game) : `gog-${item.game.key}`)}
          {selecting}
        >
          {#snippet card(item, i, rect)}
            {#if item.kind === 'romm'}
              {@const game = item.game}
              <GameCard
                testId={`library-drive-card-${game.rom_id}`}
                index={i}
                badgeId={`drive-${game.rom_id}`}
                title={game.name}
                platform={game.platform_name}
                cover={driveCover(game)}
                installed={game.installed}
                updateLabel={null}
                {cloudPlatforms}
                labels={uiSettings.cardLabels}
                {rect}
                focused={i === focusIndex && inputMode.directional}
                {selecting}
                selected={isPicked(selection, game.rom_id)}
                onSelect={(shift) => {
                  focusIndex = i;
                  selection = pick(selection, game.rom_id, shift, visibleIds);
                }}
                onOpen={() => {
                  focusIndex = i;
                  openDriveDetails(game);
                }}
                onPrimary={() => drivePrimary(game)}
                onCloud={() => openDriveDetails(game, 'save')}
                onHoverStart={() => {}}
                onHoverEnd={hover.end}
              />
            {:else}
              {@const g = item.game}
              {@const job = liveJobFor(downloads.entries, g.key)}
              {@const gogMark = storeMark('gog')}
              <!-- GameStores README: a drive's GOG games, each with a GOG tag. -->
              <GameCard
                testId={`library-drive-store-card-${g.key.replace(':', '-')}`}
                index={i}
                badgeId={`drive-${g.key.replace(':', '-')}`}
                title={g.name}
                platform="GOG"
                cover={coverPair(null, g.cover || null)}
                installed={g.installed}
                updateLabel={null}
                cloudPlatforms={new Set<string>()}
                labels={uiSettings.cardLabels}
                {rect}
                showPlatform={false}
                focused={i === focusIndex && inputMode.directional}
                selecting={false}
                onOpen={() => {
                  focusIndex = i;
                  openDriveStoreDetails(g);
                }}
                onPrimary={() => driveStoreAct(g)}
                onCloud={() => openDriveStoreDetails(g)}
                onHoverStart={() => {}}
                onHoverEnd={hover.end}
                extraTags={job ? [{ text: job.status === 'queued' ? 'QUEUED' : `${coverProgress(job)}%`, tone: 'queue' }] : []}
                progress={coverProgress(job)}
                primaryLabel={driveStorePrimary(g, job !== null).label}
                storeTag={{ name: 'GOG', mono: gogMark.mono, hue: gogMark.hue }}
                showCloud={false}
              />
            {/if}
          {/snippet}
        </CardGrid>
      {/if}
    {:else if storeSel !== null}
      {#if storePanelView !== null && storeStatus !== null}
        <StorePanel panel={storePanelView} source={storeSel.source} onAct={storePanelAct} />
      {:else if storeList.length === 0}
        <p data-testid="library-empty" class="empty panel">{noMatchText(storeSel.source, storeSel.part, search)}</p>
      {:else}
        <CardGrid
          bind:this={grid}
          gridId="library-grid"
          labels={uiSettings.cardLabels}
          display={uiSettings.cardDisplay}
          cover={(g) => coverPair(null, g.cover || null)}
          size={uiSettings.cardSizeLibrary}
          items={storeList}
          key={(g) => g.key}
          selecting={selecting && storeSelectable}
        >
          {#snippet card(g, i, rect)}
            {@render storeCard(g, i, rect, false)}
          {/snippet}
        </CardGrid>
      {/if}
    {:else if items.length === 0}
      <p data-testid="library-empty" class="empty panel">
        {search.trim() === '' ? emptyText(selected) : `No games match “${search.trim()}”`}
      </p>
    {:else}
      <CardGrid
        bind:this={grid}
        gridId="library-grid"
        labels={uiSettings.cardLabels}
        display={uiSettings.cardDisplay}
        cover={(item) =>
          item.kind === 'romm'
            ? coverPair(item.row.cover_small_path, item.row.cover_large_path)
            : coverPair(null, item.game.cover || null)}
        size={uiSettings.cardSizeLibrary}
        items={items}
        key={(item) => (item.kind === 'romm' ? installedKey(item.row) : `store-${item.game.key}`)}
        {selecting}
      >
        {#snippet card(item, i, rect)}
          {#if item.kind === 'store'}
            {@render storeCard(item.game, i, rect, true)}
          {:else}
          {@const row = item.row}
          <GameCard
            testId={`library-card-${row.rom_id ?? `x-${i}`}`}
            index={i}
            badgeId={row.rom_id ?? `x-${i}`}
            title={row.title}
            platform={row.platform}
            cover={coverPair(row.cover_small_path, row.cover_large_path)}
            installed={true}
            missing={isMissing(row)}
            updateLabel={updates.labelFor(row.rom_id)}
            {cloudPlatforms}
            labels={uiSettings.cardLabels}
            {rect}
            focused={i === focusIndex && inputMode.directional}
            {selecting}
            selected={row.rom_id !== null && isPicked(selection, row.rom_id)}
            onSelect={(shift) => {
              // A game the server does not know can't be backed up.
              if (row.rom_id === null) return;
              focusIndex = i;
              selection = pick(selection, row.rom_id, shift, visibleIds);
            }}
            onOpen={() => {
              // A click moves the selection, so keyboard navigation resumes
              // from the clicked card and the focus dwell that re-arms when
              // the popup closes targets the SAME game (user ruling
              // 2026-09-05) instead of reverting the art to index 0.
              focusIndex = i;
              openDetails(row);
            }}
            onPrimary={() => rowPrimary(row)}
            onCloud={() => openDetails(row, 'save')}
            onHoverStart={() => hoverStart(row)}
            onHoverEnd={hover.end}
          />
          {/if}
        {/snippet}
      </CardGrid>
    {/if}
    </div>
    {#if selecting && storeSel !== null}
      <!-- GameStores README: Select in GOG › Installed backs games up to a drive. -->
      <SelectionBar
        testId="library-selection"
        count={storePicks.length}
        size={selectedToWrite(storeAll, storePicks)}
        visibleCount={storeList.length}
        allSelected={storeList.length > 0 && storeList.every((g) => storePicks.includes(g.key))}
        onSelectAll={() =>
          (storePicks = storeList.every((g) => storePicks.includes(g.key)) ? [] : storeList.map((g) => g.key))}
        onClear={() => (storePicks = [])}
      >
        {#snippet actions()}
          <button
            data-testid="library-selection-backup"
            class="g-btn"
            disabled={storePicks.length === 0}
            onclick={backUpStoreSelection}
          >
            Backup to drive…
          </button>
        {/snippet}
      </SelectionBar>
    {:else if selecting}
      <SelectionBar
        testId="library-selection"
        count={selection.ids.length}
        size={sizeText(pickedBytes)}
        visibleCount={visibleIds.length}
        allSelected={allPicked(selection, visibleIds)}
        onSelectAll={() => (selection = selectAll(selection, visibleIds))}
        onClear={() => (selection = EMPTY_SELECTION)}
      >
        {#snippet actions()}
          {#if driveKey !== null}
            {#if removeError}
              <span class="bar-error" role="alert">{removeError}</span>
            {/if}
            <button
              data-testid="library-selection-install"
              class="g-btn"
              disabled={toInstall === 0}
              onclick={installSelection}
            >
              {installSelectionLabel(toInstall)}
            </button>
            <!-- Two clicks (GameBackups prototype `bkSelBar`): the second names what goes. -->
            <button
              data-testid="library-selection-remove"
              class="g-btn"
              class:secondary={!confirmRemove}
              class:destructive={!confirmRemove}
              class:danger={confirmRemove}
              disabled={selection.ids.length === 0}
              onclick={removeSelection}
            >
              {confirmRemove ? `Remove ${selection.ids.length} from drive` : 'Remove from drive…'}
            </button>
          {:else}
            <button
              data-testid="library-selection-backup"
              class="g-btn"
              disabled={selection.ids.length === 0 || backup.disabled}
              title={backup.reason || undefined}
              onclick={backUpSelection}
            >
              Backup to drive…
            </button>
          {/if}
        {/snippet}
      </SelectionBar>
    {/if}
  </div>
</section>

{#if subject}
  {#key subject.romId}
    <Details
      {subject}
      initialCloudMode={detailsCloudMode}
      onClose={closeDetails}
      onLibraryPathUnset={() => {}}
    />
  {/key}
{/if}

<style>
  .library {
    display: flex;
    align-items: stretch;
    gap: var(--gutter);
    height: 100%;
    min-height: 0;
  }

  /* The scroller scrolls, not the shell: the rail beside it is a sibling,
     so scrolling here leaves it in place. `scrollIntoView` on a focused card
     walks to that box for the same reason. The header floats over its top
     (`FoldingHeader`). */
  .body {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Reserves the header's unfolded height above the first card, so folding
     never moves one. `scroll-padding-top` keeps a card the keyboard brings
     into view clear of the folded bar. The window swaps cards on its own, so
     the browser's scroll anchoring would fight it. */
  .scroller {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    overflow-anchor: none;
    box-sizing: border-box;
    padding-top: var(--head-h, 116px);
    scroll-padding-top: var(--head-clear, 64px);
  }

  /* The title row matches the Server header's. */
  .title-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
  }

  .title-row h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-h);
  }

  .counts {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .search {
    flex: 1 1 240px;
    min-width: 120px;
    font: inherit;
    height: var(--control-h);
    box-sizing: border-box;
    padding: 0 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--field);
    color: var(--text-h);
  }

  .control {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
  }

  /* The header's chips, as on a Server platform header. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--text-muted);
  }

  .chip.link {
    cursor: pointer;
  }

  .chip.link:hover:not(:disabled) {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .chip.link:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .backup-all {
    white-space: nowrap;
  }

  /* Select / Done, as on Server: the size select's height, filled while on. */
  .select-btn {
    font: inherit;
    font-size: 12px;
    height: var(--control-h);
    box-sizing: border-box;
    padding: 0 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--field);
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .select-btn:hover {
    border-color: var(--primary);
  }

  .select-btn.active {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  /* Remove from drive: the secondary shape with a danger label, then the
     danger fill on the confirming click (as Settings › Backup drives). */
  .g-btn.secondary.destructive {
    color: var(--danger);
  }

  .g-btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
  }

  .g-btn.danger:hover:not(:disabled) {
    background: var(--danger);
    border-color: var(--danger);
    filter: brightness(1.08);
  }

  .bar-error {
    font-size: 12px;
    color: var(--danger);
  }

  .control select {
    font: inherit;
    font-size: 12px;
    height: var(--control-h);
    box-sizing: border-box;
    padding: 0 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--field);
    color: var(--text-h);
  }

  /* Notices sit on their own small panel: never bare text on the art. */
  .empty,
  .error {
    align-self: flex-start;
    margin: var(--gutter) 0 0;
    padding: 10px 14px;
  }

  .empty {
    color: var(--text-muted);
    font-size: 14px;
  }

  .error {
    color: var(--danger);
    font-size: 12px;
  }
</style>
