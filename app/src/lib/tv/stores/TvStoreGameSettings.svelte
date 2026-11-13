<script lang="ts">
  // Game settings for a store game (TVStores README § The game page › Game
  // settings page; prototype `sGameSettings`): Install folder, Launch file
  // ("Set by GOG" / "Your choice"; the folder browser in file mode), Launch
  // parameters (the keyboard), Compatibility tool and Wine prefix for the
  // Windows version or "Linux games run directly.", the Cloud saves switch,
  // and Save, which Start presses from anywhere.
  import { api, type StoreGameSettings } from '../../api';
  import { hasStoreCloud } from '../../gamestores/marks';
  import { gameStores, refreshLibrary } from '../../stores/gameStores.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { popPage } from '../nav.svelte';
  import { openKeyboard, openList } from '../overlays/overlays.svelte';
  import TvRowsPage from '../pages/TvRowsPage.svelte';
  import type { PageMain, PageRow } from '../pages/pageRows';
  import { card, compatChoices, fail } from './storeActions';
  import { cloudToggleSub, launchFileSub, LINUX_RUNS_DIRECTLY } from './storeWords';
  import { fileName } from '../../gamestores/inspection';

  let { gameKey }: { gameKey: string } = $props();

  let game = $derived(gameStores.game(gameKey));
  let s = $state<StoreGameSettings | null>(null);
  let error = $state<string | null>(null);

  // svelte-ignore state_referenced_locally
  api
    .storeGameSettings(gameKey)
    .then((x) => (s = x))
    .catch((err) => (error = err instanceof Error ? err.message : String(err)));

  let windows = $derived(game?.installed_platform === 'windows');

  let rows = $derived.by((): PageRow[] => {
    if (!game) return [];
    if (error) return [{ kind: 'note', key: 'err', text: error, err: true }];
    if (!s) return [{ kind: 'note', key: 'loading', text: 'Reading the game’s settings…' }];
    const x = s;
    const g = game;
    const R: PageRow[] = [
      { kind: 'info', key: 'folder', label: 'Install folder', value: x.install_dir },
      {
        kind: 'row',
        key: 'exe',
        label: 'Launch file',
        sub: launchFileSub(g.source, x.exe_user_pinned),
        value: fileName(x.executable) || 'None',
        run: () =>
          openFolderBrowser({
            title: 'Launch file',
            start: x.install_dir,
            files: true,
            onPick: (p) => {
              x.executable = p;
              x.exe_user_pinned = true;
            },
          }),
      },
      {
        kind: 'row',
        key: 'args',
        label: 'Launch parameters',
        sub: 'Optional',
        value: x.launch_parameters || 'None',
        run: () => void openKeyboard({ title: 'Launch parameters', value: x.launch_parameters, onDone: (v) => (x.launch_parameters = v) }),
      },
    ];
    if (windows) {
      R.push({
        kind: 'row',
        key: 'compat',
        label: 'Compatibility tool',
        value: x.compat_tool,
        run: () =>
          void openList({
            title: 'Compatibility tool',
            items: compatChoices().map((t) => ({ label: t, checked: x.compat_tool === t, onPick: () => (x.compat_tool = t) })),
          }),
      });
      R.push({ kind: 'info', key: 'prefix', label: 'Wine prefix', sub: 'Read-only', value: x.wineprefix });
    } else {
      R.push({ kind: 'note', key: 'linux', text: LINUX_RUNS_DIRECTLY });
    }
    if (hasStoreCloud(g.source) && g.cloud !== 'none') {
      R.push({ kind: 'row', key: 'cloud', label: 'Cloud saves', sub: cloudToggleSub(g.source), toggle: x.cloud_saves, run: () => (x.cloud_saves = !x.cloud_saves) });
    }
    return R;
  });

  async function save() {
    if (!s || !game) return;
    const g = game;
    try {
      await api.storeSetGameSettings(gameKey, $state.snapshot(s));
    } catch (err) {
      fail(err);
      return;
    }
    card('Game settings saved');
    void refreshLibrary(g.source);
    void popPage();
  }

  let main = $derived<PageMain | null>(s ? { label: 'Save', summary: '', disabled: false, hint: 'Save', press: () => void save() } : null);
</script>

{#if game}
  <TvRowsPage title={game.title} sub="Game settings" {rows} {main} testId="tv-store-settings" />
{/if}
