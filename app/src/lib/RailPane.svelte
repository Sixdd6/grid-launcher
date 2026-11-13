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
    /** The store heading's badge: `!` (needs attention) or `off` (not connected). */
    headingFlag?: { text: string; title: string } | null;
    /** A store group's rows sit indented under their heading. */
    indent?: boolean;
    /** The count is a spinner (`loading`) or a `!` (`attention`) instead of a number. */
    countKind?: 'number' | 'loading' | 'attention';
  };
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
  }: {
    entries: RailPaneEntry<K>[];
    testId: string;
    ariaLabel: string;
    onSelect: (key: K) => void;
  } = $props();
</script>

<nav data-testid={testId} class="rail" aria-label={ariaLabel}>
  {#each entries as entry (entry.key)}
    {#if entry.heading}
      <span class="rail-heading" class:store-head={entry.headingMark !== undefined}>
        {#if entry.headingMark}
          <span class="rail-mark" style:--st-hue={entry.headingMark.hue} aria-hidden="true">{entry.headingMark.mono}</span>
        {/if}
        {entry.heading}
        {#if entry.headingFlag}
          <span
            class="rail-flag"
            class:off={entry.headingFlag.text === 'off'}
            title={entry.headingFlag.title}
            data-testid={`${entry.testId}-flag`}>{entry.headingFlag.text}</span
          >
        {/if}
      </span>
    {/if}
    <button
      data-testid={entry.testId}
      data-rail={entry.dataRail}
      class="rail-item"
      class:indent={entry.indent}
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

  .rail-label {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .rail-item.indent {
    padding-left: 26px;
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
    .rail-spin {
      animation: none;
      border-top-color: var(--line-strong);
    }
  }
</style>
