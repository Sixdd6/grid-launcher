<script lang="ts">
  import { untrack } from 'svelte';
  import {
    api,
    EMULATOR_INSTALLED_EVENT,
    FIRMWARE_PASS_FINISHED_EVENT,
    SETUP_DEFAULTS_EVENT,
    type FirmwarePassFinished,
    type GameSummary,
    type Platform,
  } from './api';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Details from './Details.svelte';
  import UmuInstallOffer from './UmuInstallOffer.svelte';
  import { isUmuMissing } from './launchError';
  import GameCard from './GameCard.svelte';
  import { coverPair } from './cards/cover';
  import CardGrid from './CardGrid.svelte';
  import FoldingHeader from './FoldingHeader.svelte';
  import Discover from './discover/Discover.svelte';
  import { serverSearchPlaceholder } from './views/fold';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import { fromSummary } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { isInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { hostOf } from './shell';
  import { session, retry } from './stores/session.svelte';
  import { HOVER_DELAY_MS, PREFETCH_DELAY_MS, subjectFromSummary } from './background';
  import { createHoverViewed } from './lastViewedHover';
  import { dropPendingWarms } from './backgroundPrefetch';
  import { attachScrollGate, createVisibleWarmer, scrollParent } from './visibleWarm';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { inputMode, noteDirectional } from './stores/inputMode.svelte';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { filterServerGames } from './server/search';
  import {
    emulatorChipLabel,
    firmwareChipLabel,
    firmwareChipLinks,
    platformCountsLine,
    showsChips,
    type FirmwareChipState,
  } from './server/header';
  import { platformLabel } from './server/platformLabel';
  import { savedDefaultFor } from './emulators/defaults';
  import { chordBlocked, chordContext, shouldFocusSearch } from './views/searchKeys';
  import { pickFolder } from './pickers';

  let {
    active,
    onOpenSystems = () => {},
  }: {
    active: boolean;
    /** The emulator and firmware chips open Settings › Systems on this
     *  platform's row (its display label, the key its default is stored under). */
    onOpenSystems?: (system: string) => void;
  } = $props();

  let platforms = $state<Platform[]>([]);
  // `$state.raw`: a platform can hold thousands of games and the list is only
  // ever replaced whole, so a deep proxy over every game (and its screenshot
  // and genre arrays) cost time on load and on every read for nothing.
  let games = $state.raw<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  // Server › Discover is the first rail row and the default view; a
  // platform row turns it off, the Discover row back on.
  let discover = $state(true);
  let discoverView = $state<ReturnType<typeof Discover> | null>(null);
  let search = $state('');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let scrollEl = $state<HTMLElement | null>(null);
  let folded = $state(false);
  let detailsGame = $state<GameSummary | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  // Every inline failure the grid can produce: a refused install AND a
  // refused launch, both from `primary()`. Shown in `server-error`.
  let actionError = $state<string | null>(null);
  // The action error that offers the umu-launcher install (a Proton launch
  // with no umu-run); `null` for any other.
  let umuOfferFor = $state<string | null>(null);
  let cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());
  // Platform name -> default emulator name, for the header's emulator chip.
  let defaultEmulators = $state<Record<string, string>>({});
  let firmware = $state<FirmwareChipState>(null);
  // The firmware chip is a link (to the platform's row in Settings ›
  // Systems, where firmware installs) only when the server has firmware.
  let firmwareLinks = $derived(firmwareChipLinks(firmware));
  // The game list belongs to the previous platform until `listGames`
  // answers. The counts line and the empty message would both state
  // something false about the new platform in that gap, so neither renders.
  let gamesLoading = $state(false);
  // The header's own error line: a refused Install. Never a path or a token —
  // the backend's command errors carry neither, and nothing is appended.

  let libraryPathInput = $state('');
  let showLibraryBanner = $state(false);
  let libraryPathSaving = $state(false);
  let libraryPathError = $state<string | null>(null);

  let activePlatformRow = $derived(platforms.find((p) => p.id === activePlatform) ?? null);
  let activePlatformName = $derived(activePlatformRow ? platformLabel(activePlatformRow) : '');
  let visible = $derived(filterServerGames(games, search));
  let installedCount = $derived(
    games.filter((game) => isInstalled(game, activePlatformName)).length,
  );
  let defaultEmulator = $derived(savedDefaultFor(defaultEmulators, activePlatformName));
  // Native and Windows platforms, and the Emulators shelf, show neither the
  // emulator nor the firmware chip, and their firmware is never asked about.
  let chipsShown = $derived(showsChips(activePlatformName, activePlatformRow?.slug ?? ''));

  // Design §5: the Server rail is the shared `RailPane`. The row keeps the
  // `platform-btn-<id>` id the specs already use and carries the §11
  // `data-rail` value beside it.
  let railRows = $derived([
    {
      key: 'discover',
      testId: 'server-rail-discover',
      dataRail: 'server-rail-discover',
      label: 'Discover',
      selected: discover,
      divider: true,
    } satisfies RailPaneEntry<string>,
    ...platforms.map(
      (p): RailPaneEntry<string> => ({
        key: String(p.id),
        testId: `platform-btn-${p.id}`,
        countTestId: `server-rail-count-${p.id}`,
        dataRail: `server-rail-${p.id}`,
        label: platformLabel(p),
        count: p.rom_count,
        selected: !discover && p.id === activePlatform,
      }),
    ),
  ]);

  /** The label a game's platform shows, which is also the name the
   *  installed registry and the defaults know it by. Discover lists games of
   *  every platform, so it cannot use the active one. */
  function platformOf(game: GameSummary): Platform | null {
    return platforms.find((p) => p.id === game.platform_id) ?? null;
  }

  function labelFor(game: GameSummary): string {
    const row = platformOf(game);
    return row ? platformLabel(row) : game.platform_display_name;
  }

  $effect(() => {
    if (!session.connected) return; // re-runs on reconnect: session.connected is read above
    // No platform is selected for the user: Server opens on Discover.
    api.listPlatforms().then((p) => {
      platforms = p;
    });
    refreshInstalled();
    checkLibraryPath();
    loadLaunchDefaults();
  });

  // The emulator chip sends the user to Settings › Systems to change the very
  // mapping it displays, so re-read the defaults every time this view comes
  // back to the front. The cloud badges derive from the same call.
  $effect(() => {
    if (active && session.connected) loadLaunchDefaults();
  });

  function loadLaunchDefaults() {
    api
      .getLaunchDefaults()
      .then((defaults) => {
        cloudPlatforms = cloudPlatformSet(defaults.default_emulators);
        defaultEmulators = defaults.default_emulators;
      })
      .catch(() => {
        // No defaults readable: no cloud badges and a "No default emulator"
        // chip. Both are honest fallbacks, neither is worth an error line.
      });
  }

  $effect(() => {
    if (focusIndex > visible.length - 1) focusIndex = Math.max(0, visible.length - 1);
  });

  // One status call per platform selection. Reset to `null` first so the
  // chip reads "checking…" rather than the previous platform's answer.
  // One status call per platform selection, and one more per reconnect:
  // a status that was refused offline reads 'unavailable' forever otherwise.
  $effect(() => {
    const id = activePlatform;
    const name = activePlatformName;
    firmware = null;
    if (discover || id === null || name === '' || !chipsShown) return;
    void session.connected; // re-runs on reconnect
    refreshFirmwareStatus(id, name);
  });

  /** One status read, with the staleness guard both callers need. */
  function refreshFirmwareStatus(id: number, name: string) {
    api
      .platformFirmwareStatus(id, name)
      .then((status) => {
        if (activePlatform === id) firmware = status;
      })
      .catch(() => {
        // Unreachable or refused. Say so: "checking…" forever would claim a
        // call is still in flight when none is, and reporting "no firmware"
        // would claim an answer we never got.
        if (activePlatform === id) firmware = 'unavailable';
      });
  }

  // A firmware install started from Settings › Systems announces its end; the chip
  // re-reads the platform on screen.
  $effect(() => {
    let unlisten: UnlistenFn | null = null;
    let stopped = false;
    listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (e) => {
      if (e.payload.platform_id === activePlatform && activePlatformName !== '' && chipsShown) {
        refreshFirmwareStatus(activePlatform, activePlatformName);
      }
    }).then((un) => {
      if (stopped) un();
      else unlisten = un;
    });
    return () => {
      stopped = true;
      unlisten?.();
    };
  });

  // Defaults also change while this view is in front: an emulator install
  // assigns them automatically, and first-run setup applies the ones the user
  // chose just after (`setup-defaults`). Without these the header's emulator
  // chip read "No default emulator" until the view was left and re-entered.
  $effect(() => {
    const offs = [
      listen(EMULATOR_INSTALLED_EVENT, () => loadLaunchDefaults()),
      listen(SETUP_DEFAULTS_EVENT, () => loadLaunchDefaults()),
    ];
    return () => {
      for (const off of offs) off.then((un) => un());
    };
  });

  async function checkLibraryPath() {
    try {
      const path = await api.getLibraryPath();
      showLibraryBanner = path.trim() === '';
    } catch {
      // Unreadable config: leave the banner as-is rather than nag with a
      // guess — install errors will surface the real problem if there is one.
    }
  }

  function showDiscover() {
    discover = true;
    search = '';
  }

  async function selectPlatform(id: number) {
    discover = false;
    activePlatform = id;
    search = '';
    // The header renders the new platform's name at once; `games` still
    // holds the old platform's rows until the call below answers.
    gamesLoading = true;
    let g: GameSummary[];
    try {
      g = await api.listGames(id);
    } catch {
      if (activePlatform === id) {
        games = [];
        gamesLoading = false;
      }
      return;
    }
    if (activePlatform !== id) return; // superseded by a newer selection
    games = g;
    gamesLoading = false;
    focusIndex = 0;
  }

  function openDetails(game: GameSummary, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    detailsGame = game;
    noteViewed(subjectFromSummary(game));
  }

  function closeDetails() {
    detailsGame = null;
    detailsCloudMode = 'overview';
  }

  /** Design §6: the hover primary is Install when not installed, Play when
   *  installed. Both report their failure inline rather than silently. */
  async function primary(game: GameSummary) {
    actionError = null;
    try {
      if (isInstalled(game, labelFor(game))) await api.launchGame(game.id);
      else await api.installGame(game.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      actionError = message;
      umuOfferFor = isUmuMissing(err) ? message : null;
      if (message.includes('library folder')) showLibraryBanner = true;
    }
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
    if (!active || detailsGame !== null || !directional) return;
    // `visible` is a fresh array on every games reload and every search
    // keystroke. Tracking it would re-arm this dwell on a refresh and, 120ms
    // later, snap the art to whatever sits at the current index. Only the
    // SELECTION is a reason to change the background, so the game is read
    // untracked.
    const selected = untrack(() => {
      const game = visible[index];
      return game === undefined ? null : subjectFromSummary(game);
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
    const game = visible[index];
    return game === undefined ? null : subjectFromSummary(game);
  });

  $effect(() => {
    // `visible` is read as a dependency, not for its value: a search
    // keystroke or a games reload replaces the grid's children, and the new
    // ones have never been observed. `warmBackground` de-duplicates by URL,
    // so re-observing a card already warmed costs no request.
    void visible;
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
  function hoverStart(game: GameSummary) {
    if (detailsGame !== null || inputMode.directional) return;
    hover.start(subjectFromSummary(game));
  }

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    if (discover) return; // Discover has no search box
    searchEl?.focus();
    searchEl?.select();
  }

  /** Arrow-key grid movement. */
  function moveSelection(action: NavDirection) {
    if (detailsGame) return; // grid navigation is suspended while the overlay is open
    focusIndex = grid ? grid.neighbor(focusIndex, action) : moveFocus(focusIndex, action, 1, visible.length);
    // The windowed grid may not have the card rendered; it scrolls to its row.
    grid?.scrollToIndex(focusIndex);
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    // One read of the document, shared by both branches below.
    const ctx = chordContext(document);
    if (discover) {
      // No search box and no grid: Ctrl+F does nothing here. ← / → / Enter
      // drive the coverflow while nothing else has the keyboard (the
      // coverflow handles its own keys when it is focused).
      if (detailsGame || chordBlocked(ctx) || e.ctrlKey || e.metaKey || e.altKey) return;
      // A focused rail row (the Discover row just clicked) gives up ← / →
      // too, but keeps Enter, which presses it.
      const focused = document.activeElement;
      const free = !focused || focused === document.body;
      const onRail = focused instanceof HTMLElement && focused.closest('[data-testid="server-rail"]') !== null;
      if (!free && !(onRail && e.key !== 'Enter')) return;
      if (discoverView?.handleKey(e)) noteDirectional('keyboard');
      return;
    }
    if (shouldFocusSearch(e, ctx)) {
      e.preventDefault();
      focusSearch();
      return;
    }
    // The search box and the size select own their own arrow keys — and so
    // does an open dialog. Taking them for grid movement would stop
    // `server-size` changing with Arrow Up/Down.
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

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    setCardSize('server', size).catch(() => {
      // Applied for this session; a failed save is not worth an error line.
    });
  }

  async function browseLibraryPath() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) libraryPathInput = picked;
  }

  async function saveLibraryPath() {
    libraryPathError = null;
    libraryPathSaving = true;
    try {
      await api.setLibraryPath(libraryPathInput.trim());
      showLibraryBanner = false;
    } catch (err) {
      libraryPathError = err instanceof Error ? err.message : String(err);
    } finally {
      libraryPathSaving = false;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="server-section" class="server">
  {#if showLibraryBanner}
    <div data-testid="library-path-banner" class="library-banner panel">
      <span>Set a library folder to install games.</span>
      <input
        data-testid="library-path-input"
        bind:value={libraryPathInput}
        placeholder="/path/to/library"
        disabled={libraryPathSaving}
      />
      <button
        data-testid="library-path-browse"
        class="browse-secondary"
        disabled={libraryPathSaving}
        onclick={browseLibraryPath}
      >
        Browse…
      </button>
      <button data-testid="library-path-save" disabled={libraryPathSaving || !libraryPathInput.trim()} onclick={saveLibraryPath}>
        {libraryPathSaving ? 'Saving…' : 'Save'}
      </button>
      {#if libraryPathError}<span class="banner-error" role="alert">{libraryPathError}</span>{/if}
    </div>
  {/if}

  {#if !session.connected}
    <div data-testid="server-offline" class="offline panel">
      Not connected to {hostOf(session.serverUrl)}
      <button data-testid="server-retry" onclick={() => retry()}>Retry</button>
    </div>
  {:else}
    <div class="columns">
      <RailPane
        entries={railRows}
        testId="server-rail"
        ariaLabel="Server platforms"
        onSelect={(key) => (key === 'discover' ? showDiscover() : selectPlatform(Number(key)))}
      />

      {#if discover}
        <div class="body">
          {#if actionError}
            <p data-testid="server-error" class="error panel discover-error" role="alert">{actionError}</p>
            {#if actionError === umuOfferFor}<UmuInstallOffer />{/if}
          {/if}
          <Discover
            bind:this={discoverView}
            active={active && discover}
            {labelFor}
            onOpen={(game) => openDetails(game)}
            onPrimary={(game) => primary(game)}
            onOpenSystems={() => onOpenSystems('')}
          />
        </div>
      {:else}
      <div class="body">
        <FoldingHeader testId="server-head" scroller={scrollEl} resetKey={activePlatform} bind:folded>
          {#snippet title()}
            <header data-testid="server-platform-header" class="platform-header">
              <h2>{activePlatformName}</h2>
              {#if !gamesLoading}
                <p data-testid="server-platform-counts" class="counts">
                  {platformCountsLine(activePlatformRow?.rom_count ?? 0, installedCount)}
                </p>
              {/if}
              {#if chipsShown}
                <div class="chips">
                  {#if firmwareLinks}
                    <button
                      data-testid="server-firmware-chip"
                      class="chip link"
                      title="Install firmware in Settings › Systems"
                      onclick={() => onOpenSystems(activePlatformName)}
                    >
                      {firmwareChipLabel(firmware)}
                    </button>
                  {:else}
                    <span data-testid="server-firmware-chip" class="chip">{firmwareChipLabel(firmware)}</span>
                  {/if}
                  <button data-testid="server-emulator-chip" class="chip link" onclick={() => onOpenSystems(activePlatformName)}>
                    {emulatorChipLabel(defaultEmulator)}
                  </button>
                </div>
              {/if}
            </header>
          {/snippet}
          {#snippet toolbar()}
            <div class="toolbar">
              <input
                data-testid="server-search"
                class="search"
                type="search"
                placeholder={serverSearchPlaceholder(folded, activePlatformName)}
                aria-label="Search this platform"
                bind:this={searchEl}
                bind:value={search}
                onkeydown={onSearchKey}
              />
              <label class="control">
                <span>Size</span>
                <select data-testid="server-size" value={uiSettings.cardSizeServer} onchange={onSizeChange}>
                  {#each CARD_SIZES as option (option)}
                    <option value={option}>{cardSizeLabel(option)}</option>
                  {/each}
                </select>
              </label>
            </div>
          {/snippet}
        </FoldingHeader>

        <div class="scroller" bind:this={scrollEl}>
        {#if actionError}
          <p data-testid="server-error" class="error panel" role="alert">{actionError}</p>
          {#if actionError === umuOfferFor}<UmuInstallOffer />{/if}
        {/if}

        {#if gamesLoading}
          <!-- The list on screen belongs to the previous platform; saying
               "no games" about the new one before it has answered would be a
               guess. -->
        {:else if visible.length === 0}
          <p data-testid="server-empty" class="empty panel">
            {search.trim() === ''
              ? 'This platform has no games'
              : `No games match “${search.trim()}”`}
          </p>
        {:else}
          <CardGrid
            bind:this={grid}
            gridId="server-grid"
            labels={uiSettings.cardLabels}
            display={uiSettings.cardDisplay}
            cover={(game) => coverPair(game.path_cover_small, game.path_cover_large)}
            size={uiSettings.cardSizeServer}
            items={visible}
            key={(game) => game.id}
          >
            {#snippet card(game, i, rect)}
              <GameCard
                testId={`game-card-${game.id}`}
                index={i}
                badgeId={game.id}
                title={game.name}
                platform={activePlatformName}
                cover={coverPair(game.path_cover_small, game.path_cover_large)}
                installed={isInstalled(game, activePlatformName)}
                updateLabel={updates.labelFor(game.id)}
                {cloudPlatforms}
                labels={uiSettings.cardLabels}
                {rect}
                showPlatform={false}
                focused={i === focusIndex && inputMode.directional}
                onOpen={() => {
                  // A click moves the selection, so keyboard navigation
                  // resumes from the clicked card and the focus dwell that
                  // re-arms when the popup closes targets the SAME game
                  // (user ruling 2026-09-05) instead of reverting the art.
                  focusIndex = i;
                  openDetails(game);
                }}
                onPrimary={() => primary(game)}
                onCloud={() => openDetails(game, 'save')}
                onHoverStart={() => hoverStart(game)}
                onHoverEnd={hover.end}
              />
            {/snippet}
          </CardGrid>
        {/if}
        </div>
      </div>
      {/if}
    </div>
  {/if}
</section>

{#if detailsGame}
  {#key detailsGame.id}
    <Details
      subject={fromSummary(detailsGame, labelFor(detailsGame), platformOf(detailsGame)?.slug ?? '')}
      initialCloudMode={detailsCloudMode}
      onClose={closeDetails}
      onLibraryPathUnset={() => { showLibraryBanner = true; }}
    />
  {/key}
{/if}

<style>
  .server {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .columns {
    display: flex;
    align-items: stretch;
    gap: var(--gutter);
    flex: 1 1 auto;
    min-height: 0;
  }

  /* The header floats over the top of the scroller (`FoldingHeader`); the
     scroller beside it scrolls, so the platform rail stays put — same
     reasoning as `Library.svelte`'s. */
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

  .platform-header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
  }

  .platform-header h2 {
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
    gap: 8px;
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

  .chip.link:hover {
    color: var(--text-h);
    border-color: var(--primary);
  }

  .chip button {
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    border: none;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
  }

  .chip button:disabled {
    opacity: 0.6;
    cursor: default;
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

  .browse-secondary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .banner-error {
    color: var(--danger);
    flex-basis: 100%;
  }

  .offline {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 40px 24px;
    color: var(--text);
    font-size: 14px;
  }

  .offline button {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-row);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
  }
</style>
