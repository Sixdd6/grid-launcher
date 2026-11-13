<script lang="ts">
  // The install page (TVStores README § The game page › Install page;
  // prototype `sInstallPage`): the desktop's install dialog as a page.
  // Version (Linux recommended / Windows) as radio rows; Options:
  // Compatibility tool (the Windows version) and Language as lists, Install
  // folder through the folder browser; DLC you own as switches; the footer
  // "22.0 GB to download · 412 GB free on /home" and Install, which Start
  // presses from anywhere.
  import { api, type StoreInstallOptions } from '../../api';
  import { initialChoice, installRequest, installSizes, type InstallChoice } from '../../gamestores/install';
  import { storeName } from '../../gamestores/marks';
  import { sizeGb } from '../../gamestores/text';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { openFolderBrowser } from '../gameActions';
  import { popPage } from '../nav.svelte';
  import { openList } from '../overlays/overlays.svelte';
  import TvRowsPage from '../pages/TvRowsPage.svelte';
  import type { PageMain, PageRow } from '../pages/pageRows';
  import { card, compatChoices, fail, toDownloads } from './storeActions';
  import { COMPAT_SUB, folderName, installFolderSub, joinPath } from './storeWords';

  let { gameKey }: { gameKey: string } = $props();

  let game = $derived(gameStores.game(gameKey));
  let opts = $state<StoreInstallOptions | null>(null);
  let choice = $state<InstallChoice | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // svelte-ignore state_referenced_locally
  api
    .storeInstallOptions(gameKey)
    .then((o) => {
      opts = o;
      choice = initialChoice(o);
    })
    .catch((err) => (error = err instanceof Error ? err.message : String(err)));

  function pickCompat() {
    if (!choice) return;
    const c = choice;
    void openList({ title: 'Compatibility tool', items: compatChoices().map((t) => ({ label: t, checked: c.compatTool === t, onPick: () => (c.compatTool = t) })) });
  }

  function pickLanguage() {
    if (!choice || !opts) return;
    const c = choice;
    void openList({ title: 'Language', items: opts.languages.map((l) => ({ label: l, checked: c.language === l, onPick: () => (c.language = l) })) });
  }

  function pickFolder() {
    if (!choice || !game) return;
    const c = choice;
    const title = game.title;
    const parent = c.installDir.replace(/[\\/][^\\/]*$/, '');
    openFolderBrowser({ title: `Install ${title} in`, start: parent, onPick: (p) => (c.installDir = joinPath(p, folderName(title))) });
  }

  let rows = $derived.by((): PageRow[] => {
    const R: PageRow[] = [];
    if (!game) return R;
    if (error) return [{ kind: 'note', key: 'err', text: error, err: true }];
    if (!opts || !choice) return [{ kind: 'note', key: 'loading', text: 'Reading the game’s versions…' }];
    const c = choice;
    if (opts.versions.length > 1) {
      R.push({ kind: 'heading', key: 'h-ver', text: 'Version' });
      for (const p of ['linux', 'windows'] as const) {
        const v = opts.versions.find((x) => x.platform === p);
        if (!v) continue;
        R.push({
          kind: 'row',
          key: `ver-${p}`,
          label: p === 'linux' ? 'Linux' : 'Windows',
          sub: p === 'linux' ? `Runs natively on this computer${v.recommended ? ' · recommended' : ''}` : `Runs through ${c.compatTool}${v.recommended ? ' · recommended' : ''}`,
          radio: c.platform === p,
          run: () => (c.platform = p),
        });
      }
    }
    R.push({ kind: 'heading', key: 'h-opt', text: 'Options' });
    if (c.platform === 'windows') R.push({ kind: 'row', key: 'compat', label: 'Compatibility tool', sub: COMPAT_SUB, value: c.compatTool, run: pickCompat });
    if (opts.languages.length > 1) R.push({ kind: 'row', key: 'lang', label: 'Language', value: c.language, run: pickLanguage });
    R.push({ kind: 'row', key: 'folder', label: 'Install folder', sub: installFolderSub(game.source), value: c.installDir, run: pickFolder });
    if (opts.dlc.length) {
      R.push({ kind: 'heading', key: 'h-dlc', text: 'DLC you own' });
      for (const d of opts.dlc) {
        const short = d.title.startsWith(`${game.title} — `) ? d.title.slice(game.title.length + 3) : d.title;
        const on = c.dlcIds.includes(d.id);
        R.push({
          kind: 'row',
          key: `dlc-${d.id}`,
          label: short,
          sub: sizeGb(d.download_bytes),
          toggle: on,
          run: () => (c.dlcIds = on ? c.dlcIds.filter((x) => x !== d.id) : [...c.dlcIds, d.id]),
        });
      }
    }
    return R;
  });

  async function install() {
    if (!game || !opts || !choice || busy) return;
    busy = true;
    const req = installRequest(game, choice);
    try {
      await api.storeInstall(req);
    } catch (err) {
      busy = false;
      fail(err);
      return;
    }
    // The queued card is this page's: the shared notifier gives a store
    // row none, and the backend sends no card for a command's own outcome.
    card(`${game.title} added to download queue`, 'success', undefined, toDownloads);
    await popPage();
  }

  let main = $derived.by((): PageMain | null => {
    if (!game || !opts || !choice) return null;
    const sizes = installSizes(opts, choice);
    const volume = opts.volume || '/';
    const over = sizes.diskBytes > opts.free_bytes;
    return {
      label: over ? 'Not enough space' : 'Install',
      summary: `${sizeGb(sizes.downloadBytes)} to download · ${sizeGb(opts.free_bytes)} free on ${volume}`,
      disabled: over || busy,
      why: over ? `Needs ${sizeGb(sizes.diskBytes)} · ${sizeGb(opts.free_bytes)} free on ${volume}` : undefined,
      hint: 'Install',
      press: () => void install(),
    };
  });
</script>

{#if game}
  <TvRowsPage
    title={`Install ${game.title}`}
    sub={`${storeName(game.source)} · ${sizeGb(opts?.versions.find((v) => v.platform === choice?.platform)?.download_bytes ?? game.download_bytes)} download`}
    {rows}
    {main}
    testId="tv-store-install"
  />
{/if}
