<script lang="ts">
  import Library from './Library.svelte';
  import Server from './Server.svelte';
  import Downloads from './Downloads.svelte';
  import Emulators from './Emulators.svelte';
  import { api } from './api';
  import { session, retry, disconnect } from './stores/session.svelte';
  import { appUpdate, dismiss } from './stores/appUpdate.svelte';
  import { refresh as refreshInstalled } from './stores/installed.svelte';
  import { chipLabel, initialSection, type Section } from './shell';
  import type { NavDirection } from './focus/grid';

  // Set once when the shell first mounts (R2): Server when the restored/just
  // -connected session is online, Library when it came up offline. Switching
  // sections afterward is a user action via the nav buttons below.
  let section = $state<Section>(initialSection(session.connected));

  let library = $state<ReturnType<typeof Library> | null>(null);
  let server = $state<ReturnType<typeof Server> | null>(null);
  let downloads = $state<ReturnType<typeof Downloads> | null>(null);
  let showEmulators = $state(false);

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (section === 'library') library?.handleNav(action);
    else server?.handleNav(action);
  }

  $effect(() => {
    // Independent of the connection: Library shows installed games with
    // cached covers offline, so the registry must load once on mount even
    // when the shell comes up unreachable (Server.svelte's own refresh only
    // runs inside its session.connected-gated effect).
    // The `images-replenished` listener is NOT registered here: the replenish
    // job is spawned during restore/connect, before this shell ever mounts.
    // App.svelte owns it.
    refreshInstalled();
  });
</script>

<div data-testid="shell-topbar" class="topbar">
  <div class="topbar-row">
    <nav class="sections">
      <button data-testid="nav-library" class:active={section === 'library'} onclick={() => (section = 'library')}>
        Library
      </button>
      <button data-testid="nav-server" class:active={section === 'server'} onclick={() => (section = 'server')}>
        Server
      </button>
      <button data-testid="nav-downloads" onclick={() => downloads?.toggle()}>Downloads</button>
      <button data-testid="nav-emulators" onclick={() => (showEmulators = true)}>Emulators</button>
    </nav>

    <div class="session">
      <span data-testid="session-chip" class="chip" title={session.lastError ?? undefined}>
        {chipLabel(session)}
      </span>
      {#if !session.connected}
        <button data-testid="session-retry" disabled={session.busy} onclick={() => retry()}>Retry</button>
      {/if}
      <button data-testid="session-disconnect" onclick={() => disconnect()}>Disconnect</button>
    </div>
  </div>

  {#if !session.connected && session.lastError}
    <p data-testid="session-error" class="error-line">{session.lastError}</p>
  {/if}
</div>

<!-- Self-update notice (doc 10). A strip under the bar rather than a modal:
     it announces, it never blocks. `dismiss()` is per-process, so it stays
     gone for the rest of the session. -->
{#if appUpdate.notice}
  <div data-testid="app-update-banner" class="update-banner" role="status">
    <span>GRID Launcher {appUpdate.notice.tag} is available</span>
    <button data-testid="app-update-open" onclick={() => api.openReleasePage(appUpdate.notice!.url).catch(() => {})}>Open release</button>
    <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
  </div>
{/if}

<div hidden={section !== 'library'}>
  <Library active={section === 'library'} bind:this={library} />
</div>
<div hidden={section !== 'server'}>
  <Server active={section === 'server'} bind:this={server} />
</div>

<Downloads bind:this={downloads} onOpenEmulators={() => (showEmulators = true)} />
{#if showEmulators}
  <Emulators onClose={() => (showEmulators = false)} />
{/if}

<style>
  .topbar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 24px;
    box-sizing: border-box;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 5;
  }

  .topbar-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
  }

  .sections {
    display: flex;
    gap: 6px;
  }

  .sections button {
    font: inherit;
    font-size: 13px;
    padding: 6px 14px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .sections button:hover {
    background: var(--border);
  }

  .sections button.active {
    background: var(--border);
    color: var(--text-h);
    border-color: var(--border);
  }

  .session {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .chip {
    font-size: 13px;
    color: var(--text-h);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .session button {
    font: inherit;
    font-size: 12px;
    padding: 5px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .session button:hover {
    background: var(--border);
  }

  .session button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .error-line {
    margin: 0;
    color: #e5484d;
    font-size: 11px;
  }

  .update-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 24px;
    box-sizing: border-box;
    background: var(--accent);
    color: #fff;
    font-size: 13px;
  }

  /* The message yields first, so the two buttons never wrap to a second
     line — the strip stays one row tall at any window width. */
  .update-banner span {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .update-banner button {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 4px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.55);
    background: transparent;
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .update-banner button:hover,
  .update-banner button:focus-visible {
    background: rgba(255, 255, 255, 0.18);
  }

  .update-banner button.secondary {
    border-color: transparent;
    opacity: 0.85;
  }

  .update-banner button.secondary:hover,
  .update-banner button.secondary:focus-visible {
    opacity: 1;
  }
</style>
