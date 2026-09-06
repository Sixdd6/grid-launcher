<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type InstalledGame } from './api';
  import { installed, refresh as refreshInstalled } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { visibleLibraryGames } from './library';
  import { emptyText, entryForKey, matchesRail, railEntries, type RailKey } from './library/rail';
  import { librarySelection, selectRail } from './library/selection.svelte';
  import { LIBRARY_SORTS, normalizeSort, sortGames, sortLabel, titleContains, type LibrarySort } from './library/sort';
  import { cloudPlatformSet } from './cards/badges';
  import { CARD_SIZES, cardSizeLabel, type CardSize } from './cards/size';
  import { setCardSize, uiSettings } from './stores/uiSettings.svelte';
  import { fromInstalled, type DetailsSubject } from './details/subject';
  import type { CloudMode } from './details/cloud';
  import GameCard from './GameCard.svelte';
  import CardGrid from './CardGrid.svelte';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import Details from './Details.svelte';
  import { chordBlocked, chordContext, shouldFocusSearch } from './views/searchKeys';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { subjectFromInstalled } from './background';
  import { createHoverViewed } from './lastViewedHover';
  import { dropPendingWarms } from './backgroundPrefetch';
  import { attachScrollGate, createVisibleWarmer, scrollParent } from './visibleWarm';
  import { noteViewed } from './stores/lastViewed.svelte';
  import { inputMode, noteDirectional } from './stores/inputMode.svelte';

  let { active }: { active: boolean } = $props();

  let search = $state('');
  let sort = $state<LibrarySort>('title');
  let focusIndex = $state(0);
  let grid = $state<ReturnType<typeof CardGrid> | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let subject = $state<DetailsSubject | null>(null);
  let detailsCloudMode = $state<CloudMode>('overview');
  let launchError = $state<string | null>(null);
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
  let entries = $derived(railEntries(installed.list, updateRomIds, nowSeconds));
  let selected = $derived(entryForKey(entries, librarySelection.key));

  /** The rail key without its `platform:` prefix — the dash slug the
   *  §11 `library-rail-platform-<slug>` id already carries, so the count
   *  id beside it never grows a colon. */
  const slugOf = (key: RailKey): string =>
    key.startsWith('platform:') ? key.slice('platform:'.length) : key;

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
        // The three fixed entries come first; everything after them is a
        // platform, so the heading sits on the first of those.
        heading: i === 3 ? 'PLATFORMS' : undefined,
      }),
    ),
  );

  let rows = $derived(
    sortGames(
      visibleLibraryGames(installed.list).filter(
        (row) =>
          matchesRail(row, selected.key, updateRomIds, nowSeconds) && titleContains(row.title, search),
      ),
      sort,
    ),
  );

  // Which platforms have a default emulator, for the cards' cloud badge
  // (see `cloudPlatformSet`). One call per mount; the Emulators view is
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
    if (focusIndex > rows.length - 1) focusIndex = Math.max(0, rows.length - 1);
  });

  function openDetails(row: InstalledGame, mode: CloudMode = 'overview') {
    detailsCloudMode = mode;
    subject = fromInstalled(row);
    noteViewed(subjectFromInstalled(row));
  }

  function closeDetails() {
    subject = null;
    detailsCloudMode = 'overview';
  }

  async function play(row: InstalledGame) {
    if (row.rom_id === null) return;
    launchError = null;
    try {
      await api.launchGame(row.rom_id);
    } catch (err) {
      launchError = err instanceof Error ? err.message : String(err);
    }
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than 120ms.
  const hover = createHoverViewed();

  // Keyboard/gamepad selection feeds the background through the SAME 120ms
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
    const row = rows[index];
    return row === undefined ? null : subjectFromInstalled(row);
  });

  $effect(() => {
    // `rows` is read as a dependency, not for its value: a filter, a sort or
    // a refresh replaces the grid's children, and the new ones have never
    // been observed. `warmBackground` de-duplicates by URL, so re-observing
    // a card already warmed costs no request.
    void rows;
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

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (subject) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!subject) {
        const row = rows[focusIndex];
        if (row) openDetails(row);
      }
      return;
    }
    if (subject) return; // grid navigation is suspended while the overlay is open
    focusIndex = moveFocus(focusIndex, action, grid?.columns() ?? 1, rows.length);
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

  function onSortChange(e: Event) {
    sort = normalizeSort((e.currentTarget as HTMLSelectElement).value);
  }

  function onSizeChange(e: Event) {
    const size = (e.currentTarget as HTMLSelectElement).value as CardSize;
    setCardSize('library', size).catch(() => {
      // Applied for this session; a failed save is not worth an error line.
    });
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="library-section" class="library over-art">
  <RailPane
    entries={railRows}
    testId="library-rail"
    ariaLabel="Library filters"
    onSelect={selectRail}
  />

  <div class="body">
    <div class="toolbar">
      <input
        data-testid="library-search"
        class="search"
        type="search"
        placeholder="Search installed games"
        aria-label="Search installed games"
        bind:this={searchEl}
        bind:value={search}
        onkeydown={onSearchKey}
      />
      <label class="control">
        <span>Sort</span>
        <select data-testid="library-sort" value={sort} onchange={onSortChange}>
          {#each LIBRARY_SORTS as option (option)}
            <option value={option}>{sortLabel(option)}</option>
          {/each}
        </select>
      </label>
      <label class="control">
        <span>Size</span>
        <select data-testid="library-size" value={uiSettings.cardSizeLibrary} onchange={onSizeChange}>
          {#each CARD_SIZES as option (option)}
            <option value={option}>{cardSizeLabel(option)}</option>
          {/each}
        </select>
      </label>
    </div>

    {#if launchError}
      <p data-testid="library-launch-error" class="error" role="alert">{launchError}</p>
    {/if}

    {#if rows.length === 0}
      <p data-testid="library-empty" class="empty">
        {search.trim() === '' ? emptyText(selected) : `No games match “${search.trim()}”`}
      </p>
    {:else}
      <CardGrid bind:this={grid} gridId="library-grid" size={uiSettings.cardSizeLibrary}>
        {#each rows as row, i (row.rom_id ?? `x-${i}`)}
          <GameCard
            testId={`library-card-${row.rom_id ?? `x-${i}`}`}
            badgeId={row.rom_id ?? `x-${i}`}
            title={row.title}
            platform={row.platform}
            coverUrl={row.cover_small_path || null}
            installed={true}
            updateLabel={updates.labelFor(row.rom_id)}
            {cloudPlatforms}
            focused={i === focusIndex && inputMode.directional}
            onOpen={() => {
              // A click moves the selection, so keyboard navigation resumes
              // from the clicked card and the focus dwell that re-arms when
              // the popup closes targets the SAME game (user ruling
              // 2026-09-05) instead of reverting the art to index 0.
              focusIndex = i;
              openDetails(row);
            }}
            onPrimary={() => play(row)}
            onCloud={() => openDetails(row, 'save')}
            onHoverStart={() => hoverStart(row)}
            onHoverEnd={hover.end}
          />
        {/each}
      </CardGrid>
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
    height: 100%;
    min-height: 0;
  }

  /* The column scrolls, not the shell: the rail beside it is a sibling, so
     scrolling here leaves it in place. `scrollIntoView` on a focused card
     walks to this box for the same reason. */
  .body {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
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
</style>
