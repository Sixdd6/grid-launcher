<script lang="ts">
  import { untrack } from 'svelte';
  import Library from './Library.svelte';
  import Downloads from './Downloads.svelte';
  import DownloadsFooter from './DownloadsFooter.svelte';
  import BackgroundArt from './BackgroundArt.svelte';
  import Icon from './Icon.svelte';
  import Settings from './Settings.svelte';
  import ThemeToggle from './ThemeToggle.svelte';
  import BackupDialog from './backups/BackupDialog.svelte';
  import { backupDialog, closeBackup } from './backups/dialogState.svelte';
  import { driveRailKey } from './library/rail';
  import { jumpToRail, selectRail } from './library/selection.svelte';
  import { appUpdate } from './stores/appUpdate.svelte';
  import { noteInput, notePointerAt } from './stores/inputMode.svelte';
  import { registerNavigation } from './stores/navigation.svelte';
  import type { SettingsPage } from './settings/pages';
  import { toggleTheme, uiSettings } from './stores/uiSettings.svelte';
  import {
    initialPlace,
    STOREFRONT_VIEWS,
    storefrontViewForDigit,
    viewLabel,
    type View,
  } from './shell';
  // The same guard the grid views use for Ctrl+F: an accelerator must stay
  // out of the way while focus sits in a text-entry control (where
  // Ctrl+<n> can be an editor chord) or a modal dialog owns the screen and
  // switching the view behind it would strand it.
  import { chordBlocked, chordContext } from './views/searchKeys';
  import { enterTv, uiMode } from './stores/uiMode.svelte';
  import StoreLayer from './gamestores/desktop/StoreLayer.svelte';

  let {
    initialView: forcedView = undefined,
    initialSettingsPage = undefined,
  }: {
    /** The view to open on instead of the start rule: Switch to desktop
     *  mode lands on the view TV mode was showing (TV mode item 523). */
    initialView?: View;
    /** With `initialView` 'settings': the page to show. */
    initialSettingsPage?: SettingsPage;
  } = $props();

  // Set once when the shell first mounts (plan 0.5, decision 16): Library ›
  // All games. Discover and Open GRID on are removed for now (2026-11-17).
  // Switching views afterward is a user action — a pill, Ctrl+1..3, or `show()`.
  const forced = untrack(() => forcedView);
  let view = $state<View>('library');
  untrack(() => {
    if (forced !== undefined) {
      view = forced;
      return;
    }
    const place = initialPlace();
    view = place.view;
    selectRail(place.rail);
  });

  /** Library on a backup drive (Settings › Backup drives' Show in Library). */
  function showDriveInLibrary(key: string) {
    jumpToRail(driveRailKey(key));
    view = 'library';
  }

  let settings = $state<ReturnType<typeof Settings> | null>(null);

  // Back from TV mode on Settings: the page its pane matches, once.
  let settingsPageApplied = false;
  $effect(() => {
    if (settingsPageApplied || settings === null || initialSettingsPage === undefined) return;
    settingsPageApplied = true;
    settings.show(initialSettingsPage);
  });

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

  // Ctrl+1..3 (RommStorefront README "Removed"). Alt/Shift are excluded so
  // this never steals a window-manager or text-editing chord; Meta is
  // accepted alongside Ctrl so the same accelerator works on macOS.
  function onKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    const next = storefrontViewForDigit(e.key);
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

  function onWindowPointerDown() {
    noteInput('pointer');
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
    {#each STOREFRONT_VIEWS as v (v)}
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

  <!-- No server chip or menu (decision 6): RomM's status shows on its
       Library group and its Settings card, like the stores. -->
  <div class="session">
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
    <!-- TV mode (TVMode README § Decided "Entering and leaving"; the look
         is § Consultant decisions): an icon button like the light / dark
         toggle beside it, opening TV mode on the tab matching this view. -->
    <button
      type="button"
      data-testid="tv-mode-button"
      class="icon-btn tv-btn"
      aria-label="Switch to TV mode"
      title="Switch to TV mode"
      disabled={uiMode.mode !== 'desktop'}
      onclick={() => enterTv(view).catch(() => {})}
    >
      <Icon name="tv" size={18} />
    </button>
    <!-- A failed save is not worth an error here: the theme is already
         applied, the same trade the Appearance select makes. -->
    <ThemeToggle resolved={uiSettings.resolved} onToggle={() => toggleTheme().catch(() => {})} />
  </div>
</header>

<!-- The three views stay mounted and switch with `hidden` (design §3), so
     selections and in-flight fetches survive a switch. Scroll offsets do
     not: each view scrolls inside its own container, and a `display: none`
     subtree drops its scroll position. -->
<div data-testid="library-view" class="view" hidden={view !== 'library'}>
  <Library active={view === 'library'} />
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
<!-- The store screens' dialogs (GameStores README): store Details, Install,
     Game Settings, the cloud conflict prompt, the offline installer and the
     GOG backup dialog, over every view. -->
<StoreLayer onSetUpDrive={() => showSettings('backup-drives')} />

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

  .tv-btn {
    color: var(--text-h);
  }

  .tv-btn:disabled {
    color: var(--text-muted);
    cursor: default;
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
