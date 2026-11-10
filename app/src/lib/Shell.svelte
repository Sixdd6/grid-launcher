<script lang="ts">
  import Library from './Library.svelte';
  import Server from './Server.svelte';
  import Downloads from './Downloads.svelte';
  import DownloadsFooter from './DownloadsFooter.svelte';
  import BackgroundArt from './BackgroundArt.svelte';
  import Icon from './Icon.svelte';
  import Settings from './Settings.svelte';
  import ThemeToggle from './ThemeToggle.svelte';
  import BackupDialog from './backups/BackupDialog.svelte';
  import { backupDialog, closeBackup } from './backups/dialogState.svelte';
  import { drives } from './stores/drives.svelte';
  import { driveRailKey } from './library/rail';
  import { librarySelection, selectRail } from './library/selection.svelte';
  import { api } from './api';
  import { session, retry, disconnect, connectToServer } from './stores/session.svelte';
  import { appUpdate } from './stores/appUpdate.svelte';
  import { noteInput, notePointerAt } from './stores/inputMode.svelte';
  import { registerNavigation } from './stores/navigation.svelte';
  import type { SettingsPage } from './settings/pages';
  import { toggleTheme, uiSettings } from './stores/uiSettings.svelte';
  import { chipLabel, hostOf, initialView, VIEWS, viewForDigit, viewLabel, type View } from './shell';
  // The same guard the grid views use for Ctrl+F: an accelerator must stay
  // out of the way while focus sits in a text-entry control (where
  // Ctrl+<n> can be an editor chord) or a modal dialog owns the screen and
  // switching the view behind it would strand it.
  import { chordBlocked, chordContext } from './views/searchKeys';

  // Set once when the shell first mounts (R2): Server when the restored/just
  // -connected session is online, Library when it came up offline. Switching
  // views afterward is a user action — a pill, Ctrl+1..4, or `show()`.
  // Offline mode opens Library on the first connected backup drive
  // (GameBackups README, "Use without a server").
  const start = initialView(session.connected, session.offline, drives.firstLibraryDrive);
  let view = $state<View>(start.view);
  if (start.drive !== null) selectRail(driveRailKey(start.drive));

  // The drives store may not have its first snapshot when the shell mounts.
  // Offline, apply the drive half once it arrives, unless the user has moved
  // on (another view, or another Library row) by then.
  const startRail = librarySelection.key;
  let driveStartSettled = start.drive !== null || !session.offline || session.connected;
  $effect(() => {
    if (driveStartSettled || !drives.loaded) return;
    driveStartSettled = true;
    const first = drives.firstLibraryDrive;
    if (first !== null && view === 'library' && librarySelection.key === startRail) selectRail(driveRailKey(first));
  });

  /** Library on a backup drive (Settings › Backup drives' Show in Library). */
  function showDriveInLibrary(key: string) {
    selectRail(driveRailKey(key));
    view = 'library';
  }

  let settings = $state<ReturnType<typeof Settings> | null>(null);
  let serverMenuOpen = $state(false);
  let sessionEl = $state<HTMLElement | null>(null);

  /** Programmatic navigation, for the footer strip and the update badge. */
  export function show(next: View) {
    view = next;
  }

  /** A Settings page, with one system's row opened on Systems. */
  function showSettings(page: SettingsPage, opts: { system?: string } = {}) {
    view = 'settings';
    settings?.show(page, opts);
  }

  // The navigation intents (toast actions, the setup card's Open Downloads)
  // land here while the desktop shell is mounted.
  $effect(() => registerNavigation({ go: show, goSettings: showSettings }));

  // Ctrl+1..4 (design §3). Alt/Shift are excluded so this never steals a
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

  /** A pointer move puts the app back in pointer mode only once the pointer
   *  has actually MOVED: WebKit dispatches a synthetic `pointermove` at the
   *  unchanged cursor position after any scroll, including the
   *  `scrollIntoView` an arrow key triggers, and that must not take the
   *  selection away from a keyboard user whose mouse merely rests on the
   *  grid. The threshold and the anchor live in the store (`notePointerAt`),
   *  which is where they can be tested. */
  function onWindowPointerMove(e: PointerEvent) {
    notePointerAt(e.clientX, e.clientY);
  }

  /** A click anywhere outside the session cluster closes the server menu. */
  function onWindowPointerDown(e: MouseEvent) {
    noteInput('pointer');
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
</script>

<svelte:window
  onkeydown={onKeydown}
  onpointerdown={onWindowPointerDown}
  onpointermove={onWindowPointerMove}
/>

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
        onclick={() => showSettings('updates')}
      >
        Update
      </button>
    {/if}
    <!-- A failed save is not worth an error here: the theme is already
         applied, the same trade the Appearance select makes. -->
    <ThemeToggle resolved={uiSettings.resolved} onToggle={() => toggleTheme().catch(() => {})} />
    <span
      data-testid="session-dot"
      class="status-dot"
      class:online={session.connected}
      class:offline={session.offline && !session.connected}
      aria-hidden="true"
    ></span>
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
        {#if session.offline && !session.connected}
          <!-- Offline mode (GameBackups README): one way out, back to the
               Connect screen. Nothing is cleared until a connect succeeds. -->
          <button
            data-testid="session-connect"
            role="menuitem"
            onclick={() => { serverMenuOpen = false; connectToServer(); }}
          >
            Connect to a server…
          </button>
          <span class="menu-host" role="none">Library and backup drives work offline</span>
        {:else}
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
        {/if}
      </div>
    {/if}
  </div>
</header>

{#if !session.connected && session.lastError}
  <p data-testid="session-error" class="error-line">{session.lastError}</p>
{/if}

<!-- All four views stay mounted and switch with `hidden` (design §3), so
     selections and in-flight fetches survive a switch. Scroll offsets do
     not: each view scrolls inside its own container, and a `display: none`
     subtree drops its scroll position. -->
<div data-testid="library-view" class="view" hidden={view !== 'library'}>
  <Library active={view === 'library'} />
</div>
<div data-testid="server-view" class="view" hidden={view !== 'server'}>
  <Server
    active={view === 'server'}
    onOpenSystems={(system) => {
      // The emulator and firmware chips open Settings › Systems on the
      // platform's row, opened (ProposedApp README).
      showSettings('systems', { system });
    }}
  />
</div>
<div data-testid="downloads-view" class="view scrolls" hidden={view !== 'downloads'}>
  <Downloads />
</div>
<div data-testid="settings-view" class="view" hidden={view !== 'settings'}>
  <Settings active={view === 'settings'} bind:this={settings} onShowDriveInLibrary={showDriveInLibrary} />
</div>

<!-- Mounted outside the view roots, and never hidden: `hidden` on an
     ancestor is `display: none`, so inside a view root the strip would
     vanish from every other view. It is the last row of `#app`'s column,
     so the views end where it starts. -->
<DownloadsFooter onOpen={() => (view = 'downloads')} />

<!-- The one Backup dialog (GameBackups README), over every view and over
     Details. "Set up a drive…" goes to Settings › Backup drives. -->
{#if backupDialog.current}
  {#key backupDialog.current}
    <BackupDialog
      open={backupDialog.current}
      onClose={closeBackup}
      onSetUp={() => {
        // Details (when it opened the dialog) closes too: Settings is not
        // behind it.
        backupDialog.current?.onLeave?.();
        closeBackup();
        showSettings('backup-drives');
      }}
    />
  {/key}
{/if}

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
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border-bottom: 1px solid var(--line);
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
    background: var(--field);
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
    color: var(--on-primary);
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

  /* Offline mode is chosen, not a fault: grey, not the danger red. */
  .status-dot.offline {
    background: var(--text-muted);
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
    color: var(--on-primary);
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
    border: 1px solid var(--line);
    background: var(--overlay);
    box-shadow: var(--shadow-overlay);
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
    background: var(--panel);
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
    /* No bottom padding: the download strip is a row of `#app`'s column
       under the views (DownloadsFooter.svelte), not a fixed overlay, so
       nothing needs clearing — and a rail view's rail runs to the bottom
       edge. A view that scrolls as a whole gets its breathing room from
       `.view.scrolls` below. */
    box-sizing: border-box;
    /* The gutter around the view's panels: the art shows through it. */
    padding: var(--gutter);
    /* The app's scroll boundary. `#app` is exactly one viewport tall, so a
       view that overflows scrolls HERE and never on the document; the fixed
       `BackgroundArt` layer then stays composited instead of repainting its
       40px blur on every frame. Views that own their own scrollers (all four
       rail views) sit at `height: 100%` and never reach this one.
       Does not create a stacking context, so the note above still holds. */
    overflow-y: auto;
  }

  /* Downloads is the one view with no rail: it scrolls as a whole, so its
     last row keeps some room above the window's (or the strip's) edge. */
  .view.scrolls {
    padding-bottom: calc(var(--gutter) + 12px);
  }

</style>
