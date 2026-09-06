<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Connect from './lib/Connect.svelte';
  import Shell from './lib/Shell.svelte';
  import { session, restore } from './lib/stores/session.svelte';
  import { init as initDownloads } from './lib/stores/downloads.svelte';
  import { init as initSessions } from './lib/stores/sessions.svelte';
  import { init as initCompatTools } from './lib/stores/compatTools.svelte';
  import { init as initUpdates } from './lib/stores/updates.svelte';
  import { initAppUpdate } from './lib/stores/appUpdate.svelte';
  import { initReplenishListener } from './lib/stores/installed.svelte';
  import { initUiSettings } from './lib/stores/uiSettings.svelte';
  import { noteDirectional } from './lib/stores/inputMode.svelte';

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

  // The self-update notice can arrive before the shell mounts, same
  // reasoning as initReplenishListener above.
  $effect(() => {
    const un = initAppUpdate();
    return () => {
      un.then((f) => f());
    };
  });

  // The theme must be on `<html>` before the first paint the user sees, so
  // this registers alongside the other pre-shell effects rather than inside
  // Shell.svelte.
  $effect(() => {
    const un = initUiSettings();
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
      // The gamepad moves a selection, so it puts the app in directional
      // mode — the grid views' own key handlers do the same for the keyboard.
      noteDirectional('gamepad');
      shell?.handleNav(e.payload.action);
    });
    const unDownloads = session.phase === 'shell' ? initDownloads() : undefined;
    const unSessions = session.phase === 'shell' ? initSessions() : undefined;
    const unCompatTools = session.phase === 'shell' ? initCompatTools() : undefined;
    const unUpdates = session.phase === 'shell' ? initUpdates() : undefined;
    return () => {
      un.then((f) => f());
      unDownloads?.then((f) => f());
      unSessions?.then((f) => f());
      unCompatTools?.then((f) => f());
      unUpdates?.then((f) => f());
    };
  });
</script>

{#if session.phase === 'shell'}
  <Shell bind:this={shell} />
{:else if session.phase === 'none'}
  <Connect />
{/if}
