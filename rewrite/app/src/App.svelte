<script lang="ts">
  import Connect from './lib/Connect.svelte';
  import Library from './lib/Library.svelte';
  import { session, restore } from './lib/stores/session.svelte';
  $effect(() => {
    restore();
  });
</script>

{#if session.state?.connected}
  <Library />
{:else}
  <Connect />
{/if}
