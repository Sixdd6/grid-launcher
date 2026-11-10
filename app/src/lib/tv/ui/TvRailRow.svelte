<script lang="ts">
  // One rail row. Focusing it selects it at once, without A (item 105: the
  // pane or grid beside it follows); A or Right moves into that pane (item
  // 85). The selected row has the `selected` fill and a primary bar (item
  // 106). A folder header (`fold`) opens and closes with A instead, with a
  // turning chevron; `sub` rows are indented under it.
  import type { Snippet } from 'svelte';
  import { tvf } from '../focus/actions';
  import { railOpen } from '../nav.svelte';
  import TvIcon from './TvIcon.svelte';

  let {
    k,
    selected = false,
    count = null,
    sub = false,
    fold = null,
    xl = undefined,
    testId = undefined,
    onSelect = undefined,
    onToggle = undefined,
    onX = undefined,
    children,
  }: {
    k: string;
    selected?: boolean;
    count?: number | null;
    sub?: boolean;
    /** A folder header: whether it is open. */
    fold?: boolean | null;
    xl?: string;
    testId?: string;
    /** The row took focus: select it. */
    onSelect?: () => void;
    /** A on a folder header. */
    onToggle?: () => void;
    onX?: () => void;
    children: Snippet;
  } = $props();
</script>

{#if fold !== null}
  <button
    class="ri fold"
    data-k={k}
    data-al={fold ? 'Close' : 'Open'}
    data-testid={testId}
    aria-expanded={fold}
    use:tvf={{ a: () => onToggle?.() }}
  >
    <span class="chev" class:open={fold}><TvIcon name="chevronRight" px={16} /></span>{@render children()}
  </button>
{:else}
  <button
    class="ri"
    class:sel={selected}
    class:sub
    data-k={k}
    data-al="Open"
    data-xl={xl}
    data-rail-open
    data-testid={testId}
    aria-current={selected ? 'true' : undefined}
    use:tvf={{ a: railOpen, x: onX, focus: () => onSelect?.() }}
  >
    {@render children()}{#if count !== null}<span class="n">{count}</span>{/if}
  </button>
{/if}
