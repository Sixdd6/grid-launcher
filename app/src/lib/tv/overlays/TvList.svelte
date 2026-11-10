<script lang="ts">
  // A list (items 368–372; tvmode.js `openSheet`): a centred sheet, 36rem
  // wide and at most 80% of the screen tall, scrolling inside, with its
  // title, an optional line, group headings (not focusable) and the options.
  // A choice list checks its current value and opens focused on it.
  import { tvf } from '../focus/actions';
  import { closeLayer } from '../nav.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import type { ListSpec } from './overlays.svelte';

  let { spec }: { spec: ListSpec } = $props();

  let choice = $derived(spec.items.some((it) => 'label' in it && it.checked !== undefined));

  function pick(fn: (() => void) | undefined) {
    void closeLayer().then(() => fn?.());
  }
</script>

<div class="sheet" data-nav="v" data-scroll="sheet" data-testid="tv-list" role="dialog" aria-label={spec.title}>
  <h2>{spec.title}</h2>
  {#if spec.sub}<p class="sub">{spec.sub}</p>{/if}
  {#each spec.items as it, i (i)}
    {#if 'group' in it}
      <div class="sgroup">{it.group}</div>
    {:else}
      <button
        class="opt"
        class:dng={it.danger}
        data-k={`o${i}`}
        data-autofocus={it.checked ? '' : undefined}
        data-testid={it.testId ?? `tv-list-opt-${i}`}
        aria-checked={choice ? !!it.checked : undefined}
        role={choice ? 'menuitemradio' : 'menuitem'}
        use:tvf={{ a: () => pick(it.onPick) }}
      >
        {#if choice}<span class="ck">{#if it.checked}<TvIcon name="check" px={20} />{/if}</span>{/if}{it.label}{#if it.sub}<span class="s">{it.sub}</span>{/if}
      </button>
    {/if}
  {/each}
</div>
