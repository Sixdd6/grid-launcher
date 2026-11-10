<script lang="ts">
  // The app's icon at a rem size, so it scales with TV mode's root font
  // (item 2: every measure in rem). `px` is the prototype's size at its 20px
  // root (22 → 1.1rem).
  import Icon from '../../Icon.svelte';
  import type { IconName } from '../../icons';

  let { name, px = 16 }: { name: IconName; px?: number } = $props();
</script>

<span class="tv-ic" style={`--s:${px / 20}rem`}><Icon {name} size={px} /></span>

<style>
  .tv-ic {
    display: inline-flex;
    flex: 0 0 auto;
  }
  .tv-ic :global(svg) {
    width: var(--s);
    height: var(--s);
  }
</style>
