<script lang="ts">
  // The folder browser (items 349–352; tvmode.js `openBrowser`): its title,
  // the folder shown in a monospace box, then Up one folder (not at the
  // root) and the folder's folders. A opens a folder and focuses its first
  // row; Y "Use this folder" picks the folder shown and goes back. In file
  // mode (Program) there is no Y: files show without a chevron and A on one
  // picks it.
  import { api, type FolderListing } from '../../api';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { focusables } from '../focus/dom';
  import type { FolderPick } from '../gameActions';
  import { popPage, setFocus } from '../nav.svelte';
  import { useScreen } from '../screen.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { browserHints, browserRows, rowAction, type BrowserRow } from './folderBrowser';

  let { pick }: { pick: FolderPick } = $props();

  let listing = $state<FolderListing | null>(null);
  let error = $state<string | null>(null);
  let listEl = $state<HTMLElement | null>(null);
  let rows = $derived(listing ? browserRows(listing, !!pick.files) : []);

  async function go(path: string, focusFirst: boolean) {
    try {
      const next = await api.listFolder(path, !!pick.files);
      listing = next;
      error = null;
    } catch (err) {
      // A folder that can't be opened: stay where we are, say why.
      const msg = err instanceof Error ? err.message : String(err);
      if (listing) pushToast(msg, 'error');
      else if (path !== '') return go('', focusFirst);
      else error = msg;
      return;
    }
    if (focusFirst) {
      await Promise.resolve();
      requestAnimationFrame(() => {
        if (listEl) listEl.scrollTop = 0;
        const first = focusables(listEl)[0];
        if (first) setFocus(first, { instant: true });
      });
    }
  }

  // svelte-ignore state_referenced_locally
  void go(pick.start ?? '', false);

  async function choose(path: string) {
    const onPick = pick.onPick;
    await popPage();
    onPick(path);
  }

  function activate(r: BrowserRow) {
    if (r.kind === 'file') void choose(r.path);
    else void go(r.path, true);
  }

  useScreen({
    hints: () => browserHints(!!pick.files),
    get y() {
      if (pick.files || !listing) return undefined;
      const path = listing.path;
      return () => void choose(path);
    },
  });
</script>

<div class="page" data-scroll="fs" bind:this={listEl} data-testid="tv-folder-browser">
  <div class="page-in">
    <h1>{pick.title}</h1>
    {#if listing}<div class="path" data-testid="tv-fs-path">{listing.path}</div>{/if}
    {#if error}<p class="note err">{error}</p>{/if}
    <div data-nav="v" id="fs-list">
      {#each rows as r (`${listing?.path}|${r.key}`)}
        <div class="lrow" data-k={r.key} data-al={rowAction(r)} data-testid={`tv-fs-${r.key}`} use:tvf={{ a: () => activate(r) }}>
          {#if r.kind === 'up'}
            <TvIcon name="arrowLeft" px={20} /><div class="grow t">Up one folder</div>
          {:else}
            <div class="grow t">{r.name}</div>
            {#if r.kind === 'dir'}<TvIcon name="chevronRight" px={18} />{/if}
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>
