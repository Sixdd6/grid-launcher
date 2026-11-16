<script lang="ts">
  // TV mode's shell (TV mode plan § 1.1; items 2–8, 77, 119–138): the
  // background art, the top bar, the stage, the hint bar and the overlay
  // layer, in a fullscreen column sized in rem. App.svelte mounts it instead
  // of the desktop Shell while the mode is TV, and sets html[data-ui="tv"],
  // which tv.css is scoped to.
  import { tick, untrack } from 'svelte';
  import './tv.css';
  import './stores/tvstores.css';
  import './romm/tvromm.css';
  import { startStoreEvents } from './stores/storeEvents';
  import { rommConnecting, rommLive } from '../romm/rail';
  import { libraryUnreachable } from '../stores/libraryUnreachable.svelte';
  import { romm } from '../stores/romm.svelte';
  import { registerNavigation } from '../stores/navigation.svelte';
  import { session } from '../stores/session.svelte';
  import { uiMode } from '../stores/uiMode.svelte';
  import { setTvThemeActive, setTvWelcomed, uiSettings } from '../stores/uiSettings.svelte';
  import { initialPlace } from '../shell';
  import { tvPaneFor, tvTabFor } from '../uiMode';
  import { startClock } from './clock.svelte';
  import { startRouter } from './input/router.svelte';
  import {
    attachRoots,
    nav,
    registerMenu,
    registerView,
    rememberFocus,
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
  import { selectLibRail } from './tabs/library/library.svelte';
  import { expiredCard, unreachableCard } from './romm/rommActions';
  import { rommRowKey, selectRommView, toggleRommFold, tvRomm } from './romm/tvRomm.svelte';
  import { railRowKey } from './tabs/library/libraryRail';
  import TvChooseSystems from './startup/TvChooseSystems.svelte';
  import TvWelcome from './startup/TvWelcome.svelte';
  import { resetWelcome } from './startup/welcomeState.svelte';
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
  // (GRID can't run without it), the welcome, Choose your systems — or the
  // tabs. Nothing until the session and the settings are known.
  type Phase = 'loading' | 'unreachable' | 'welcome' | 'choose' | 'shell';
  let phase = $derived.by<Phase>(() => {
    if (!uiSettings.loaded || session.phase === 'loading') return 'loading';
    if (libraryUnreachable.blocking) return 'unreachable';
    if (session.phase === 'welcome') return 'welcome';
    if (session.phase === 'setup') return 'choose';
    return 'shell';
  });

  let entered: Phase | null = null;
  let firstStart = true;

  $effect(() => {
    const p = phase;
    untrack(() => {
      if (p === entered) return;
      entered = p;
      setStartup(p !== 'shell');
      if (p === 'loading') return;
      if (p === 'welcome') {
        void resetWelcome();
        void resetStartup({ title: 'Welcome', component: TvWelcome });
      } else if (p === 'choose') {
        void resetStartup({ title: 'Choose your systems', component: TvChooseSystems });
      } else if (p === 'unreachable') {
        void resetStartup({ title: 'Your library', component: TvUnreachable });
      } else {
        startShell();
      }
    });
  });

  /**
   * Where TV mode opens (TVRommStorefront README § Where GRID opens): the
   * desktop view it was entered from (the TV button); Settings › Game stores
   * with the RomM row focused after Choose your systems opened from the
   * RomM page (decision 9); else the shared start rule (Open GRID on):
   * Library › RomM › Discover when RomM is connected at startup, else All
   * games, never a Backup drive (decision 16). A RomM that then can't be
   * reached falls back to RomM › Installed (the Library's own rule).
   */
  function startShell() {
    const from = firstStart ? uiMode.tvStart : null;
    firstStart = false;
    if (from) {
      resetNav(from.tab, from.pane);
    } else if (session.setupReturn === 'settings') {
      session.setupReturn = null;
      resetNav('settings', 'stores');
      rememberFocus('tab:settings', 'sto-romm');
    } else {
      session.setupReturn = null;
      // "Connected at startup": the config says so, which the status shows
      // as connected, connecting, or a probe that failed (Can't reach,
      // Sign-in expired).
      const s = romm.status;
      const configured = rommLive(s) || rommConnecting(s) || s.connection === 'unreachable' || s.connection === 'expired';
      const place = initialPlace(configured, uiSettings.openOn);
      resetNav('library', null);
      if (place.rail === 'romm:discover') {
        selectRommView('romm:discover');
        if (!tvRomm.open) toggleRommFold();
        rememberFocus('tab:library', rommRowKey('romm:discover'));
      } else {
        selectLibRail('all');
        rememberFocus('tab:library', railRowKey('all'));
      }
    }
    // The welcome card, once the main screen is up the first time TV mode
    // ever opens (`[ui] tv_welcomed`; Menu › Controls shows it again; user
    // decision 2026-11-10).
    void showScreen().then(() => {
      if (uiSettings.tvWelcomed) return;
      showNotice(WELCOME);
      void setTvWelcomed().catch(() => {});
    });
  }

  // RomM's state changing under the shell: a 401 mid-session and a startup
  // probe that found no answer each raise one card (README § Library ›
  // RomM group). Reconnect and the sign-in page raise their own.
  let lastConn: { connection: string; connecting: boolean } | null = null;
  $effect(() => {
    const s = romm.status;
    const next = { connection: s.connection, connecting: rommConnecting(s) };
    untrack(() => {
      const prev = lastConn;
      lastConn = next;
      if (prev === null || phase !== 'shell') return;
      if (prev.connection === 'connected' && next.connection === 'expired') expiredCard();
      else if (prev.connecting && next.connection === 'unreachable') unreachableCard();
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
    // (the RomM sign-in page pastes).
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
