<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Connect from './lib/Connect.svelte';
  import Shell from './lib/Shell.svelte';
  import { session, restore } from './lib/stores/session.svelte';
  import { init as initDownloads } from './lib/stores/downloads.svelte';
  import { init as initSessions } from './lib/stores/sessions.svelte';
  import { initReplenishListener } from './lib/stores/installed.svelte';

  let shell = $state<ReturnType<typeof Shell> | null>(null);
  let restored = false; // restore() must fire once on mount, not on every phase change

  // Declared before the restore effect below, and reading no reactive state,
  // so it registers once and as early as possible: `restore_session` and
  // `connect` both spawn the replenish job on the Rust side, and a job that
  // finished before the listener existed would leave the Library showing the
  // stale, cover-less rows until the next refresh. Registering it in
  // Shell.svelte was too late — the shell only mounts once the session is
  // already restored.
  $effect(() => {
    const un = initReplenishListener();
    return () => {
      un.then((f) => f());
    };
  });

  $effect(() => {
    if (!restored) {
      restored = true;
      restore();
    }
    const un = listen<{ action: 'up' | 'down' | 'left' | 'right' | 'accept' | 'back' }>('nav', (e) => {
      shell?.handleNav(e.payload.action);
    });
    const unDownloads = session.phase === 'shell' ? initDownloads() : undefined;
    const unSessions = session.phase === 'shell' ? initSessions() : undefined;
    return () => {
      un.then((f) => f());
      unDownloads?.then((f) => f());
      unSessions?.then((f) => f());
    };
  });
</script>

{#if session.phase === 'shell'}
  <Shell bind:this={shell} />
{:else if session.phase === 'none'}
  <Connect />
{/if}
