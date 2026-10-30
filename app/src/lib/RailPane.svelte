<script lang="ts" module>
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
  };
</script>

<script lang="ts" generics="K extends string">
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
      <span class="rail-heading">{entry.heading}</span>
    {/if}
    <button
      data-testid={entry.testId}
      data-rail={entry.dataRail}
      class="rail-item"
      class:active={entry.selected}
      aria-current={entry.selected ? 'page' : undefined}
      onclick={() => onSelect(entry.key)}
    >
      <span class="rail-label">{entry.label}</span>
      {#if entry.count !== undefined}
        <span data-testid={entry.countTestId} class="rail-count">{entry.count}</span>
      {/if}
    </button>
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

  .rail-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rail-count {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
