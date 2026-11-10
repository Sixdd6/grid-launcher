<script lang="ts">
  // TV mode's shell (TV mode plan § 1.1; items 2–8, 77, 119–138): the
  // background art, the top bar, the stage, the hint bar and the overlay
  // layer, in a fullscreen column sized in rem. App.svelte mounts it instead
  // of the desktop Shell while the mode is TV, and sets html[data-ui="tv"],
  // which tv.css is scoped to.
  import { tick } from 'svelte';
  import './tv.css';
  import { drives } from '../stores/drives.svelte';
  import { libraryUnreachable } from '../stores/libraryUnreachable.svelte';
  import { registerNavigation } from '../stores/navigation.svelte';
  import { session } from '../stores/session.svelte';
  import { leaveTv, uiMode } from '../stores/uiMode.svelte';
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
    resetNav,
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
  import TvToasts from './TvToasts.svelte';
  import TvBackground from './TvBackground.svelte';
  import TvHintBar from './TvHintBar.svelte';
  import TvStage from './TvStage.svelte';
  import TvTopBar from './TvTopBar.svelte';
  import TvButton from './ui/TvButton.svelte';

  // e2e only (the build drops it otherwise): the overlays and a notice with
  // an action, reachable before the screens that open them exist (P7–P13).
  if (import.meta.env.VITE_E2E) {
    (window as unknown as { __tvE2E?: unknown }).__tvE2E = { notify, openList, openKeyboard, confirmBox };
  }

  let rootEl = $state<HTMLElement | null>(null);
  let stageEl = $state<HTMLElement | null>(null);
  let layersEl = $state<HTMLElement | null>(null);

  let startup = $derived(session.phase !== 'shell' || libraryUnreachable.blocking || !uiSettings.loaded);

  // Where TV mode opens: the desktop view it was entered from (the TV
  // button), else the shared start rule (Open GRID on; items 413–416).
  let started = false;
  $effect(() => {
    setStartup(startup);
    if (startup || started) return;
    started = true;
    const from = uiMode.tvStart;
    const tab = from ? from.tab : initialView(session.connected, session.offline, drives.firstLibraryDrive, uiSettings.openOn).view;
    resetNav(tab, from?.pane ?? null);
    // The welcome card, each time TV mode opens, once the main screen is
    // up (ruling 15).
    void showScreen().then(() => showNotice(WELCOME));
  });

  // TV mode's own card stack replaces the desktop's while it is up
  // (items 139–150); Start opens the Menu and View acts on the newest card
  // with an action (items 102, 148).
  $effect(() => {
    attachNotices(true);
    registerMenu(() => void openMenu());
    registerView(() => void viewNewest());
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
    {#if !startup}
      <TvStage />
    {:else if session.phase !== 'loading'}
      <!-- TODO(P12): TvConnect, TvChooseSystems and TvUnreachable. Until
           then a startup screen in TV mode offers the desktop's. -->
      <div class="start">
        <div class="start-box" data-nav="v">
          <h1>{libraryUnreachable.blocking ? "Your library can't be reached" : session.phase === 'setup' ? 'Choose your systems' : 'Connect to your RomM server'}</h1>
          <p class="sub">Finish this step in desktop mode.</p>
          <div class="btns start-foot" data-nav="h">
            <TvButton k="st-desktop" primary onA={() => void leaveTv(nav.tab, nav.pane)}>Switch to desktop mode</TvButton>
          </div>
        </div>
      </div>
    {/if}
  </main>
  <TvHintBar />
  <div class="layers" bind:this={layersEl}>
    {#each nav.layers as layer (layer.id)}
      <TvLayer {layer} />
    {/each}
  </div>
  <TvToasts />
</div>
