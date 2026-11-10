<script lang="ts">
  // The Menu (Start; items 362–366; tvmode.js `guideMenu`): a 30rem panel
  // from the right with the clock, "<user> · <server host>" (or "Offline"),
  // then Discover, Downloads (with the active count), Notifications (with
  // the count), Settings, Reconnect, Switch to desktop mode and Quit GRID.
  // Each item closes the Menu first. The prototype's "Prototype only"
  // section is not part of the app.
  import { downloads } from '../../stores/downloads.svelte';
  import { session } from '../../stores/session.svelte';
  import { leaveTv } from '../../stores/uiMode.svelte';
  import { clock } from '../clock.svelte';
  import { tvf } from '../focus/actions';
  import { closeLayer, nav, pushPage, showTab } from '../nav.svelte';
  import { tvNotices } from '../notices.svelte';
  import TvNotifications from '../pages/TvNotifications.svelte';
  import { reconnect } from '../reconnect';
  import { activeCount, menuHead, menuItems, type MenuItemKey } from './menu';
  import { quitBox } from './overlays.svelte';

  let items = $derived(menuItems(activeCount(downloads.entries), tvNotices.history.length));

  const RUN: Record<MenuItemKey, () => void> = {
    // TODO(P7): select Discover on the Server rail; Server's `arrive` then
    // focuses Discover's content (items 104, 365).
    discover: () => void showTab('server'),
    downloads: () => void showTab('downloads'),
    notifications: () => void pushPage({ title: 'Notifications', component: TvNotifications }, { fresh: true }),
    settings: () => void showTab('settings'),
    reconnect: () => void reconnect(),
    desktop: () => void leaveTv(nav.tab, nav.pane),
    quit: () => void quitBox(),
  };

  function pick(key: MenuItemKey) {
    void closeLayer().then(RUN[key]);
  }
</script>

<aside class="guide" data-nav="v" data-scroll="guide" data-testid="tv-menu" aria-label="Menu">
  <div class="gh"><b data-testid="tv-menu-clock">{clock.text}</b><span class="muted" data-testid="tv-menu-head">{menuHead(session)}</span></div>
  {#each items as it, i (it.key)}
    <button class="opt" data-k={`gm${i}`} data-testid={`tv-menu-${it.key}`} use:tvf={{ a: () => pick(it.key) }}>{it.label}</button>
  {/each}
</aside>
