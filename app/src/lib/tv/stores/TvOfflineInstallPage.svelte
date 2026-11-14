<script lang="ts">
  // A recognised GOG offline installer (TVStores README § Offline GOG
  // installer; prototype `sPicked`): what it is, how it installs (unpacked,
  // or run silently: TV mode never opens an installer's own setup window),
  // Compatibility tool (a Windows game), Install folder (a new game), and
  // Install / Update / Install DLC, which Start presses from anywhere.
  import { api, type InstallerInspection } from '../../api';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { popPage } from '../nav.svelte';
  import { openList } from '../overlays/overlays.svelte';
  import TvRowsPage from '../pages/TvRowsPage.svelte';
  import type { PageMain, PageRow } from '../pages/pageRows';
  import { card, compatChoices, defaultCompat, fail, toDownloads } from './storeActions';
  import { folderName, joinPath, tvInspectionView } from './storeWords';
  import { installerSpaceNote } from '../../gamestores/inspection';

  let { insp }: { insp: InstallerInspection } = $props();

  // svelte-ignore state_referenced_locally
  let compat = $state(defaultCompat());
  let gog = $derived(gameStores.status('gog'));
  let view = $derived(tvInspectionView(insp, compat, gog?.connection === 'connected'));
  // svelte-ignore state_referenced_locally
  let folder = $state(joinPath(gog?.install_dir || '~/Games/GRID/stores/gog', folderName(insp.title)));
  // svelte-ignore state_referenced_locally
  let freeBytes = $state<number | null>(insp.free_bytes);
  let busy = $state(false);
  let dlc = $derived(insp.verdict.kind === 'dlc');

  let rows = $derived.by((): PageRow[] => {
    const v = view;
    if (v.kind !== 'found') return [];
    const [first, ...rest] = v.what.split(' · ');
    const R: PageRow[] = [{ kind: 'info', key: 'what', label: dlc ? first : v.title, value: dlc ? ['DLC', ...rest.slice(1)].join(' · ') : v.what }];
    R.push({ kind: 'note', key: 'how', text: v.how });
    if (v.already) R.push({ kind: 'note', key: 'already', text: v.already, warn: true });
    if (v.showCompat) {
      R.push({
        kind: 'row',
        key: 'compat',
        label: 'Compatibility tool',
        value: compat,
        run: () => void openList({ title: 'Compatibility tool', items: compatChoices().map((t) => ({ label: t, checked: compat === t, onPick: () => (compat = t) })) }),
      });
    }
    if (v.showFolder) {
      R.push({
        kind: 'row',
        key: 'folder',
        label: 'Install folder',
        value: folder,
        run: () => openFolderBrowser({ title: `Install ${v.title} in`, start: folder.replace(/[\\/][^\\/]*$/, ''), onPick: (p) => pickFolder(joinPath(p, folderName(v.title))) }),
      });
    }
    if (v.joins) R.push({ kind: 'note', key: 'joins', text: v.joins });
    return R;
  });

  /** A new install folder, and the free space on its volume for the summary. */
  function pickFolder(next: string) {
    folder = next;
    api
      .storeFreeSpace(next)
      .then((free) => {
        if (folder === next) freeBytes = free;
      })
      .catch(() => {
        if (folder === next) freeBytes = null;
      });
  }

  async function install() {
    if (busy || view.kind !== 'found') return;
    busy = true;
    try {
      await api.offlineInstall(insp.path, insp.kind === 'windows_exe' ? compat : '', view.showFolder ? folder : undefined);
    } catch (err) {
      busy = false;
      fail(err);
      return;
    }
    // The queued card is this page's: the shared notifier gives a store
    // row none, and the backend sends no card for a command's own outcome.
    card(`${insp.title || 'The game'} added to download queue`, 'success', undefined, toDownloads);
    void popPage();
  }

  let main = $derived.by((): PageMain | null => {
    if (view.kind !== 'found') return null;
    return { label: view.button, summary: installerSpaceNote(insp, freeBytes), disabled: busy, hint: 'Install', press: () => void install() };
  });
</script>

{#if view.kind === 'found'}
  <TvRowsPage title={view.heading} sub={view.from} {rows} {main} testId="tv-store-offline" />
{/if}
