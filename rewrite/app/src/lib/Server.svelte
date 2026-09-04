<script lang="ts">
  import { api, FIRMWARE_PASS_FINISHED_EVENT, type FirmwarePassFinished, type GameSummary, type Platform } from './api';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Details from './Details.svelte';
  import GameCard from './GameCard.svelte';
  import CardGrid from './CardGrid.svelte';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import { fromSummary } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { isInstalled, refresh as refreshInstalled } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { hostOf } from './shell';
  import { session, retry } from './stores/session.svelte';
  import { createHoverViewed } from './lastViewedHover';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { titleContains } from './library/sort';
  import {
    emulatorChipLabel,
    firmwareChipLabel,
    firmwareInstallable,
    platformCountsLine,
    type FirmwareChipState,
  } from './server/header';
  import {
    firmwareInstallLabel,
    firmwarePassFinished,
    firmwareRequested,
    firmwareStatusSettled,
    NO_FIRMWARE_REQUEST,
    type FirmwareRequest,
  } from './server/firmware';
  import { savedDefaultFor } from './emulators/defaults';
  import { chordBlocked, chordContext, shouldFocusSearch } from './views/searchKeys';

  let {
    active,
    onOpenEmulators = () => {},
  }: { active: boolean; onOpenEmulators?: () => void } = $props();

  let platforms = $state<Platform[]>([]);
  let games = $state<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  let search = $state('');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let detailsGame = $state<GameSummary | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  // Every inline failure the grid can produce: a refused install AND a
  // refused launch, both from `primary()`. Shown in `server-error`.
  let actionError = $state<string | null>(null);
  let cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());
  // Platform name -> default emulator name, for the header's emulator chip.
  let defaultEmulators = $state<Record<string, string>>({});
  let firmware = $state<FirmwareChipState>(null);
  let firmwareRequest = $state<FirmwareRequest>(NO_FIRMWARE_REQUEST);
  // True only while THIS platform's Install is unanswered: a request left
  // over from another platform must never disable this one's button.
  let firmwarePending = $derived(
    firmwareRequest.pending && firmwareRequest.platformId === activePlatform,
  );
  // The game list belongs to the previous platform until `listGames`
  // answers. The counts line and the empty message would both state
  // something false about the new platform in that gap, so neither renders.
  let gamesLoading = $state(false);
  // The header's own error line: a refused Install. Never a path or a token —
  // the backend's command errors carry neither, and nothing is appended.
  let headerError = $state<string | null>(null);

  let libraryPathInput = $state('');
  let showLibraryBanner = $state(false);
  let libraryPathSaving = $state(false);
  let libraryPathError = $state<string | null>(null);

  let activePlatformRow = $derived(platforms.find((p) => p.id === activePlatform) ?? null);
  let activePlatformName = $derived(activePlatformRow?.name ?? '');
  let visible = $derived(games.filter((game) => titleContains(game.name, search)));
  let installedCount = $derived(
    games.filter((game) => isInstalled(game, activePlatformName)).length,
  );
  let defaultEmulator = $derived(savedDefaultFor(defaultEmulators, activePlatformName));

  // Design §5: the Server rail is the shared `RailPane`. The row keeps the
  // `platform-btn-<id>` id the specs already use and carries the §11
  // `data-rail` value beside it.
  let railRows = $derived(
    platforms.map(
      (p): RailPaneEntry<string> => ({
        key: String(p.id),
        testId: `platform-btn-${p.id}`,
        countTestId: `server-rail-count-${p.id}`,
        dataRail: `server-rail-${p.id}`,
        label: p.name,
        count: p.rom_count,
        selected: p.id === activePlatform,
      }),
    ),
  );

  $effect(() => {
    if (!session.connected) return; // re-runs on reconnect: session.connected is read above
    api.listPlatforms().then((p) => {
      platforms = p;
      if (p.length && activePlatform === null) selectPlatform(p[0].id);
    });
    refreshInstalled();
    checkLibraryPath();
    loadLaunchDefaults();
  });

  // The emulator chip sends the user to the Emulators view to change the very
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
    if (id === null || name === '') return;
    void session.connected; // re-runs on reconnect
    firmware = null;
    // A new platform is a new question: the previous platform's install
    // button must not still read "Requested…", nor its error still stand.
    firmwareRequest = NO_FIRMWARE_REQUEST;
    headerError = null;
    refreshFirmwareStatus(id, name);
  });

  /** One status read, with the staleness guard both callers need. Ends any
   *  Install request for `id`: the chip is showing a fresh answer, so the
   *  button has nothing left to wait for. */
  function refreshFirmwareStatus(id: number, name: string) {
    api
      .platformFirmwareStatus(id, name)
      .then((status) => {
        if (activePlatform === id) firmware = status;
        firmwareRequest = firmwareStatusSettled(firmwareRequest, id);
      })
      .catch(() => {
        // Unreachable or refused. Say so: "checking…" forever would claim a
        // call is still in flight when none is, and reporting "no firmware"
        // would claim an answer we never got.
        if (activePlatform === id) firmware = 'unavailable';
        firmwareRequest = firmwareStatusSettled(firmwareRequest, id);
      });
  }

  // The background pass announces its own end (it is fire-and-forget, so the
  // command's return says nothing about it). Listener first, at mount, so no
  // event can land between an Install and the subscription.
  $effect(() => {
    let unlisten: UnlistenFn | null = null;
    let stopped = false;
    listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (e) => {
      const next = firmwarePassFinished(firmwareRequest, e.payload, activePlatform);
      firmwareRequest = next.state;
      if (next.refetch && activePlatform !== null && activePlatformName !== '') {
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

  function installFirmware() {
    const id = activePlatform;
    if (id === null) return;
    firmwareRequest = firmwareRequested(id);
    headerError = null;
    api.installFirmwareForPlatform(id, activePlatformName).catch((e) => {
      // Same staleness guard as the status fetch: a rejection that arrives
      // after the user moved on belongs to the platform it was asked about,
      // not to whichever one is on screen now.
      firmwareRequest = firmwareStatusSettled(firmwareRequest, id);
      if (activePlatform !== id) return;
      // The command's own message, verbatim. grid-core's command errors name
      // no path and carry no token, and nothing is appended to them here.
      headerError = String(e);
    });
  }

  async function checkLibraryPath() {
    try {
      const path = await api.getLibraryPath();
      showLibraryBanner = path.trim() === '';
    } catch {
      // Unreadable config: leave the banner as-is rather than nag with a
      // guess — install errors will surface the real problem if there is one.
    }
  }

  async function selectPlatform(id: number) {
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
    noteViewed(game.path_cover_large);
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
      if (isInstalled(game, activePlatformName)) await api.launchGame(game.id);
      else await api.installGame(game.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      actionError = message;
      if (message.includes('library folder')) showLibraryBanner = true;
    }
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than half a second.
  const hover = createHoverViewed();

  /** Design §3: `Ctrl+F` focuses the current view's search box. */
  export function focusSearch() {
    searchEl?.focus();
    searchEl?.select();
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (detailsGame) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!detailsGame) {
        const game = visible[focusIndex];
        if (game) openDetails(game);
      }
      return;
    }
    if (detailsGame) return; // grid navigation is suspended while the overlay is open
    focusIndex = moveFocus(focusIndex, action, grid?.columns() ?? 1, visible.length);
    grid?.element()?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    // One read of the document, shared by both branches below.
    const ctx = chordContext(document);
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
      handleNav(action);
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
    <div data-testid="library-path-banner" class="library-banner">
      <span>Set a library folder to install games.</span>
      <input
        data-testid="library-path-input"
        bind:value={libraryPathInput}
        placeholder="/path/to/library"
        disabled={libraryPathSaving}
      />
      <button data-testid="library-path-save" disabled={libraryPathSaving || !libraryPathInput.trim()} onclick={saveLibraryPath}>
        {libraryPathSaving ? 'Saving…' : 'Save'}
      </button>
      {#if libraryPathError}<span class="banner-error" role="alert">{libraryPathError}</span>{/if}
    </div>
  {/if}

  {#if !session.connected}
    <div data-testid="server-offline" class="offline">
      Not connected to {hostOf(session.serverUrl)}
      <button data-testid="server-retry" onclick={() => retry()}>Retry</button>
    </div>
  {:else}
    <div class="columns">
      <RailPane
        entries={railRows}
        testId="server-rail"
        ariaLabel="Server platforms"
        onSelect={(key) => selectPlatform(Number(key))}
      />

      <div class="body">
        <header data-testid="server-platform-header" class="platform-header">
          <h2>{activePlatformName}</h2>
          {#if !gamesLoading}
            <p data-testid="server-platform-counts" class="counts">
              {platformCountsLine(activePlatformRow?.rom_count ?? 0, installedCount)}
            </p>
          {/if}
          <div class="chips">
            <span data-testid="server-firmware-chip" class="chip">
              {firmwareChipLabel(firmware)}
              {#if firmwareInstallable(firmware)}
                <button
                  data-testid="server-firmware-install"
                  onclick={installFirmware}
                  disabled={firmwarePending}
                >
                  {firmwareInstallLabel(firmwarePending)}
                </button>
              {/if}
            </span>
            <button data-testid="server-emulator-chip" class="chip link" onclick={onOpenEmulators}>
              {emulatorChipLabel(defaultEmulator)}
            </button>
          </div>
          {#if headerError !== null}
            <p data-testid="server-header-error" class="header-error">{headerError}</p>
          {/if}
        </header>

        <div class="toolbar">
          <input
            data-testid="server-search"
            class="search"
            type="search"
            placeholder="Search this platform"
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

        {#if actionError}
          <p data-testid="server-error" class="error" role="alert">{actionError}</p>
        {/if}

        {#if gamesLoading}
          <!-- The list on screen belongs to the previous platform; saying
               "no games" about the new one before it has answered would be a
               guess. -->
        {:else if visible.length === 0}
          <p data-testid="server-empty" class="empty">
            {search.trim() === ''
              ? 'This platform has no games'
              : `No games match “${search.trim()}”`}
          </p>
        {:else}
          <CardGrid bind:this={grid} gridId="server-grid" size={uiSettings.cardSizeServer}>
            {#each visible as game, i (game.id)}
              <GameCard
                testId={`game-card-${game.id}`}
                badgeId={game.id}
                title={game.name}
                platform={activePlatformName}
                coverUrl={game.path_cover_small}
                installed={isInstalled(game, activePlatformName)}
                updateLabel={updates.labelFor(game.id)}
                {cloudPlatforms}
                focused={i === focusIndex}
                onOpen={() => openDetails(game)}
                onPrimary={() => primary(game)}
                onCloud={() => openDetails(game, 'save')}
                onHoverStart={() => hover.start(game.path_cover_large)}
                onHoverEnd={hover.end}
              />
            {/each}
          </CardGrid>
        {/if}
      </div>
    </div>
  {/if}

  {#if detailsGame}
    {#key detailsGame.id}
      <Details
        subject={fromSummary(detailsGame, activePlatformName)}
        initialCloudMode={detailsCloudMode}
        onClose={closeDetails}
        onLibraryPathUnset={() => { showLibraryBanner = true; }}
      />
    {/key}
  {/if}
</section>

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
    flex: 1 1 auto;
    min-height: 0;
  }

  .body {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .platform-header {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px 14px;
    padding: 16px 24px 0;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
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

  .header-error {
    flex-basis: 100%;
    margin: 0;
    font-size: 12px;
    color: var(--danger);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font: inherit;
    font-size: 11px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface);
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
    color: #fff;
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
    padding: 12px 24px 0;
    width: 100%;
    max-width: 1920px;
    margin: 0 auto;
    box-sizing: border-box;
  }

  .search {
    flex: 1 1 240px;
    min-width: 120px;
    font: inherit;
    padding: 6px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
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
    padding: 5px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .empty {
    padding: 40px 24px;
    color: var(--text-muted);
    font-size: 14px;
  }

  .error {
    margin: 8px 24px 0;
    color: var(--danger);
    font-size: 12px;
  }

  .library-banner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 10px 24px;
    background: var(--surface);
    color: var(--text-h);
    font-size: 13px;
  }

  .library-banner input {
    flex: 1 1 240px;
    min-width: 160px;
    font: inherit;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .library-banner button {
    font: inherit;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .library-banner button:disabled {
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
    color: #fff;
    cursor: pointer;
  }
</style>
