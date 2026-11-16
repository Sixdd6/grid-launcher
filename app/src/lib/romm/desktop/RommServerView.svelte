<script lang="ts">
  // Library › RomM › Discover and the RomM platform screens (RommStorefront
  // README "RomM › Discover and platform screens"; plan 0.3). They are the
  // old Server view's screens, now inside the Library: Discover unchanged
  // but for its words, and the platform screen with its folding header,
  // Firmware and Emulator chips, Backup all N…, Select, search and Size. A
  // platform's grid is the server's games merged with the installed RomM
  // rows (`platformGames`): Play for installed, Install for the rest, and
  // the installed rows the server doesn't have tagged NOT LINKED or NOT ON
  // ROMM. The rail is the Library's; this component is the body beside it.
  import { untrack } from 'svelte';
  import type { GameSummary, InstalledGame } from '../../api';
  import Details from '../../Details.svelte';
  import UmuInstallOffer from '../../UmuInstallOffer.svelte';
  import { quickAction } from '../../games/quick';
  import { installServerGame, playInstalled, type RunOutcome } from '../../games/run';
  import type { Catalog } from '../../server/catalog.svelte';
  import GameCard from '../../GameCard.svelte';
  import { coverPair } from '../../cards/cover';
  import CardGrid from '../../CardGrid.svelte';
  import FoldingHeader from '../../FoldingHeader.svelte';
  import Discover from '../../discover/Discover.svelte';
  import { serverSearchPlaceholder } from '../../views/fold';
  import { fromInstalled, fromSummary, type DetailsSubject } from '../../details/subject';
  import type { CloudMode } from '../../details/cloud';
  import { moveFocus, type NavDirection } from '../../focus/grid';
  import { isInstalled } from '../../stores/installed.svelte';
  import { updates } from '../../stores/updates.svelte';
  import { navigation } from '../../stores/navigation.svelte';
  import { HOVER_DELAY_MS, PREFETCH_DELAY_MS, subjectFromInstalled, subjectFromSummary } from '../../background';
  import { createHoverViewed } from '../../lastViewedHover';
  import { dropPendingWarms } from '../../backgroundPrefetch';
  import { attachScrollGate, createVisibleWarmer, scrollParent } from '../../visibleWarm';
  import { noteViewed } from '../../stores/lastViewed.svelte';
  import { inputMode, noteDirectional } from '../../stores/inputMode.svelte';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from '../../cards/size';
  import { setCardSize, uiSettings } from '../../stores/uiSettings.svelte';
  import { filterServerGames } from '../../server/search';
  import { firmwareChipLinks, showsChips } from '../../server/header';
  import { savedDefaultFor } from '../../emulators/defaults';
  import { chordBlocked, chordContext, shouldFocusSearch } from '../../views/searchKeys';
  import { pickFolder } from '../../pickers';
  import { installedKey } from '../../library';
  import { isMissing } from '../../library/missing';
  import { platformSlug } from '../../library/rail';
  import { titleContains } from '../../library/sort';
  import SelectionBar from '../../backups/SelectionBar.svelte';
  import { openBackup } from '../../backups/dialogState.svelte';
  import { backupRequest, sizeText } from '../../backups/dialog';
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
  } from '../../backups/selection';
  import { installedOnPlatform, localTagLabel, platformGameCounts, platformGames, type RommGridItem } from '../platformGames';
  import type { RommRailRow } from '../rail';
  import { rommConnectingPanel, rommPlatformCountsLine } from '../words';
  import RommMark from '../RommMark.svelte';
  import RommPlatformHeader from './RommPlatformHeader.svelte';

  let {
    active,
    catalog,
    row,
    connecting,
    host,
    liveServer,
    installedRows,
  }: {
    /** The Library is the front view and this screen is selected. */
    active: boolean;
    /** The Library's server catalog: the platforms, a platform's games, the defaults. */
    catalog: Catalog;
    /** The selected platform row; `null` for Discover. */
    row: RommRailRow | null;
    /** RomM is connecting at startup: Discover shows its connecting panel. */
    connecting: boolean;
    /** The server's host, for the connecting panel. */
    host: string;
    /** The connected server's URL (matches installed rows to server games). */
    liveServer: string;
    /** The installed RomM rows (`rommInstalledRows`). */
    installedRows: readonly InstalledGame[];
  } = $props();

  let discover = $derived(row === null);
  let serverPlatform = $derived(row?.platform ?? null);
  let platformId = $derived(serverPlatform?.id ?? null);
  /** The platform's label: the key the installed registry and the defaults use. */
  let platformName = $derived(row?.label ?? '');

  let discoverView = $state<ReturnType<typeof Discover> | null>(null);
  let search = $state('');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let scrollEl = $state<HTMLElement | null>(null);
  let folded = $state(false);
  let subject = $state<DetailsSubject | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  // A refused launch that needs umu-launcher: shown in `server-error` beside
  // the install offer. Every other refusal is a card.
  let actionError = $state<string | null>(null);
  let umuOfferFor = $state<string | null>(null);

  let libraryPathInput = $state('');
  let showLibraryBanner = $derived(catalog.libraryPathUnset);
  let libraryPathSaving = $state(false);
  let libraryPathError = $state<string | null>(null);

  // The server's games for this platform, once the catalog has them; none
  // for a platform only installed games have.
  let serverGames = $derived(platformId !== null && catalog.activePlatform === platformId ? catalog.games : []);
  // While `listGames` runs, the list belongs to the previous platform: the
  // counts line and the empty message would both state something false.
  let gamesLoading = $derived(platformId !== null && (catalog.activePlatform !== platformId || catalog.gamesLoading));
  let localRows = $derived(row === null ? [] : installedOnPlatform(installedRows, row.slug ?? '', catalog.platforms));
  let merged = $derived(platformGames(serverGames, localRows, liveServer));
  let counts = $derived(platformGameCounts(merged));
  let shownIds = $derived(new Set(filterServerGames(serverGames, search).map((g) => g.id)));
  let visible = $derived(
    merged.filter((item) => (item.kind === 'server' ? shownIds.has(item.game.id) : titleContains(item.row.title, search))),
  );

  // Select mode (GameBackups README): the server's games only; leaving the
  // platform, or Discover, leaves it.
  let selecting = $state(false);
  let selection = $state.raw<Selection>(EMPTY_SELECTION);
  let visibleIds = $derived(visible.flatMap((item) => (item.kind === 'server' ? [item.game.id] : [])));
  let sizeById = $derived(new Map(serverGames.map((g) => [g.id, g.fs_size_bytes])));
  let pickedBytes = $derived(totalBytes(selection, (id) => sizeById.get(id) ?? 0));

  let cloudPlatforms = $derived(catalog.cloudPlatforms);
  let defaultEmulator = $derived(savedDefaultFor(catalog.defaultEmulators, platformName));
  let chipsShown = $derived(showsChips(platformName, row?.slug ?? ''));
  let firmware = $derived(catalog.firmware);
  let firmwareLinks = $derived(firmwareChipLinks(firmware));

  const labelFor = (game: GameSummary) => catalog.labelFor(game);
  const platformOf = (game: GameSummary) => catalog.platformOf(game);

  // A platform row with a server platform loads its games; the header shows
  // the new platform's name at once.
  $effect(() => {
    const id = platformId;
    if (id === null) return;
    untrack(() => {
      if (catalog.activePlatform === id && !catalog.gamesLoading && catalog.games.length > 0) return;
      void catalog.selectPlatform(id).then((ok) => {
        if (ok) focusIndex = 0;
      });
    });
  });

  // A new row: a fresh search, focus and select mode.
  $effect(() => {
    void row?.key;
    untrack(() => {
      search = '';
      focusIndex = 0;
      leaveSelectMode();
    });
  });

  // The emulator chip sends the user to Settings › Systems to change the very
  // mapping it displays: re-read the defaults whenever this screen comes back.
  $effect(() => {
    if (active) void catalog.loadLaunchDefaults();
  });

  $effect(() => {
    if (focusIndex > visible.length - 1) focusIndex = Math.max(0, visible.length - 1);
  });

  // One firmware status call per platform. Reset first so the chip reads
  // "checking…" rather than the previous platform's answer.
  $effect(() => {
    const id = platformId;
    const name = platformName;
    const shown = chipsShown;
    untrack(() => {
      catalog.firmware = null;
      if (id === null || name === '' || !shown) return;
      void catalog.refreshFirmwareStatus(id, name);
    });
  });

  // While mounted: a firmware install from Settings › Systems announces its
  // end and the chip re-reads; an emulator install or first-run setup's
  // defaults re-read the emulator chip.
  $effect(() => catalog.watch());

  function leaveSelectMode() {
    selecting = false;
    selection = EMPTY_SELECTION;
  }

  function toggleSelectMode() {
    if (selecting) leaveSelectMode();
    else selecting = true;
  }

  // A reload can drop a game: the selection never counts one off screen.
  $effect(() => {
    const ids = serverGames.map((g) => g.id);
    const next = keepOnly(untrack(() => selection), ids);
    if (next !== untrack(() => selection)) selection = next;
  });

  /** "Backup to drive…" on the selection bar. */
  function backUpSelection() {
    if (selection.ids.length === 0 || platformId === null) return;
    openBackup({
      request: backupRequest({ kind: 'games', count: selection.ids.length }, selection.ids, [platformId]),
      onQueued: leaveSelectMode,
    });
  }

  /** "Backup all N…" in the platform header: every server game of the platform. */
  function backUpPlatform() {
    if (platformId === null || serverGames.length === 0) return;
    openBackup({
      request: backupRequest(
        { kind: 'platform', platform: platformName, count: serverGames.length },
        serverGames.map((g) => g.id),
        [platformId],
      ),
    });
  }

  function openServerDetails(game: GameSummary, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromSummary(game, labelFor(game), platformOf(game)?.slug ?? '');
    noteViewed(subjectFromSummary(game));
  }

  function openRowDetails(r: InstalledGame, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromInstalled(r);
    noteViewed(subjectFromInstalled(r));
  }

  function closeDetails() {
    subject = null;
    detailsCloudMode = 'overview';
  }

  function showOutcome(outcome: RunOutcome) {
    if (outcome.kind === 'ok') return;
    const missing = outcome.kind === 'needs-umu';
    actionError = missing ? outcome.reason : null;
    umuOfferFor = missing ? outcome.reason : null;
    if (outcome.kind === 'needs-library-folder') catalog.libraryPathUnset = true;
  }

  /** Design §6: the hover primary is Install when not installed, Play when installed. */
  async function primary(game: GameSummary) {
    actionError = null;
    const quick = quickAction({ installed: isInstalled(game, labelFor(game)), romId: game.id });
    showOutcome(quick.kind === 'play' ? await playInstalled(game.id, game.name) : await installServerGame(game.id, game.name));
  }

  /** An installed row only on this computer: Play, or Locate… (Details) for missing files. */
  async function localPrimary(r: InstalledGame) {
    const quick = quickAction({ installed: true, romId: r.rom_id, missing: isMissing(r) });
    if (quick.kind === 'locate' || r.rom_id === null) {
      openRowDetails(r);
      return;
    }
    actionError = null;
    showOutcome(await playInstalled(r.rom_id, r.title));
  }

  function itemSubject(item: RommGridItem | undefined) {
    if (item === undefined) return null;
    return item.kind === 'server' ? subjectFromSummary(item.game) : subjectFromInstalled(item.row);
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than 120ms.
  const hover = createHoverViewed(HOVER_DELAY_MS, PREFETCH_DELAY_MS, typeof window === 'undefined' ? null : window);
  // Keyboard selection feeds the background through the same dwell, on its own timer.
  const focusDwell = createHoverViewed();

  // Precedence is details > focus > hover.
  $effect(() => {
    const directional = inputMode.directional;
    const index = focusIndex;
    if (!active || discover || subject !== null || !directional) return;
    // Only the SELECTION is a reason to change the background: the list is
    // read untracked, so a reload does not re-arm the dwell.
    const selected = untrack(() => itemSubject(visible[index]));
    if (selected === null) return;
    hover.end();
    focusDwell.start(selected);
    return () => focusDwell.end();
  });

  /** The scroll gate's detach, held for the life of the component. */
  let gateDetach: (() => void) | null = null;
  $effect(() => () => {
    gateDetach?.();
    gateDetach = null;
  });

  // Design §3: a card builds its background art as it scrolls into view.
  const warmer = createVisibleWarmer((index) => itemSubject(visible[index]));

  $effect(() => {
    void visible;
    const el = grid?.element();
    if (!el) return;
    warmer.observe(el);
    gateDetach ??= attachScrollGate(scrollParent(el) ?? window);
    return () => {
      warmer.disconnect();
      dropPendingWarms();
    };
  });

  $effect(() => {
    const el = grid?.element();
    if (!el) return;
    void grid?.range();
    warmer.observe(el);
  });

  function hoverStart(item: RommGridItem) {
    if (subject !== null || inputMode.directional) return;
    const s = itemSubject(item);
    if (s !== null) hover.start(s);
  }

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    if (discover) return; // Discover has no search box
    searchEl?.focus();
    searchEl?.select();
  }

  function moveSelection(action: NavDirection) {
    if (subject) return;
    focusIndex = grid ? grid.neighbor(focusIndex, action) : moveFocus(focusIndex, action, 1, visible.length);
    grid?.scrollToIndex(focusIndex);
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    const ctx = chordContext(document);
    if (discover) {
      // No search box and no grid: ← / → / Enter drive the coverflow while
      // nothing else has the keyboard.
      if (subject || chordBlocked(ctx) || e.ctrlKey || e.metaKey || e.altKey) return;
      // A focused rail row (the Discover row just clicked) gives up ← / →
      // too, but keeps Enter, which presses it.
      const focused = document.activeElement;
      const free = !focused || focused === document.body;
      const onRail = focused instanceof HTMLElement && focused.closest('[data-testid="library-rail"]') !== null;
      if (!free && !(onRail && e.key !== 'Enter')) return;
      if (discoverView?.handleKey(e)) noteDirectional('keyboard');
      return;
    }
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
    if (chordBlocked(ctx)) return;
    const map: Record<string, NavDirection> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      noteDirectional('keyboard');
      moveSelection(action);
    }
  }

  /** Escape leaves the search box; the text stays. */
  function onSearchKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLInputElement).blur();
  }

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    // The RomM screens use the Library's card size (plan 0.3).
    setCardSize('library', size).catch(() => {});
  }

  function openSystems(system: string) {
    navigation.goSettings('systems', system === '' ? {} : { system });
  }

  async function browseLibraryPath() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) libraryPathInput = picked;
  }

  async function saveLibraryPath() {
    libraryPathError = null;
    libraryPathSaving = true;
    try {
      await catalog.saveLibraryPath(libraryPathInput);
    } catch (err) {
      libraryPathError = err instanceof Error ? err.message : String(err);
    } finally {
      libraryPathSaving = false;
    }
  }

  const localBadge = (r: InstalledGame) => `local-${platformSlug(r.title) || 'game'}`;
  let connectingPanel = $derived(rommConnectingPanel(host));
</script>

<svelte:window onkeydown={onKey} />

<div data-testid={discover ? 'romm-discover-view' : 'romm-platform-view'} class="body">
  {#if showLibraryBanner && !connecting}
    <div data-testid="library-path-banner" class="library-banner panel">
      <span>Set a library folder to install games.</span>
      <input data-testid="library-path-input" bind:value={libraryPathInput} placeholder="/path/to/library" disabled={libraryPathSaving} />
      <button data-testid="library-path-browse" class="browse-secondary" disabled={libraryPathSaving} onclick={browseLibraryPath}>
        Browse…
      </button>
      <button data-testid="library-path-save" disabled={libraryPathSaving || !libraryPathInput.trim()} onclick={saveLibraryPath}>
        {libraryPathSaving ? 'Saving…' : 'Save'}
      </button>
      {#if libraryPathError}<span class="banner-error" role="alert">{libraryPathError}</span>{/if}
    </div>
  {/if}

  {#if discover}
    {#if connecting}
      <!-- Prototype: Discover while the startup check runs. -->
      <div data-testid="romm-connecting" class="state">
        <span class="ico"><RommMark size="lg" /></span>
        <h3><span class="spin" aria-hidden="true"></span> {connectingPanel.title}</h3>
        <p>{connectingPanel.text}</p>
      </div>
    {:else}
      {#if actionError}
        <p data-testid="server-error" class="error panel discover-error" role="alert">{actionError}</p>
        {#if actionError === umuOfferFor}<UmuInstallOffer />{/if}
      {/if}
      <Discover
        bind:this={discoverView}
        active={active && discover}
        {labelFor}
        onOpen={(game) => openServerDetails(game)}
        onPrimary={(game) => primary(game)}
        onOpenSystems={() => openSystems('')}
      />
    {/if}
  {:else}
    <FoldingHeader testId="server-head" scroller={scrollEl} resetKey={row?.key} bind:folded>
      {#snippet title()}
        <RommPlatformHeader
          name={platformName}
          counts={gamesLoading ? null : rommPlatformCountsLine(row?.count ?? counts.onRomm, counts.installed, counts.onlyHere)}
          {chipsShown}
          {firmware}
          firmwareShown={platformId !== null}
          {firmwareLinks}
          {defaultEmulator}
          backupCount={gamesLoading ? 0 : serverGames.length}
          onBackupAll={backUpPlatform}
          onOpenSystems={() => openSystems(platformName)}
        />
      {/snippet}
      {#snippet toolbar()}
        <div class="toolbar">
          <input
            data-testid="server-search"
            class="search"
            type="search"
            placeholder={serverSearchPlaceholder(folded, platformName)}
            aria-label="Search this platform"
            bind:this={searchEl}
            bind:value={search}
            onkeydown={onSearchKey}
          />
          <label class="control">
            <span>Size</span>
            <select data-testid="server-size" value={uiSettings.cardSizeLibrary} onchange={onSizeChange}>
              {#each CARD_SIZES as option (option)}
                <option value={option}>{cardSizeLabel(option)}</option>
              {/each}
            </select>
          </label>
          <button data-testid="server-select" class="select-btn" class:active={selecting} aria-pressed={selecting} onclick={toggleSelectMode}>
            {selecting ? 'Done' : 'Select'}
          </button>
        </div>
      {/snippet}
    </FoldingHeader>

    <div class="scroller" bind:this={scrollEl}>
      {#if actionError}
        <p data-testid="server-error" class="error panel" role="alert">{actionError}</p>
        {#if actionError === umuOfferFor}<UmuInstallOffer />{/if}
      {/if}

      {#if gamesLoading && localRows.length === 0}
        <!-- The list on screen belongs to the previous platform. -->
      {:else if visible.length === 0}
        <p data-testid="server-empty" class="empty panel">
          {search.trim() === '' ? 'This platform has no games' : `No games match “${search.trim()}”`}
        </p>
      {:else}
        <CardGrid
          bind:this={grid}
          gridId="server-grid"
          labels={uiSettings.cardLabels}
          display={uiSettings.cardDisplay}
          cover={(item) =>
            item.kind === 'server'
              ? coverPair(item.game.path_cover_small, item.game.path_cover_large)
              : coverPair(item.row.cover_small_path, item.row.cover_large_path)}
          size={uiSettings.cardSizeLibrary}
          items={visible}
          key={(item) => (item.kind === 'server' ? item.game.id : `local-${installedKey(item.row)}`)}
          {selecting}
        >
          {#snippet card(item, i, rect)}
            {#if item.kind === 'server'}
              {@const game = item.game}
              <GameCard
                testId={`game-card-${game.id}`}
                index={i}
                badgeId={game.id}
                title={game.name}
                platform={platformName}
                cover={coverPair(game.path_cover_small, game.path_cover_large)}
                installed={item.installed !== null || isInstalled(game, platformName)}
                updateLabel={updates.labelFor(game.id)}
                {cloudPlatforms}
                labels={uiSettings.cardLabels}
                {rect}
                showPlatform={false}
                focused={i === focusIndex && inputMode.directional}
                {selecting}
                selected={isPicked(selection, game.id)}
                onSelect={(shift) => {
                  focusIndex = i;
                  selection = pick(selection, game.id, shift, visibleIds);
                }}
                onOpen={() => {
                  focusIndex = i;
                  openServerDetails(game);
                }}
                onPrimary={() => primary(game)}
                onCloud={() => openServerDetails(game, 'save')}
                onHoverStart={() => hoverStart(item)}
                onHoverEnd={hover.end}
              />
            {:else}
              {@const r = item.row}
              <!-- An installed game RomM doesn't have (decision 15): never selectable. -->
              <GameCard
                testId={`game-card-${localBadge(r)}`}
                index={i}
                badgeId={localBadge(r)}
                title={r.title}
                platform={r.platform}
                cover={coverPair(r.cover_small_path, r.cover_large_path)}
                installed={true}
                missing={isMissing(r)}
                updateLabel={null}
                {cloudPlatforms}
                labels={uiSettings.cardLabels}
                {rect}
                showPlatform={false}
                focused={i === focusIndex && inputMode.directional}
                {selecting}
                selected={false}
                onSelect={() => {}}
                onOpen={() => {
                  focusIndex = i;
                  openRowDetails(r);
                }}
                onPrimary={() => localPrimary(r)}
                onCloud={() => openRowDetails(r, 'save')}
                onHoverStart={() => hoverStart(item)}
                onHoverEnd={hover.end}
                extraTags={[{ text: localTagLabel(item.tag), tone: 'local' }]}
              />
            {/if}
          {/snippet}
        </CardGrid>
      {/if}
    </div>
    {#if selecting}
      <SelectionBar
        testId="server-selection"
        count={selection.ids.length}
        size={sizeText(pickedBytes)}
        visibleCount={visibleIds.length}
        allSelected={allPicked(selection, visibleIds)}
        onSelectAll={() => (selection = selectAll(selection, visibleIds))}
        onClear={() => (selection = EMPTY_SELECTION)}
      >
        {#snippet actions()}
          <button data-testid="server-selection-backup" class="g-btn" disabled={selection.ids.length === 0} onclick={backUpSelection}>
            Backup to drive…
          </button>
        {/snippet}
      </SelectionBar>
    {/if}
  {/if}
</div>

{#if subject}
  {#key `${subject.source}-${subject.romId ?? subject.name}`}
    <Details
      {subject}
      initialCloudMode={detailsCloudMode}
      onClose={closeDetails}
      onLibraryPathUnset={() => {
        catalog.libraryPathUnset = true;
      }}
    />
  {/key}
{/if}

<style>
  /* The header floats over the top of the scroller (`FoldingHeader`); the
     scroller scrolls, so the Library rail beside it stays put. */
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
     never moves one (as the Library's scroller). */
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

  .discover-error {
    margin: 0 0 var(--gutter);
  }

  /* Prototype `stState`: the connecting panel in Discover's place. */
  .state {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 34px 24px;
    text-align: center;
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
  }

  .state h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 16px;
    color: var(--text-h);
  }

  .state p {
    margin: 0;
    max-width: 480px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .ico {
    display: inline-grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-row);
    background: var(--field);
  }

  .spin {
    display: inline-block;
    width: 10px;
    height: 10px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }

  .library-banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 10px 16px;
    margin-bottom: var(--gutter);
    color: var(--text-h);
    font-size: 13px;
  }

  .library-banner input {
    flex: 1 1 240px;
    min-width: 160px;
    font: inherit;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--field);
    color: var(--text-h);
  }

  .library-banner button {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
    white-space: nowrap;
  }

  .library-banner button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .browse-secondary {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .banner-error {
    color: var(--danger);
    flex-basis: 100%;
  }
</style>
