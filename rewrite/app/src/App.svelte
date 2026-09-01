<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Connect from './lib/Connect.svelte';
  import Library from './lib/Library.svelte';
  import Downloads from './lib/Downloads.svelte';
  import { session, restore } from './lib/stores/session.svelte';
  import { init as initDownloads } from './lib/stores/downloads.svelte';

  let library = $state<ReturnType<typeof Library> | null>(null);
  let restored = false; // restore() must fire once on mount, not on every connected toggle

  $effect(() => {
    if (!restored) {
      restored = true;
      restore();
    }
    const un = listen<{ action: 'up' | 'down' | 'left' | 'right' | 'accept' | 'back' }>('nav', (e) => {
      library?.handleNav(e.payload.action);
    });
    const unDownloads = session.state?.connected ? initDownloads() : undefined;
    return () => {
      un.then((f) => f());
      unDownloads?.then((f) => f());
    };
  });
</script>

{#if session.state?.connected}
  <Library bind:this={library} />
  <Downloads />
{:else}
  <Connect />
{/if}
