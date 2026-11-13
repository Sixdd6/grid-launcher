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
    countSpin = false,
    countWarn = false,
    sub = false,
    fold = null,
    holdsSel = false,
    xl = undefined,
    testId = undefined,
    onSelect = undefined,
    onToggle = undefined,
    onX = undefined,
    children,
  }: {
    k: string;
    selected?: boolean;
    /** A number, or `!` for a store row that needs attention (game stores). */
    count?: number | string | null;
    /** A spinner in place of the count while a store's owned list loads. */
    countSpin?: boolean;
    countWarn?: boolean;
    sub?: boolean;
    /** A folder header: whether it is open. */
    fold?: boolean | null;
    /** A closed folder header hiding the selected row: B and Left into the
     *  rail land here instead of selecting another row. */
    holdsSel?: boolean;
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
    data-holds-sel={holdsSel ? '' : undefined}
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
    {@render children()}{#if countSpin}<span class="n"><span class="sspin" aria-label="Loading"></span></span>{:else if count !== null}<span class="n" class:warn={countWarn}>{count}</span>{/if}
  </button>
{/if}
