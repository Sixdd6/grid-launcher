<script lang="ts">
  // The welcome, step 2 (TVRommStorefront README § The welcome): "Connect
  // your games", optional. Five rows in name order (Amazon Games, Epic Games,
  // GOG, RomM, Steam), each with its mark, line and pill. A on a store opens
  // its login window over TV mode (Steam: its page); on RomM, the sign-in
  // page. A on a connected row opens a list: Disconnect (RomM also Edit
  // sign-in…). The button reads Skip for now until one is connected, then
  // Continue; with RomM connected and setup needed, Choose your systems
  // follows, else the shell. B goes back to step 1 (a page over it).
  import { api, type StoreSource } from '../../api';
  import { storeName } from '../../gamestores/marks';
  import { disconnectRomm, romm } from '../../stores/romm.svelte';
  import { finishWelcome, session } from '../../stores/session.svelte';
  import { gameStores, initGameStores } from '../../stores/gameStores.svelte';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { nav } from '../nav.svelte';
  import { openList } from '../overlays/overlays.svelte';
  import { openSignIn } from '../romm/rommActions';
  import TvRommMark from '../romm/TvRommMark.svelte';
  import { useScreen } from '../screen.svelte';
  import { openStorePage, startLogin } from '../stores/storeActions';
  import TvStoreMark from '../stores/TvStoreMark.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { CONNECT_LEDE, CONNECT_TITLE, STEP_2, welcomeButton, welcomeRows, type WelcomeRow } from './welcomePage';
  import { welcome } from './welcomeState.svelte';

  // The stores' statuses for the rows (shared for the app's life).
  void initGameStores();

  let rows = $derived(welcomeRows(romm.status, (s) => gameStores.status(s), welcome.imported));
  let button = $derived(welcomeButton(rows));

  const fail = (err: unknown) => void pushToast(err instanceof Error ? err.message : String(err), 'error');

  function open(r: WelcomeRow) {
    if (r.id === 'romm') {
      if (!r.connected) {
        openSignIn('connect', 'welcome');
        return;
      }
      void openList({
        title: 'RomM',
        sub: r.sub,
        items: [
          { label: 'Edit sign-in…', testId: 'tv-welcome-romm-edit', onPick: () => openSignIn('edit', 'welcome') },
          { label: 'Disconnect RomM', danger: true, testId: 'tv-welcome-romm-disconnect', onPick: () => void disconnectRomm().then((e) => e !== null && fail(e)) },
        ],
      });
      return;
    }
    const source = r.id as StoreSource;
    if (source === 'steam') {
      openStorePage('steam');
      return;
    }
    if (!r.connected) {
      startLogin(source);
      return;
    }
    void openList({
      title: storeName(source),
      sub: r.sub,
      items: [{ label: `Disconnect ${storeName(source)}`, danger: true, testId: `tv-welcome-${source}-disconnect`, onPick: () => void api.storeDisconnect(source).catch(fail) }],
    });
  }

  async function done() {
    if (welcome.busy) return;
    welcome.busy = true;
    try {
      await finishWelcome();
      if (session.error) welcome.error = session.error;
    } finally {
      welcome.busy = false;
    }
  }

  useScreen({
    hints: () => (nav.startup ? [{ button: 'B', label: 'Back' }] : []),
  });
</script>

<div class="start" data-testid="tv-welcome-stores">
  <div class="start-box wide rwel" data-nav="v">
    <div class="rstep">{STEP_2}</div>
    <h1>{CONNECT_TITLE}</h1>
    <p class="sub">{CONNECT_LEDE}</p>
    {#if welcome.error}<p class="errline" role="alert" data-testid="tv-welcome-error">{welcome.error}</p>{/if}
    <div class="start-body" data-nav="v" id="wel-stores">
      {#each rows as r (r.id)}
        <div class="srow wrow" data-k={`w-${r.id}`} data-al={r.a} data-testid={`tv-welcome-row-${r.id}`} use:tvf={{ a: () => open(r) }}>
          <span style="margin-right:.2rem">{#if r.id === 'romm'}<TvRommMark lg />{:else}<TvStoreMark source={r.id as StoreSource} lg />{/if}</span>
          <div class="lab"><div class="t">{r.name}</div><div class="s" data-testid={`tv-welcome-row-${r.id}-sub`}>{r.sub}</div></div>
          <div class="val">
            <span class="st" class:ok={r.connected} data-testid={`tv-welcome-row-${r.id}-pill`}>{r.connected ? 'Connected' : 'Not connected'}</span>
            <span class="chev"><TvIcon name="chevronRight" px={18} /></span>
          </div>
        </div>
      {/each}
    </div>
    <div class="btns start-foot" data-nav="h">
      <TvButton k="w-go" primary disabled={welcome.busy} testId="tv-welcome-go" onA={() => void done()}>{button}</TvButton>
    </div>
  </div>
</div>
