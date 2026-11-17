<script lang="ts" module>
  import type { IconName } from './icons';
  /** One row of a view's left rail. The caller owns the labels, counts and
   *  test ids; this component owns the markup and the 220px column. */
  export type RailPaneEntry<K extends string = string> = {
    key: K;
    /** `data-testid` for the row's button. */
    testId: string;
    /** `data-testid` for the count badge. Only read when `count` is set. */
    countTestId?: string;
    label: string;
    /** The count badge. Omit it (the Settings rail does) and no badge renders. */
    count?: number;
    selected: boolean;
    /** A section heading rendered above this row when set (e.g. "PLATFORMS"). */
    heading?: string;
    /** A `data-rail` attribute value for the row, when the view wants one. */
    dataRail?: string;
    /** A divider under this row (Server's Discover row). */
    divider?: boolean;
    /** An icon before the label (Library's backup drive rows). */
    icon?: IconName;
    /** The row's tooltip (a drive row's volume label). */
    title?: string;
    /** A store group's heading (GameStores README): its monogram tile before the heading text. */
    headingMark?: { mono: string; hue: string };
    /** The store heading's badge: `!` (needs attention), `off` (not connected) or `busy` (a spinner: RomM connecting). */
    headingFlag?: { text: string; title: string } | null;
    /** A store group's rows sit indented under their heading. */
    indent?: boolean;
    /** The count is a spinner (`loading`) or a `!` (`attention`) instead of a number. */
    countKind?: 'number' | 'loading' | 'attention';
  };

  /** A folding group of the Library rail (RommStorefront README § Library
   *  rail › Folding groups): an inset panel whose heading button opens and
   *  closes it. The caller owns whether it is closed (`[ui] rail_closed`). */
  export type RailPaneGroup<K extends string = string> = {
    /** The group's id (`drives`, `gog`, `romm`…), handed back to `onToggle`. */
    group: string;
    /** Sentence case: "Backup drives", "GOG". */
    name: string;
    /** `data-testid` for the heading button. */
    testId: string;
    /** The store's monogram tile; none for Backup drives. */
    mark?: { mono: string; hue: string };
    /** `!`, `off` or `busy` (a spinner), shown open or closed. */
    flag?: { text: string; title: string; testId: string } | null;
    closed: boolean;
    /** Closed only: the Installed number or the number of drives. */
    count: { n: number; label: string } | null;
    /** Closed and holding the selected row: the heading's tooltip. */
    holds: string | null;
    /** The heading's accessible description while it holds the selection. */
    holdsDescription?: string;
    rows: RailPaneEntry<K>[];
  };

  export type RailPaneItem<K extends string = string> = RailPaneEntry<K> | RailPaneGroup<K>;
</script>

<script lang="ts" generics="K extends string">
  import Icon from './Icon.svelte';
  // Design §5: the Library, Server, Emulators and Settings views share one
  // rail — 220px, a list of labelled counts, optional section headings.
  // Only the entries differ, so the markup and the CSS live here once.
  let {
    entries,
    testId,
    ariaLabel,
    onSelect,
    onToggle = undefined,
  }: {
    entries: RailPaneItem<K>[];
    testId: string;
    ariaLabel: string;
    onSelect: (key: K) => void;
    /** A group heading clicked, or Enter / Space on it. */
    onToggle?: (group: string) => void;
  } = $props();

  const isGroup = (item: RailPaneItem<K>): item is RailPaneGroup<K> => 'rows' in item;
  const itemKey = (item: RailPaneItem<K>): string => (isGroup(item) ? `group:${item.group}` : item.key);
</script>

{#snippet flagOf(flag: { text: string; title: string; testId: string })}
  {#if flag.text === 'busy'}
    <!-- RommStorefront prototype `.rm-flag-spin`: a spinner while RomM connects. -->
    <span class="rail-flag busy" title={flag.title} aria-label={flag.title} data-testid={flag.testId}
      ><span class="rail-spin small"></span></span
    >
  {:else}
    <span class="rail-flag" class:off={flag.text === 'off'} title={flag.title} data-testid={flag.testId}>{flag.text}</span>
  {/if}
{/snippet}

{#snippet row(entry: RailPaneEntry<K>)}
    {#if entry.heading}
      <span class="rail-heading" class:store-head={entry.headingMark !== undefined}>
        {#if entry.headingMark}
          <span class="rail-mark" style:--st-hue={entry.headingMark.hue} aria-hidden="true" data-mono={entry.headingMark.mono}></span>
        {/if}
        {entry.heading}
        {#if entry.headingFlag}{@render flagOf({ ...entry.headingFlag, testId: `${entry.testId}-flag` })}{/if}
      </span>
    {/if}
    <button
      data-testid={entry.testId}
      data-rail={entry.dataRail}
      class="rail-item"
      class:indent={entry.indent}
      class:has-icon={entry.icon !== undefined}
      class:active={entry.selected}
      aria-current={entry.selected ? 'page' : undefined}
      title={entry.title}
      onclick={() => onSelect(entry.key)}
    >
      {#if entry.icon}
        <span class="rail-icon" aria-hidden="true"><Icon name={entry.icon} size={14} /></span>
      {/if}
      <span class="rail-label">{entry.label}</span>
      {#if entry.countKind === 'loading'}
        <span data-testid={entry.countTestId} class="rail-count" aria-label="Loading"><span class="rail-spin"></span></span>
      {:else if entry.countKind === 'attention'}
        <span data-testid={entry.countTestId} class="rail-count warn" aria-label="Needs attention">!</span>
      {:else if entry.count !== undefined}
        <span data-testid={entry.countTestId} class="rail-count">{entry.count}</span>
      {/if}
    </button>
    {#if entry.divider}
      <hr class="rail-divider" />
    {/if}
{/snippet}

<nav data-testid={testId} class="rail" aria-label={ariaLabel}>
  {#each entries as item (itemKey(item))}
    {#if isGroup(item)}
      {@const rowsId = `${testId}-rows-${item.group}`}
      <section class="rail-group" class:closed={item.closed} class:holds={item.holds !== null} aria-label={item.name} data-rail-group={item.group}>
        <button
          type="button"
          class="group-head"
          data-testid={item.testId}
          aria-expanded={!item.closed}
          aria-controls={rowsId}
          title={item.holds ?? undefined}
          aria-describedby={item.holds !== null && item.holdsDescription ? `${rowsId}-holds` : undefined}
          onclick={() => onToggle?.(item.group)}
          onkeydown={(e) => {
            // Enter or Space toggles, once: handled here (the button's own
            // activation is cancelled) so it never depends on how the
            // webview turns a key into a click.
            if (e.key !== 'Enter' && e.key !== ' ') return;
            e.preventDefault();
            if (!e.repeat) onToggle?.(item.group);
          }}
        >
          <span class="group-chev" aria-hidden="true"><Icon name="chevronRight" size={12} /></span>
          {#if item.mark}
            <span class="rail-mark" style:--st-hue={item.mark.hue} aria-hidden="true" data-mono={item.mark.mono}></span>
          {/if}
          <span class="group-name">{item.name}</span>
          <span class="group-sp"></span>
          {#if item.flag}{@render flagOf(item.flag)}{/if}
          {#if item.closed && item.count}
            <span class="group-n" title={item.count.label} aria-label={item.count.label} data-testid={`${item.testId}-count`}>{item.count.n}</span>
          {/if}
        </button>
        {#if item.holds !== null && item.holdsDescription}
          <span class="sr-only" id={`${rowsId}-holds`}>{item.holdsDescription}</span>
        {/if}
        <div class="group-rows" id={rowsId} role="group" aria-label={item.name} hidden={item.closed}>
          {#if !item.closed}
            {#each item.rows as entry (entry.key)}
              {@render row(entry)}
            {/each}
          {/if}
        </div>
      </section>
    {:else}
      {@render row(item)}
    {/if}
  {/each}
</nav>

<style>
  /* Design §5: the rail is 220px. A floating panel (look-and-feel plan
     2026-10-30), separated from the view by the shell's gutter, so it reads
     as its own group and its labels never sit on the art. */
  .rail {
    flex: 0 0 220px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
    box-sizing: border-box;
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    overflow-y: auto;
    /* The scrollbar gets a lane of its own instead of overlaying the rows
       (WebKitGTK draws GTK overlay scrollbars unless one is styled), and the
       lane is kept while nothing scrolls, so rows never shift when a group
       opens. The padding's right side gives way to the lane. */
    padding-right: 2px;
    scrollbar-gutter: stable;
  }

  .rail::-webkit-scrollbar {
    width: 8px;
  }

  .rail::-webkit-scrollbar-track {
    background: transparent;
  }

  .rail::-webkit-scrollbar-thumb {
    background: var(--line);
    border-radius: 4px;
    border: 2px solid transparent;
    background-clip: padding-box;
  }

  .rail::-webkit-scrollbar-thumb:hover {
    background-color: var(--text-muted);
  }

  /* Labels start 16px in (the rail's 8px + 8px here): the same left edge as
     the top bar's brandmark above, which sits at the bar's 16px inset. */
  .rail-heading {
    margin: 12px 8px 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .rail-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 8px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rail-item:hover {
    background: var(--field);
    color: var(--text-h);
  }

  /* The one selected style for rows: a primary tint and an inset bar. */
  .rail-item.active {
    background: var(--selected);
    box-shadow: inset 3px 0 0 var(--primary);
    color: var(--text-h);
    font-weight: 600;
  }

  /* GameBackups prototype `.bk-rail-icon`: muted, the row's colour when selected. */
  .rail-icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    color: var(--text-muted);
  }

  .rail-item.active .rail-icon {
    color: inherit;
  }

  /* A long name (a RomM platform such as "PC Fangames & Ports") wraps onto
     more lines, as in TV mode, instead of being cut off. */
  .rail-label {
    flex: 1 1 auto;
    min-width: 0;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }

  .rail-divider {
    margin: 6px 8px;
    border: none;
    border-top: 1px solid var(--line);
  }

  .rail-count {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
  }

  /* GameStores prototype `.st-head`, `.st-flag`, `.st-row`, `.st-spin`. */
  .rail-heading.store-head {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  /* The letter is drawn, not text: a label reads as the store's name alone. */
  .rail-mark::before {
    content: attr(data-mono);
  }

  .rail-mark {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 4px;
    background: var(--st-hue, var(--field));
    color: #fff;
    font: 700 9px/1 system-ui, sans-serif;
    letter-spacing: 0;
  }

  .rail-flag {
    margin-left: auto;
    display: inline-grid;
    place-items: center;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    box-sizing: border-box;
    border-radius: var(--r-pill);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0;
    background: var(--warning);
    color: #1a1a12;
  }

  .rail-flag.off {
    background: var(--field);
    color: var(--text-muted);
    border: 1px solid var(--line-strong);
  }

  .rail-flag.busy {
    background: transparent;
    border: 0;
    padding: 0;
    min-width: 0;
  }

  .rail-spin.small {
    width: 8px;
    height: 8px;
  }

  .rail-item.indent {
    padding-left: 26px;
  }

  /* Library rail: folding groups (RommStorefront README § Folding groups,
     prototype railfold.css). Each group is an inset panel inside the rail
     (the `surface` fill, `r-row` corners + 2), 8px under Updates and 6px
     apart, so one storefront never runs into the next. */
  .rail-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 8px;
    padding: 2px;
    border-radius: calc(var(--r-row) + 2px);
    background: var(--surface);
  }

  .rail-group + .rail-group {
    margin-top: 6px;
  }

  /* The holds-the-selection description: read, not shown. */
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
  }

  .group-rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .group-rows[hidden] {
    display: none;
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 30px;
    box-sizing: border-box;
    padding: 5px 8px 5px 4px;
    border: 0;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-h);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  .group-head:hover {
    background: var(--field);
  }

  .group-head:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -2px;
  }

  .group-chev {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--text-muted);
    transition: transform 150ms ease;
  }

  .rail-group:not(.closed) .group-chev {
    transform: rotate(90deg);
  }

  .group-head .rail-mark {
    width: 16px;
    height: 16px;
    font-size: 10px;
  }

  .group-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-sp {
    flex: 1 1 auto;
  }

  .group-head .rail-flag {
    margin-left: 0;
  }

  .group-n {
    flex: none;
    margin-left: 6px;
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted);
  }

  /* A closed group holding the selected row: the selected row's fill and bar. */
  .rail-group.holds .group-head {
    background: var(--selected);
    box-shadow: inset 3px 0 0 var(--primary);
  }

  /* Rows start under the mark (4 + chevron 14 + gap 6 = 24px); a drive
     row's own icon starts at 20px so its name keeps room in the 220px rail. */
  .group-rows .rail-item {
    padding-left: 24px;
  }

  .group-rows .rail-item.has-icon {
    padding-left: 20px;
  }

  .rail-count.warn {
    color: var(--warning-text);
    font-weight: 700;
  }

  .rail-spin {
    display: inline-block;
    width: 10px;
    height: 10px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: rail-spin 800ms linear infinite;
    vertical-align: -1px;
  }

  @keyframes rail-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .group-chev {
      transition: none;
    }

    .rail-spin {
      animation: none;
      border-top-color: var(--line-strong);
    }
  }
</style>
