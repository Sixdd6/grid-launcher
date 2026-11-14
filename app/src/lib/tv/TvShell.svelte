<script lang="ts">
  // TV mode's shell (TV mode plan § 1.1; items 2–8, 77, 119–138): the
  // background art, the top bar, the stage, the hint bar and the overlay
  // layer, in a fullscreen column sized in rem. App.svelte mounts it instead
  // of the desktop Shell while the mode is TV, and sets html[data-ui="tv"],
  // which tv.css is scoped to.
  import { tick, untrack } from 'svelte';
  import './tv.css';
  import './stores/tvstores.css';
  import { startStoreEvents } from './stores/storeEvents';
  import { connectPrefill } from '../connectPrefill';
  import { librarySelection } from '../library/selection.svelte';
  import { drives } from '../stores/drives.svelte';
  import { libraryUnreachable } from '../stores/libraryUnreachable.svelte';
  import { registerNavigation } from '../stores/navigation.svelte';
  import { session } from '../stores/session.svelte';
  import { uiMode } from '../stores/uiMode.svelte';
  import { setTvThemeActive, setTvWelcomed, uiSettings } from '../stores/uiSettings.svelte';
  import { initialView } from '../shell';
  import { tvPaneFor, tvTabFor } from '../uiMode';
  import { startClock } from './clock.svelte';
  import { startRouter } from './input/router.svelte';
  import {
    attachRoots,
    nav,
    registerMenu,
    registerView,
    focusMemory,
    resetNav,
    resetStartup,
    restoreFocus,
    revealFocus,
    setFocus,
    setPane,
    setStartup,
    showScreen,
    showTab,
    stageChanged,
  } from './nav.svelte';
  import { attachNotices, showNotice, tvNotices, viewNewest } from './notices.svelte';
  import { WELCOME } from './notices';
  import TvLayer from './overlays/TvLayer.svelte';
  import { confirmBox, openKeyboard, openList, openMenu, openStartupMenu } from './overlays/overlays.svelte';
  import { notify } from '../notifications/notifier';
  import { openDetails, openSystem, playGame } from './gameActions';
  import { selectDriveRow } from './tabs/library/library.svelte';
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
    (window as unknown as { __tvE2E?: unknown }).__tvE2E = {
      notify,
      openList,
      openKeyboard,
      confirmBox,
      // Every notice of the session, oldest first, as "title" or "title | sub":
      // a card lasts ten seconds, so specs read this log instead (P13).
      memory: () => focusMemory(),
      // Focuses an item as a controller would reach it: TV mode ignores the
      // mouse, so specs that jump to an item use this, then press A.
      focus: (el: HTMLElement | null) => setFocus(el),
      // Switches to a top-bar tab as LB / RB would.
      tab: (t: string) => void showTab(t as Parameters<typeof showTab>[0]),
      notices: () =>
        [...tvNotices.history]
          .reverse()
          .map((h) => (typeof h.sub === 'string' && h.sub !== '' ? `${h.title} | ${h.sub}` : h.title)),
    };
  }

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
    selectDriveRow(key);
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
    // The welcome card, once the main screen is up the first time TV mode
    // ever opens (`[ui] tv_welcomed`; Menu › Controls shows it again; user
    // decision 2026-11-10).
    void showScreen().then(() => {
      if (uiSettings.tvWelcomed) return;
      showNotice(WELCOME);
      void setTvWelcomed().catch(() => {});
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
    // A startup screen's Menu holds only the ways out (user decision
    // 2026-11-10).
    registerMenu(() => void (nav.startup ? openStartupMenu() : openMenu()));
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
  // Game stores: notices as cards, the login's end, installer failures.
  $effect(() => startStoreEvents());
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
      // A "<title> installed" card's Play (TVStores prototype's installed toast).
      play: (game) => void playGame(game),
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


  // A new window size (TV mode going fullscreen after it is drawn, a
  // resized window) changes 1rem: the scroll rules run again for the
  // focused item, once the layout has settled.
  let resizeFrame = 0;
  function onResize() {
    cancelAnimationFrame(resizeFrame);
    resizeFrame = requestAnimationFrame(() => revealFocus());
  }

  function onKeydown(e: KeyboardEvent) {
    // Tab would walk the browser's focus through the page and scroll it.
    if (e.key === 'Tab') e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} onresize={onResize} />

<div class="tv" data-tv-root data-size={uiSettings.tvCardSize} data-labels={uiSettings.tvCardLabels} data-testid="tv-shell">
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
