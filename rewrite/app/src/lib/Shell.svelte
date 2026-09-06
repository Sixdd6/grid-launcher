<script lang="ts">
  import Library from './Library.svelte';
  import Server from './Server.svelte';
  import Downloads from './Downloads.svelte';
  import DownloadsFooter from './DownloadsFooter.svelte';
  import Emulators from './Emulators.svelte';
  import BackgroundArt from './BackgroundArt.svelte';
  import Icon from './Icon.svelte';
  import Settings from './Settings.svelte';
  import Toast from './Toast.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { api, CLOUD_UPLOAD_FINISHED_EVENT, type CloudUploadFinished } from './api';
  import { session, retry, disconnect } from './stores/session.svelte';
  import { appUpdate } from './stores/appUpdate.svelte';
  import { installed, refresh as refreshInstalled } from './stores/installed.svelte';
  import { seedLastViewed } from './stores/lastViewed.svelte';
  import { pushToast } from './stores/toasts.svelte';
  import { chipLabel, hostOf, initialView, VIEWS, viewForDigit, viewLabel, type View } from './shell';
  import type { NavDirection } from './focus/grid';
  // The same guard the grid views use for Ctrl+F: an accelerator must stay
  // out of the way while focus sits in a text-entry control (where
  // Ctrl+<n> can be an editor chord) or a modal dialog owns the screen and
  // switching the view behind it would strand it.
  import { chordBlocked, chordContext } from './views/searchKeys';

  // Set once when the shell first mounts (R2): Server when the restored/just
  // -connected session is online, Library when it came up offline. Switching
  // views afterward is a user action — a pill, Ctrl+1..5, or `show()`.
  let view = $state<View>(initialView(session.connected));

  let library = $state<ReturnType<typeof Library> | null>(null);
  let server = $state<ReturnType<typeof Server> | null>(null);
  let settings = $state<ReturnType<typeof Settings> | null>(null);
  let emulators = $state<ReturnType<typeof Emulators> | null>(null);
  let serverMenuOpen = $state(false);
  let sessionEl = $state<HTMLElement | null>(null);

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (view === 'library') library?.handleNav(action);
    else if (view === 'server') server?.handleNav(action);
  }

  /** Programmatic navigation, for the footer strip and the update badge. */
  export function show(next: View) {
    view = next;
  }

  // Ctrl+1..5 (design §3). Alt/Shift are excluded so this never steals a
  // window-manager or text-editing chord; Meta is accepted alongside Ctrl so
  // the same accelerator works on macOS.
  function onKeydown(e: KeyboardEvent) {
    // Escape dismisses the server menu. Nothing is preventDefault-ed: other
    // Escape handlers (the details panel) still see the key.
    if (e.key === 'Escape' && serverMenuOpen) {
      serverMenuOpen = false;
      return;
    }
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    const next = viewForDigit(e.key);
    if (next === null || chordBlocked(chordContext(document))) return;
    e.preventDefault();
    view = next;
  }

  /** A click anywhere outside the session cluster closes the server menu. */
  function onWindowPointerDown(e: MouseEvent) {
    if (!serverMenuOpen) return;
    const target = e.target as Node | null;
    if (target !== null && sessionEl?.contains(target)) return;
    serverMenuOpen = false;
  }

  function openServer() {
    serverMenuOpen = false;
    api.openServerPage().catch(() => {
      // The opener refuses a URL it cannot browse to (userinfo, or none
      // stored). Nothing to report: the menu item is a convenience.
    });
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

  // Startup fallback for the background art. Re-runs as the registry loads;
  // `seedLastViewed` is idempotent and never overwrites a real view.
  $effect(() => {
    seedLastViewed(installed.list);
  });

  // The only report an auto upload has: it runs after the game has exited,
  // with no command in flight and usually no popup open. Mounted here, not
  // in Details.svelte, because the Shell is mounted exactly once and is
  // never `hidden` — the same reason `Toast.svelte` lives here.
  $effect(() => {
    const unlisten = listen<CloudUploadFinished>(CLOUD_UPLOAD_FINISHED_EVENT, (e) => {
      const { title, message, failed } = e.payload;
      pushToast(title === '' ? message : `${title} — ${message}`, failed ? 'error' : 'success');
    });
    return () => {
      void unlisten.then((off) => off());
    };
  });
</script>

<svelte:window onkeydown={onKeydown} onpointerdown={onWindowPointerDown} />

<BackgroundArt />

<header data-testid="shell-topbar" class="topbar">
  <div class="brand">
    <span class="logo"><Icon name="grid" size={20} /></span>
    <span class="wordmark">GRID</span>
  </div>

  <nav class="pills" aria-label="Views">
    {#each VIEWS as v (v)}
      <button
        data-testid={`nav-${v}`}
        class="pill"
        class:active={view === v}
        aria-current={view === v ? 'page' : undefined}
        onclick={() => (view = v)}
      >
        {viewLabel(v)}
      </button>
    {/each}
  </nav>

  <div class="session" bind:this={sessionEl}>
    {#if appUpdate.notice}
      <button
        data-testid="app-update-badge"
        class="update-badge"
        title={`GRID Launcher ${appUpdate.notice.tag} is available`}
        onclick={() => {
          view = 'settings';
          settings?.show('updates');
        }}
      >
        Update
      </button>
    {/if}
    <span class="status-dot" class:online={session.connected} aria-hidden="true"></span>
    <button
      data-testid="session-chip"
      class="chip"
      title={session.lastError ?? undefined}
      aria-expanded={serverMenuOpen}
      onclick={() => (serverMenuOpen = !serverMenuOpen)}
    >
      {chipLabel(session)}
    </button>
    {#if serverMenuOpen}
      <div class="server-menu" role="menu">
        {#if !session.connected}
          <button
            data-testid="session-retry"
            role="menuitem"
            disabled={session.busy}
            onclick={() => { serverMenuOpen = false; retry(); }}
          >
            Reconnect
          </button>
        {/if}
        <button
          data-testid="session-disconnect"
          role="menuitem"
          onclick={() => { serverMenuOpen = false; disconnect(); }}
        >
          Disconnect
        </button>
        <button data-testid="session-open-romm" role="menuitem" onclick={openServer}>
          Open RomM in browser
        </button>
        <span class="menu-host" role="none">{hostOf(session.serverUrl)}</span>
      </div>
    {/if}
  </div>
</header>

{#if !session.connected && session.lastError}
  <p data-testid="session-error" class="error-line over-art">{session.lastError}</p>
{/if}

<!-- All five views stay mounted and switch with `hidden` (design §3), so
     selections and in-flight fetches survive a switch. Scroll offsets do
     not: each view scrolls inside its own container, and a `display: none`
     subtree drops its scroll position. -->
<div data-testid="library-view" class="view" hidden={view !== 'library'}>
  <Library active={view === 'library'} bind:this={library} />
</div>
<div data-testid="server-view" class="view" hidden={view !== 'server'}>
  <Server
    active={view === 'server'}
    onOpenEmulators={() => {
      // Design §6: the default-emulator chip links to Emulators › Platform defaults.
      view = 'emulators';
      emulators?.show('defaults');
    }}
    bind:this={server}
  />
</div>
<div data-testid="downloads-view" class="view" hidden={view !== 'downloads'}>
  <Downloads />
</div>
<div class="view" hidden={view !== 'emulators'}>
  <Emulators active={view === 'emulators'} bind:this={emulators} />
</div>
<div data-testid="settings-view" class="view" hidden={view !== 'settings'}>
  <Settings active={view === 'settings'} bind:this={settings} />
</div>

<!-- Mounted outside the view roots, and never hidden. The footer strip is
     `position: fixed` global chrome, and `hidden` on an ancestor is
     `display: none` — inside a view root the strip would vanish from every
     other view. -->
<DownloadsFooter onOpen={() => (view = 'downloads')} />

<!-- Mounted here for the same reason as the footer strip: `position: fixed`
     global chrome inside a `hidden` view root would vanish with that view. -->
<Toast />

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    /* `flex: none` so a tall view can never compress the bar: `#app` is now
       a fixed viewport height, and every other row there is shrinkable. */
    flex: none;
    height: var(--topbar-h);
    padding: 0 16px;
    box-sizing: border-box;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 5;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 0;
    min-width: 0;
    color: var(--text-h);
  }

  /* Just the colour carrier for the brandmark: the `Icon` inside paints with
     `currentColor`, and `display: flex` keeps the 20px mark from sitting on
     the wordmark's text baseline. */
  .logo {
    display: flex;
    color: var(--primary);
  }

  .wordmark {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.08em;
  }

  .pills {
    display: flex;
    gap: 4px;
    flex: 0 0 auto;
    padding: 3px;
    border-radius: var(--r-pill);
    background: var(--surface);
  }

  .pill {
    font: inherit;
    font-size: 13px;
    padding: 5px 16px;
    border-radius: var(--r-pill);
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--m-fast) ease, color var(--m-fast) ease;
  }

  .pill:hover {
    color: var(--text-h);
  }

  .pill.active {
    background: var(--primary);
    color: #fff;
  }

  .session {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    flex: 1 1 0;
    min-width: 0;
  }

  .status-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }

  .status-dot.online {
    background: var(--success);
  }

  .chip {
    font: inherit;
    font-size: 13px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }

  .chip:hover {
    border-color: var(--border);
  }

  .update-badge {
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
  }

  .server-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 6;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    min-width: 200px;
    padding: 4px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }

  .server-menu button {
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .server-menu button:hover:not(:disabled) {
    background: var(--surface);
  }

  .server-menu button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .menu-host {
    padding: 6px 10px 4px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-line {
    flex: none;
    margin: 0;
    padding: 4px 16px;
    color: var(--danger);
    font-size: 11px;
  }

  .view {
    flex: 1 1 auto;
    min-height: 0;
    /* Positioned, but `z-index: auto`: a stacking context here would trap
       the Details dialog (z 20) below the top bar and the footer strip.
       The background art sits at z -1 so these wrappers still paint over
       it in DOM order. */
    position: relative;
    /* Clearance under the fixed 28px download strip (design §3), applied
       once here instead of a bottom padding per view. */
    padding-bottom: calc(var(--footer-h) + 24px);
    box-sizing: border-box;
    /* The app's scroll boundary. `#app` is exactly one viewport tall, so a
       view that overflows scrolls HERE and never on the document; the fixed
       `BackgroundArt` layer then stays composited instead of repainting its
       40px blur on every frame. Views that own their own scrollers (all four
       rail views) sit at `height: 100%` and never reach this one.
       Does not create a stacking context, so the note above still holds. */
    overflow-y: auto;
  }

</style>
