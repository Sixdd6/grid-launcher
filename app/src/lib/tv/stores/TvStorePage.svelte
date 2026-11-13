<script lang="ts">
  // A store's page in Settings › Stores (TVStores README; prototype
  // `sStorePage`): Games you own, Store tool (or a warning row: Install /
  // Try again), License (the source link opens in desktop mode), Install
  // folder (the folder browser), the Cloud saves switch, Show in Library,
  // Install from installer… (GOG), Refresh games and Disconnect…
  // (confirmed). Login expired adds the warning note and Sign in again… as
  // the main button (Start); not connected says what connecting does, with
  // Connect GOG… (Start), and GOG keeps Install from installer….
  import { api, type StoreSource } from '../../api';
  import { hasStoreCloud, storeName, storeTool } from '../../gamestores/marks';
  import { expiredWarning, licenseLine } from '../../gamestores/settings';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { popPage } from '../nav.svelte';
  import { confirmBox } from '../overlays/overlays.svelte';
  import type { PageRow } from '../pages/pageRows';
  import { useScreen } from '../screen.svelte';
  import TvButton from '../ui/TvButton.svelte';
  import TvPageRows from '../ui/TvPageRows.svelte';
  import TvStoreMark from './TvStoreMark.svelte';
  import { card, fail, installTool, openInstaller, refreshStore, showStoreInLibrary, startLogin } from './storeActions';
  import { disconnectWords, notConnectedNote, ownedValue, toolRow } from './storeWords';
  import { CLOUD_SWITCH_TEXT } from '../../gamestores/settings';

  let { source }: { source: StoreSource } = $props();

  let status = $derived(gameStores.status(source));
  let name = $derived(storeName(source));
  let nowSeconds = $derived.by(() => {
    void status;
    return Math.floor(Date.now() / 1000);
  });

  const installerRow = (): PageRow => ({ kind: 'row', key: 'installer', label: 'Install from installer…', sub: 'A setup_….exe or ….sh you downloaded', run: () => void openInstaller() });

  let rows = $derived.by((): PageRow[] => {
    const st = status;
    if (!st) return [];
    const R: PageRow[] = [];
    if (st.connection === 'not_connected') {
      R.push({ kind: 'note', key: 'off', text: notConnectedNote(st) });
      if (source === 'gog') R.push(installerRow());
      return R;
    }
    if (st.connection === 'expired') {
      const w = expiredWarning(st);
      R.push({ kind: 'note', key: 'expired', text: `${w.bold} ${w.rest}`, warn: true });
    }
    R.push({ kind: 'info', key: 'owned', label: 'Games you own', value: ownedValue(st, nowSeconds) });
    const t = toolRow(st);
    if (t.info) R.push({ kind: 'info', key: 'tool', label: 'Store tool', value: t.value });
    else
      R.push({
        kind: 'row',
        key: 'tool',
        label: t.label,
        sub: t.sub,
        value: t.value,
        valueWarn: true,
        warn: true,
        disabled: t.busy,
        why: t.busy ? `Installing ${storeTool(source)?.name ?? 'the tool'}…` : undefined,
        run: () => installTool(source),
      });
    const lic = licenseLine(st);
    R.push({ kind: 'info', key: 'license', label: 'License', sub: `${lic.label} (opens in desktop mode)`, value: lic.text });
    R.push({
      kind: 'row',
      key: 'folder',
      label: 'Install folder',
      sub: `New ${name} installs go here`,
      value: st.install_dir,
      run: () =>
        openFolderBrowser({
          title: `Folder for new ${name} games`,
          start: st.install_dir.replace(/[\\/][^\\/]*$/, ''),
          onPick: (p) =>
            void api.storeSetInstallDir(source, p).then(() => card(`New ${name} games install in ${p}`), fail),
        }),
    });
    if (hasStoreCloud(source)) {
      R.push({
        kind: 'row',
        key: 'cloud',
        label: 'Cloud saves',
        sub: CLOUD_SWITCH_TEXT,
        toggle: st.cloud_saves,
        run: () => {
          const on = !st.cloud_saves;
          void api.storeSetCloudSaves(source, on).then(() => card(on ? `Cloud saves on for ${name} games` : `Cloud saves off for ${name} games`), fail);
        },
      });
    }
    R.push({ kind: 'row', key: 'library', label: 'Show in Library', run: () => showStoreInLibrary(source) });
    if (source === 'gog') R.push(installerRow());
    R.push({ kind: 'row', key: 'refresh', label: 'Refresh games', run: () => refreshStore(source) });
    R.push({
      kind: 'row',
      key: 'disconnect',
      label: 'Disconnect…',
      danger: true,
      run: () => void confirmBox({ ...disconnectWords(st), danger: true, onOk: () => void api.storeDisconnect(source).then(() => void popPage(), fail) }),
    });
    return R;
  });

  let main = $derived.by(() => {
    const st = status;
    if (!st || st.connection === 'connected') return null;
    return { label: st.connection === 'not_connected' ? `Connect ${name}…` : 'Sign in again…', press: () => startLogin(source) };
  });

  useScreen({
    hints: () => (main ? [{ button: 'MENU', label: 'Sign in' }] : []),
    main: () => (main ? { disabled: false, press: main.press } : null),
  });
</script>

<div class="page" data-scroll="page" data-testid="tv-store-page">
  <div class="page-in" data-nav="v" id="rows-page">
    <h1 data-testid="tv-rows-title"><TvStoreMark {source} lg /> {name}</h1>
    {#if status}
      <p class="sub" data-testid="tv-rows-sub">{#if status.connection === 'not_connected'}Not signed in{:else}Signed in as <b>{status.username}</b>{/if}</p>
    {/if}
    <TvPageRows {rows} />
    {#if main}
      <div class="bk-foot">
        <span class="muted"></span>
        <TvButton k="main" primary glyph="MENU" testId="tv-rows-main" onA={main.press}>{main.label}</TvButton>
      </div>
    {/if}
  </div>
</div>
