<script lang="ts" module>
  // Kept for the app's life: the next Locate starts where the last one was.
  let lastFolder = '';
</script>

<script lang="ts">
  // Locate files (RommStorefront README, "Locate files dialog", "Checking",
  // "Report"; prototype `rmPickHTML` / `rmReportHTML`): GRID's own chooser —
  // a folder's files with ticks (one radio for the PS3 PUP), Use the whole
  // folder, Open — then the check, then the report. The folder field is
  // additive to Browse…, as every Browse in the app: it is the path the
  // end-to-end suite drives (design decision 10: a folder is the chooser's
  // second button, since a desktop file dialog can't pick files and a
  // folder at once).
  import { api, type FolderListing } from '../api';
  import StoreDialog from '../gamestores/desktop/StoreDialog.svelte';
  import Icon from '../Icon.svelte';
  import { pickFolder } from '../pickers';
  import { childPath } from '../tv/pages/folderBrowser';
  import { locateLede, locateTitle } from './firmwareBlock';
  import { checkingLine, checkingTitle, reportView, type LocateAnswer, type LocateMode } from './locate';

  let {
    system,
    emulator,
    pup,
    onCheck,
    onClose,
  }: {
    system: string;
    emulator: string;
    pup: boolean;
    /** Runs the check: the picked paths (or the folder), answers the report. */
    onCheck: (paths: string[], mode: LocateMode) => Promise<LocateAnswer>;
    onClose: () => void;
  } = $props();

  type Phase =
    | { kind: 'pick' }
    | { kind: 'checking'; count: number }
    | { kind: 'report'; mode: LocateMode; folder: string; picked: number; answer: LocateAnswer };

  let phase = $state<Phase>({ kind: 'pick' });
  let path = $state(lastFolder);
  let listing = $state<FolderListing | null>(null);
  let error = $state<string | null>(null);
  let picked = $state<Record<string, true>>({});
  let pickedCount = $derived(Object.keys(picked).length);

  async function go(next: string) {
    try {
      listing = await api.listFolder(next, true);
      path = listing.path;
      lastFolder = listing.path;
      error = null;
      picked = {};
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  // svelte-ignore state_referenced_locally
  void go(path);

  async function browse() {
    const folder = await pickFolder(locateTitle(system, pup));
    if (folder !== null) await go(folder);
  }

  function toggle(file: string) {
    if (pup) {
      picked = picked[file] ? {} : { [file]: true };
      return;
    }
    const { [file]: was, ...rest } = picked;
    picked = was ? rest : { ...picked, [file]: true };
  }

  async function check(mode: LocateMode) {
    if (!listing) return;
    const folder = listing.path;
    const paths = mode === 'folder' ? [folder] : Object.keys(picked).map((name) => childPath(folder, name));
    const count = mode === 'folder' ? listing.entries.filter((e) => !e.is_dir).length : paths.length;
    phase = { kind: 'checking', count };
    const answer = await onCheck(paths, mode);
    phase = { kind: 'report', mode, folder, picked: paths.length, answer };
  }

  function again() {
    picked = {};
    phase = { kind: 'pick' };
    void go(path);
  }

  let view = $derived(
    phase.kind === 'report'
      ? reportView({ system, emulator, mode: phase.mode, folder: phase.folder, picked: phase.picked, answer: phase.answer })
      : null,
  );
</script>

<StoreDialog testId="locate-dialog" label={locateTitle(system, pup)} {onClose} width={560}>
  {#if phase.kind === 'pick'}
    <h3>{locateTitle(system, pup)}</h3>
    <p class="note">{locateLede(emulator, pup)}</p>
    <div class="path-row">
      <span class="ico"><Icon name="drive" size={13} /></span>
      <input
        class="g-field path"
        data-testid="locate-path"
        aria-label="Folder"
        bind:value={path}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void go(path);
          }
        }}
        onchange={() => void go(path)}
      />
      <button class="small-btn" data-testid="locate-browse" onclick={() => void browse()}>Browse…</button>
    </div>
    {#if error}<p class="error" data-testid="locate-error">{error}</p>{/if}
    {#if listing}
      <ul class="files" data-testid="locate-files">
        {#if listing.parent !== null}
          {@const up = listing.parent}
          <li>
            <button class="row" data-testid="locate-up" onclick={() => void go(up)}>
              <Icon name="arrowLeft" size={14} /><span>Up one folder</span>
            </button>
          </li>
        {/if}
        {#each listing.entries as e (e.name)}
          <li>
            {#if e.is_dir}
              {@const dir = childPath(listing.path, e.name)}
              <button class="row" data-testid={`locate-dir-${e.name}`} onclick={() => void go(dir)}>
                <span>{e.name}/</span><span class="chev"><Icon name="chevronRight" size={14} /></span>
              </button>
            {:else}
              <label class="row">
                <input
                  type={pup ? 'radio' : 'checkbox'}
                  name="locate-pick"
                  data-testid={`locate-file-${e.name}`}
                  checked={picked[e.name] === true}
                  onchange={() => toggle(e.name)}
                />
                <span>{e.name}</span>
              </label>
            {/if}
          </li>
        {:else}
          <li class="empty note">This folder is empty.</li>
        {/each}
      </ul>
    {/if}
    <div class="foot">
      {#if pup}
        <span></span>
      {:else}
        <button class="g-btn secondary" data-testid="locate-folder" disabled={!listing} onclick={() => void check('folder')}>
          Use the whole folder
        </button>
      {/if}
      <span class="spacer"></span>
      <button class="g-btn secondary" data-testid="locate-cancel" onclick={onClose}>Cancel</button>
      <button
        class="g-btn"
        data-testid="locate-open"
        disabled={pickedCount === 0}
        onclick={() => void check(pup ? 'pup' : 'files')}
      >
        Open
      </button>
    </div>
  {:else if phase.kind === 'checking'}
    <h3 data-testid="locate-checking">{checkingTitle(phase.count)}</h3>
    <div class="reading"><span class="spin"></span>{checkingLine(emulator)}</div>
  {:else if view}
    <h3 data-testid="locate-report-title">{view.title}</h3>
    <p class="note" data-testid="locate-report-lede">{view.lede}</p>
    {#each view.sections as sec (sec.kind)}
      <div class={`rep ${sec.kind}`} data-testid={`locate-report-${sec.kind}`}>
        <h4>{sec.title}</h4>
        <ul>
          {#each sec.items as item, i (`${item.file}|${i}`)}
            <li data-testid={`locate-report-item-${sec.kind}`}>
              <span
                ><code>{item.file}</code>{#if item.from}&nbsp;from <code>{item.from}</code>{/if}{#if item.as}&nbsp;→ <code
                    >{item.as}</code
                  >{/if}</span
              >
              <span class="note">{item.note}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
    {#if view.unmatched}<p class="note" data-testid="locate-report-unmatched">{view.unmatched}</p>{/if}
    <div class="foot">
      <span class="spacer"></span>
      {#if view.nothing}
        <button class="g-btn secondary" data-testid="locate-again" onclick={again}>Choose other files…</button>
      {/if}
      <button class="g-btn" data-testid="locate-done" onclick={onClose}>Done</button>
    </div>
  {/if}
</StoreDialog>

<style>
  .path-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ico {
    display: inline-flex;
    color: var(--text-muted);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
    font: 12px ui-monospace, monospace;
  }

  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-row);
    background: var(--surface);
    max-height: 300px;
    overflow-y: auto;
  }

  .files li + li {
    border-top: 1px solid var(--line);
  }

  .row {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-h);
  }

  .row:hover,
  .row:focus-visible {
    background: var(--selected);
  }

  .row input {
    margin: 0;
    accent-color: var(--primary);
  }

  .chev {
    margin-left: auto;
    display: inline-flex;
    color: var(--text-muted);
  }

  .empty {
    padding: 10px 12px;
  }

  .spacer {
    flex: 1 1 auto;
  }

  .reading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 4px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--line);
    border-top-color: var(--primary);
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }

  .rep {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rep ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rep li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px;
    border-radius: var(--r-control);
    background: var(--surface);
    border-left: 3px solid var(--primary);
    font-size: 13px;
    color: var(--text-h);
  }

  .rep.nv li {
    border-left-color: var(--warning);
  }

  .rep.bad li {
    border-left-color: var(--danger);
  }

  code {
    font: 12px ui-monospace, monospace;
    overflow-wrap: anywhere;
  }
</style>
