<script lang="ts" module>
  // Kept for the session: the next Locate opens where the last one was.
  let lastFolder = '';
</script>

<script lang="ts">
  // The TV file browser in file mode (TVRommStorefront README, "The TV file
  // browser in file mode (Locate files)"): the folder browser's page with
  // picks. Multi mode: A opens a folder or picks / unpicks a file (a tick
  // box), picks are kept across folders, Y checks the folder shown (three
  // levels deep, links not followed), Start checks the picks. Single mode
  // (the PS3 PUP): no ticks, A chooses a file. Its own component, beside the
  // folder browser (TvFolderBrowser), which it does not change.
  import { api, type FolderListing } from '../../api';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import { focusables } from '../focus/dom';
  import { setFocus } from '../nav.svelte';
  import { useScreen } from '../screen.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { NOTHING_PICKED, pickRowAction, pickRows, pickerHints, picksLine, pickerSub, type PickRow } from './filePicker';

  let {
    title,
    emulator,
    single = false,
    onCheck,
  }: {
    title: string;
    emulator: string;
    single?: boolean;
    /** The picks (or, with `folder`, the folder shown). */
    onCheck: (paths: string[], folder: boolean) => void;
  } = $props();

  let listing = $state<FolderListing | null>(null);
  let error = $state<string | null>(null);
  let listEl = $state<HTMLElement | null>(null);
  let picked = $state<string[]>([]);
  let rows = $derived(listing ? pickRows(listing) : []);

  async function go(path: string, focusFirst: boolean) {
    try {
      listing = await api.listFolder(path, true);
      lastFolder = listing.path;
      error = null;
    } catch (err) {
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
  void go(lastFolder, false);

  function activate(r: PickRow) {
    if (r.kind !== 'file') {
      void go(r.path, true);
      return;
    }
    if (single) {
      onCheck([r.path], false);
      return;
    }
    picked = picked.includes(r.path) ? picked.filter((p) => p !== r.path) : [...picked, r.path];
  }

  useScreen({
    hints: () => pickerHints(picked.length, single),
    get y() {
      if (single || !listing) return undefined;
      const path = listing.path;
      return () => onCheck([path], true);
    },
    main: () =>
      single
        ? null
        : {
            disabled: picked.length === 0,
            why: NOTHING_PICKED,
            press: () => onCheck([...picked], false),
          },
  });
</script>

<div class="page" data-scroll="fs" bind:this={listEl} data-testid="tv-file-picker">
  <div class="page-in">
    <h1>{title}</h1>
    <p class="sub">{pickerSub(emulator, single)}</p>
    {#if listing}<div class="path" data-testid="tv-fp-path">{listing.path}</div>{/if}
    {#if !single}<p class="note" data-testid="tv-fp-picks">{picksLine(picked)}</p>{/if}
    {#if error}<p class="note err">{error}</p>{/if}
    <div data-nav="v" id="fp-list">
      {#each rows as r (`${listing?.path}|${r.key}`)}
        {@const on = r.kind === 'file' && picked.includes(r.path)}
        <div
          class="lrow"
          class:picked={on}
          data-k={r.key}
          data-al={pickRowAction(r, on, single)}
          data-testid={`tv-fp-${r.key}`}
          use:tvf={{ a: () => activate(r) }}
        >
          {#if r.kind === 'up'}
            <TvIcon name="arrowLeft" px={20} /><div class="grow t">Up one folder</div>
          {:else if r.kind === 'dir'}
            <div class="grow t">{r.name}</div>
            <TvIcon name="chevronRight" px={18} />
          {:else}
            {#if !single}
              <span class="tick" class:on data-testid={`tv-fp-tick-${r.name}`}>
                {#if on}<TvIcon name="check" px={16} />{/if}
              </span>
            {/if}
            <div class="grow t">{r.name}</div>
            {#if r.size}<span class="muted" data-testid={`tv-fp-size-${r.name}`}>{r.size}</span>{/if}
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .tick {
    flex: none;
    width: 1.3rem;
    height: 1.3rem;
    display: grid;
    place-items: center;
    border-radius: 0.3rem;
    border: 2px solid var(--text-muted);
    margin-right: 0.7rem;
  }

  .tick.on {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--on-primary);
  }
</style>
