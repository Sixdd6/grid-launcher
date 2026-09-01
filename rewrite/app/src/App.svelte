<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Connect from './lib/Connect.svelte';
  import Library from './lib/Library.svelte';
  import { session, restore } from './lib/stores/session.svelte';

  let library = $state<ReturnType<typeof Library> | null>(null);

  $effect(() => {
    restore();
    const un = listen<{ action: 'up' | 'down' | 'left' | 'right' | 'accept' | 'back' }>('nav', (e) => {
      library?.handleNav(e.payload.action);
    });
    return () => { un.then((f) => f()); };
  });
</script>

{#if session.state?.connected}
  <Library bind:this={library} />
{:else}
  <Connect />
{/if}
