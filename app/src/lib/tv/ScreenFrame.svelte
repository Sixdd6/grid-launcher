<script lang="ts">
  // One screen on the stage: names it for `useScreen` (screen.svelte.ts).
  import type { Snippet } from 'svelte';
  import { provideScreen } from './screen.svelte';

  let { screen, children }: { screen: string; children: Snippet } = $props();
  provideScreen(() => screen);
</script>

{@render children()}
