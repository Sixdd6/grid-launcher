<script lang="ts">
  // A settings-style page (item 320): a centred panel up to 62rem with a
  // 1.9rem title and a muted line under it, scrolling as one list.
  import type { Snippet } from 'svelte';

  let { title, sub = undefined, scroll = 'page', children }: { title: string; sub?: string; scroll?: string; children: Snippet } = $props();
</script>

<div class="page" data-scroll={scroll}>
  <div class="page-in" data-nav="v">
    <h1>{title}</h1>
    {#if sub}<p class="sub">{sub}</p>{/if}
    {@render children()}
  </div>
</div>
