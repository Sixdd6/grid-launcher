<script lang="ts">
  // TV mode's shell (TV mode plan § 1.1; items 2–8, 77, 119–138): the
  // background art, the top bar, the stage, the hint bar and the overlay
  // layer, in a fullscreen column sized in rem. App.svelte mounts it instead
  // of the desktop Shell while the mode is TV, and sets html[data-ui="tv"],
  // which tv.css is scoped to.
  import { tick, untrack } from 'svelte';
  import './tv.css';
  import { connectPrefill } from '../connectPrefill';
  import { driveRailKey } from '../library/rail';
  import { librarySelection } from '../library/selection.svelte';
  import { drives } from '../stores/drives.svelte';
  import { libraryUnreachable } from '../stores/libraryUnreachable.svelte';
  import { registerNavigation } from '../stores/navigation.svelte';
  import { session } from '../stores/session.svelte';
  import { uiMode } from '../stores/uiMode.svelte';
  import { setTvThemeActive, uiSettings } from '../stores/uiSettings.svelte';
  import { initialView } from '../shell';
  import { tvPaneFor, tvTabFor } from '../uiMode';
  import { startClock } from './clock.svelte';
  import { startRouter } from './input/router.svelte';
  import {
    attachRoots,
    nav,
    registerMenu,
    registerView,
    rememberFocus,
    resetNav,
    resetStartup,
    restoreFocus,
    setPane,
    setStartup,
    showScreen,
    showTab,
    stageChanged,
  } from './nav.svelte';
  import { attachNotices, showNotice, viewNewest } from './notices.svelte';
  import { WELCOME } from './notices';
  import TvLayer from './overlays/TvLayer.svelte';
  import { confirmBox, openKeyboard, openList, openMenu } from './overlays/overlays.svelte';
  import { notify } from '../notifications/notifier';
  import { openDetails, openSystem } from './gameActions';
  import { lib, selectLibRail } from './tabs/library/library.svelte';
  import { railRowKey } from './tabs/library/libraryRail';
  import { selectRail as selectServerRail } from './tabs/server/server.svelte';
  import { resetConnectForm } from './startup/connectForm.svelte';
  import TvChooseSystems from './startup/TvChooseSystems.svelte';
  import TvConnect from './startup/TvConnect.svelte';
  import TvSetupCard from './startup/TvSetupCard.svelte';
  import TvUnreachable from './startup/TvUnreachable.svelte';
  import TvToasts from './TvToasts.svelte';
  import TvBackground from './TvBackground.svelte';
  import TvHintBar from './TvHintBar.svelte';
  import TvStage from './TvStage.svelte';
  import TvTopBar from './TvTopBar.svelte';

  // e2e only (the build drops it otherwise): the overlays and a notice with
  // an action, reachable before the screens that open them exist (P7–P13).
  if (import.meta.env.VITE_E2E) {
    (window as unknown as { __tvE2E?: unknown }).__tvE2E = { notify, openList, openKeyboard, confirmBox };
  }

  let rootEl = $state<HTMLElement | null>(null);
  let stageEl = $state<HTMLElement | null>(null);
  let layersEl = $state<HTMLElement | null>(null);

  // What the stage shows (P12): a startup screen — the unreachable library
  // (GRID can't run without it), Connect, Choose your systems — or the
  // tabs. Nothing until the session and the settings are known.
  type Phase = 'loading' | 'unreachable' | 'connect' | 'choose' | 'shell';
  let phase = $derived<Phase>(
    !uiSettings.loaded || session.phase === 'loading'
      ? 'loading'
      : libraryUnreachable.blocking
        ? 'unreachable'
        : session.phase === 'none'
          ? 'connect'
          : session.phase === 'setup'
            ? 'choose'
            : 'shell',
  );

  let entered: Phase | null = null;
  let firstStart = true;
  let welcomed = false;
  /** Used without a server before the drives were known: the first drive
   *  is selected once they are, unless the player has moved on (item 415). */
  let pendingDrive: string | null = null;

  $effect(() => {
    const p = phase;
    untrack(() => {
      if (p === entered) return;
      entered = p;
      setStartup(p !== 'shell');
      if (p === 'loading') return;
      if (p === 'connect') {
        resetConnectForm(connectPrefill(session));
        void resetStartup({ title: 'Connect', component: TvConnect });
      } else if (p === 'choose') {
        void resetStartup({ title: 'Choose your systems', component: TvChooseSystems });
      } else if (p === 'unreachable') {
        void resetStartup({ title: 'Your library', component: TvUnreachable });
      } else {
        startShell();
      }
    });
  });

  /** The Library on a Backup drive: its folder open, its row selected and
   *  focused (item 415; tvmode.js `showDrive`). */
  function selectDrive(key: string) {
    const rail = driveRailKey(key);
    selectLibRail(rail);
    lib.folds.drives = true;
    rememberFocus('tab:library', railRowKey(rail));
  }

  /**
   * Where TV mode opens: the desktop view it was entered from (the TV
   * button), else the shared start rule (Open GRID on; items 413–416):
   * Server › Discover when connected, else the Library, on the first
   * connected Backup drive when used without a server.
   */
  function startShell() {
    const from = firstStart ? uiMode.tvStart : null;
    firstStart = false;
    const start = from ? null : initialView(session.connected, session.offline, drives.firstLibraryDrive, uiSettings.openOn);
    const tab = from ? from.tab : tvTabFor(start!.view);
    resetNav(tab, from?.pane ?? null);
    pendingDrive = null;
    if (!from && tab === 'server') selectServerRail('discover');
    if (start?.drive) selectDrive(start.drive);
    else if (!from && tab === 'library' && session.offline && !session.connected && !drives.loaded) pendingDrive = librarySelection.key;
    // The welcome card, the first time the main screen is up (ruling 15).
    void showScreen().then(() => {
      if (welcomed) return;
      welcomed = true;
      showNotice(WELCOME);
    });
  }

  $effect(() => {
    if (!drives.loaded) return;
    untrack(() => {
      const startKey = pendingDrive;
      if (startKey === null) return;
      pendingDrive = null;
      const first = drives.firstLibraryDrive;
      if (first === null || phase !== 'shell' || nav.tab !== 'library' || nav.pages.length || librarySelection.key !== startKey) return;
      selectDrive(first);
      void showTab('library', { again: true });
    });
  });

  // TV mode's own card stack replaces the desktop's while it is up
  // (items 139–150); Start opens the Menu and View acts on the newest card
  // with an action (items 102, 148).
  $effect(() => {
    attachNotices(true);
    registerMenu(() => void openMenu());
    // View acts on the newest card with an action, else the screen's own
    // (Connect pastes, item 396).
    registerView(() => {
      if (!viewNewest()) nav.hooks.view?.();
    });
    return () => {
      registerMenu(null);
      registerView(null);
      attachNotices(false);
    };
  });

  $effect(() => {
    attachRoots(stageEl, layersEl);
    return () => attachRoots(null, null);
  });

  // "Follow system" shows dark while TV mode is up (ruling 22).
  $effect(() => {
    setTvThemeActive(true);
    return () => setTvThemeActive(false);
  });

  $effect(() => startClock());
  $effect(() => startRouter());

  // Toasts' actions and the setup card's Open Downloads land here while TV
  // mode is up (the same intents the desktop answers).
  $effect(() =>
    registerNavigation({
      go: (view) => void showTab(tvTabFor(view)),
      goSettings: (page) => {
        setPane(tvPaneFor(page));
        void showTab('settings');
      },
      openDetails,
      openSystem,
    }),
  );

  // A redraw keeps focus on the same item, or the one in its place (item
  // 115); late content gets focus once it arrives (item 77).
  $effect(() => {
    const roots = [stageEl, layersEl].filter((e): e is HTMLElement => e !== null);
    if (roots.length === 0) return;
    const mo = new MutationObserver(() => stageChanged());
    for (const r of roots) {
      mo.observe(r, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-al', 'data-xl', 'aria-disabled', 'disabled', 'class'] });
    }
    void tick().then(() => restoreFocus());
    return () => mo.disconnect();
  });

  // Focus is a class here, never the browser's: a click must not move the
  // browser's focus (and scroll) to the button pressed.
  $effect(() => {
    const el = rootEl;
    if (!el) return;
    const keep = (e: MouseEvent) => e.preventDefault();
    el.addEventListener('mousedown', keep);
    return () => el.removeEventListener('mousedown', keep);
  });

  function onKeydown(e: KeyboardEvent) {
    // Tab would walk the browser's focus through the page and scroll it.
    if (e.key === 'Tab') e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="tv" data-tv-root data-testid="tv-shell" bind:this={rootEl}>
  <TvBackground />
  <TvTopBar />
  <main class="stage" bind:this={stageEl} data-testid="tv-stage">
    <!-- A startup screen is the only page on nav's stack (resetStartup), so
         the stage draws it, and the folder browser over it, like any page. -->
    {#if phase === 'shell' || (phase !== 'loading' && nav.pages.length > 0)}
      <TvStage />
    {/if}
  </main>
  <TvHintBar />
  <div class="layers" bind:this={layersEl}>
    {#each nav.layers as layer (layer.id)}
      <TvLayer {layer} />
    {/each}
  </div>
  <TvToasts />
  {#if phase === 'shell'}<TvSetupCard />{/if}
</div>
