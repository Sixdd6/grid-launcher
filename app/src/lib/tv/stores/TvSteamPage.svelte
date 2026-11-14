<script lang="ts">
  // Steam's page in Settings › Stores (TVStores README; prototype
  // `sSteamPage`): Steam client, Account (the sheet of the accounts found in
  // Steam's files, plus "Enter a SteamID64…" through the keyboard), Web API
  // key (the masked keyboard), Test, then Show in Library and Disconnect…
  // (confirmed) once connected. Save / Connect Steam is the main button
  // (Start). Test results are cards, in the desktop's words.
  //
  // The key never sits in a Svelte store or `$state`: the masked keyboard
  // hands it to a plain variable here, which goes once to `steamTest` /
  // `steamSave` and is cleared after a Save. The row shows dots only.
  import { untrack } from 'svelte';
  import { api, type SteamTestResult } from '../../api';
  import {
    ASKING_STEAM,
    accountNoteTv,
    ENTER_STEAM_ID,
    initialManual,
    initialPick,
    KEY_TV_SUB,
    keySaved,
    pickedSteamId,
    SAVED_KEY_TV,
    saveLabel,
    STEAM_DISCONNECT,
    STEAM_ID_PLACEHOLDER,
    STEAM_TV_LEDE,
    steamCard,
    steamClientPlace,
    testCardTv,
  } from '../../gamestores/steam';
  import { gameStores, refreshLibrary } from '../../stores/gameStores.svelte';
  import { popPage } from '../nav.svelte';
  import { confirmBox, openKeyboard, openList } from '../overlays/overlays.svelte';
  import type { PageRow } from '../pages/pageRows';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvPageRows from '../ui/TvPageRows.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { card, fail, showStoreInLibrary } from './storeActions';
  import { disconnectedCard } from './storeWords';

  let status = $derived(gameStores.status('steam'));

  // The form: which account, the manual SteamID64, and whether a new key was entered.
  let pick = $state(untrack(() => (status ? initialPick(status) : 'manual')));
  let manual = $state(untrack(() => (status ? initialManual(status) : '')));
  let keyEntered = $state(false);
  let busy = $state(false);
  /** The key typed on the masked keyboard: a plain variable, never reactive. */
  let pendingKey = '';

  function forgetKey(): void {
    pendingKey = '';
    keyEntered = false;
  }

  function chooseAccount(): void {
    const card0 = steamCard(status);
    if (!card0) return;
    void openList({
      title: 'Steam account',
      items: [
        ...card0.accounts.map((a) => ({
          label: `${a.persona} · ${a.steam_id}`,
          sub: accountNoteTv(a.most_recent),
          checked: pick === a.steam_id,
          testId: `tv-steam-acct-${a.steam_id}`,
          onPick: () => (pick = a.steam_id),
        })),
        {
          label: `${ENTER_STEAM_ID}…`,
          sub: STEAM_ID_PLACEHOLDER,
          checked: pick === 'manual',
          testId: 'tv-steam-acct-manual',
          onPick: () =>
            void openKeyboard({
              title: 'SteamID64',
              value: manual,
              onDone: (v) => {
                pick = 'manual';
                manual = v.trim();
              },
            }),
        },
      ],
    });
  }

  function enterKey(): void {
    void openKeyboard({
      title: 'Steam Web API key',
      mask: true,
      onDone: (v) => {
        pendingKey = v.trim();
        keyEntered = pendingKey !== '';
      },
    });
  }

  async function ask(save: boolean): Promise<void> {
    if (busy) return;
    const id = pickedSteamId(pick, manual);
    const key = pendingKey === '' ? undefined : pendingKey;
    busy = true;
    card(ASKING_STEAM);
    let r: SteamTestResult;
    try {
      r = save ? await api.steamSave(id, key) : await api.steamTest(id, key);
    } catch (err) {
      fail(err);
      return;
    } finally {
      busy = false;
    }
    const saved = save && (r.kind === 'ok' || r.kind === 'private');
    // A Save that connected gets the backend's "Steam connected" card; every other answer is shown here.
    if (!saved) {
      const c = testCardTv(r);
      card(c.text, c.error ? 'error' : 'success', c.sub);
    } else {
      forgetKey();
      void refreshLibrary('steam');
    }
  }

  function disconnect(): void {
    void confirmBox({
      title: STEAM_DISCONNECT.bold,
      body: STEAM_DISCONNECT.rest,
      ok: 'Disconnect',
      danger: true,
      onOk: () =>
        void api.storeDisconnect('steam').then(() => {
          forgetKey();
          const c = disconnectedCard('steam');
          card(c.text, 'success', c.sub);
          void popPage();
        }, fail),
    });
  }

  let rows = $derived.by((): PageRow[] => {
    const st = status;
    const c = steamCard(st);
    if (!st || !c) return [];
    const place = steamClientPlace(st);
    const acct = c.accounts.find((a) => a.steam_id === pick);
    const R: PageRow[] = [
      place !== null
        ? { kind: 'info', key: 'client', label: 'Steam client', value: `Found · ${place}` }
        : { kind: 'info', key: 'client', label: 'Steam client', sub: 'Install Steam in desktop mode, sign in once, then come back', value: 'Not installed' },
      {
        kind: 'row',
        key: 'account',
        label: 'Account',
        sub: 'Found in Steam’s files on this computer',
        value: acct ? `${acct.persona} · ${acct.steam_id}` : manual.trim() || ENTER_STEAM_ID,
        run: chooseAccount,
      },
      {
        kind: 'row',
        key: 'key',
        label: 'Web API key',
        sub: KEY_TV_SUB,
        value: keyEntered || keySaved(st) ? SAVED_KEY_TV : 'Not set',
        run: enterKey,
      },
      { kind: 'row', key: 'test', label: 'Test', disabled: busy, why: busy ? ASKING_STEAM : undefined, run: () => void ask(false) },
    ];
    if (st.connection !== 'not_connected') {
      R.push({ kind: 'row', key: 'library', label: 'Show in Library', run: () => showStoreInLibrary('steam') });
      R.push({ kind: 'row', key: 'disconnect', label: 'Disconnect…', danger: true, run: disconnect });
    }
    return R;
  });

  let mainLabel = $derived(status ? saveLabel(status) : 'Connect Steam');

  useScreen({
    hints: () => [{ button: 'MENU', label: 'Save' }],
    main: () => ({ disabled: busy, press: () => void ask(true) }),
  });
</script>

<div class="page" data-scroll="page" data-testid="tv-steam-page">
  <div class="page-in" data-nav="v" id="rows-page">
    <h1 data-testid="tv-rows-title"><TvStoreMark source="steam" lg /> Steam</h1>
    <p class="sub" data-testid="tv-rows-sub">{STEAM_TV_LEDE}</p>
    <TvPageRows {rows} />
    <div class="bk-foot">
      <span class="muted"></span>
      <TvButton k="main" primary glyph="MENU" disabled={busy} testId="tv-rows-main" onA={() => void ask(true)}>{mainLabel}</TvButton>
    </div>
  </div>
</div>
