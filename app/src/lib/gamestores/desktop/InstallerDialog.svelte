<script lang="ts">
  // Install from installer… (GameStores README, "Offline GOG installer";
  // prototype `stPickerHTML`, `stReadingHTML`, `stFoundHTML`): the system's
  // file chooser, "Reading the installer…", then the recognised card or a
  // refusal with Choose another file…. The path field beside Choose file… is
  // the path end-to-end tests drive (every Browse in the app is additive).
  import { api, type InstallerInspection } from '../../api';
  import { pickFile } from '../../pickers';
  import { compatTools } from '../../stores/compatTools.svelte';
  import { gameStores } from '../../stores/gameStores.svelte';
  import { inspectionView, PICKER_FOOT, PICKER_TITLE, readingLine, REFUSED_HEADING, spaceNote, fileName } from '../inspection';
  import { queuedNotice, refusedNotice } from '../notices';
  import StoreCoverSm from './StoreCoverSm.svelte';
  import StoreDialog from './StoreDialog.svelte';
  import { showCard } from './ui.svelte';

  let { onClose }: { onClose: () => void } = $props();

  type Step = { kind: 'pick' } | { kind: 'reading'; path: string } | { kind: 'done'; insp: InstallerInspection };
  let step = $state<Step>({ kind: 'pick' });
  let path = $state('');
  let compat = $state(compatTools.defaultTool || 'GE-Proton10-17');
  let installDir = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  let gog = $derived(gameStores.status('gog'));
  let view = $derived(
    step.kind === 'done' ? inspectionView(step.insp, { compat, gogConnected: gog?.connection === 'connected' }) : null,
  );

  async function choose() {
    const picked = await pickFile(PICKER_TITLE, [{ name: 'GOG offline installers', extensions: ['exe', 'sh', 'pkg'] }]);
    if (picked) {
      path = picked;
      await read();
    }
  }

  // The system's chooser opens straight away (not in an E2E build: there is
  // no portal behind it, and the path field is what the specs type into).
  $effect(() => {
    if (!import.meta.env.VITE_E2E && !import.meta.env.VITE_FAKE_STORES) void choose();
  });

  async function read() {
    const p = path.trim();
    if (p === '') return;
    error = null;
    step = { kind: 'reading', path: p };
    try {
      const insp = await api.offlineInspect(p);
      const base = gog?.install_dir ?? '';
      installDir = base === '' ? '' : `${base}/${insp.title.replace(/[^A-Za-z0-9]+/g, '')}`;
      step = { kind: 'done', insp };
    } catch (err) {
      step = { kind: 'pick' };
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function another() {
    step = { kind: 'pick' };
    path = '';
    error = null;
  }

  async function changeFolder() {
    const { pickFolder } = await import('../../pickers');
    const picked = await pickFolder('Install folder');
    if (picked) installDir = picked;
  }

  async function install() {
    if (step.kind !== 'done') return;
    busy = true;
    try {
      const found = view?.kind === 'found' ? view : null;
      await api.offlineInstall(step.insp.path, found?.showCompat ? compat : '', installDir.trim() === '' || !found?.showFolder ? undefined : installDir);
      showCard(queuedNotice(step.insp.title || fileName(step.insp.path)));
      onClose();
    } catch (err) {
      showCard(refusedNotice(`Couldn’t install ${step.insp.title || fileName(step.insp.path)}`, err instanceof Error ? err.message : String(err)));
    } finally {
      busy = false;
    }
  }

  let compatChoices = $derived.by(() => {
    const names = compatTools.tools.map((t) => t.name);
    return compat !== '' && !names.includes(compat) ? [compat, ...names] : names;
  });
</script>

<StoreDialog testId="store-installer-dialog" label={PICKER_TITLE} {onClose}>
  {#if step.kind === 'pick'}
    <h3>{PICKER_TITLE}</h3>
    <p class="note">
      Pick a GOG offline installer you downloaded: <code class="path">setup_….exe</code> (Windows) or
      <code class="path">….sh</code> (Linux).
    </p>
    <div class="st-field">
      <span>Installer</span>
      <span class="inline">
        <input
          class="g-field"
          data-testid="store-installer-path"
          placeholder="/home/you/Downloads/setup_….exe"
          bind:value={path}
          onkeydown={(e) => e.key === 'Enter' && read()}
        />
        <button class="small-btn" type="button" data-testid="store-installer-choose" onclick={choose}>Choose file…</button>
      </span>
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="foot">
      <span class="note">{PICKER_FOOT}</span>
      <button class="g-btn secondary" onclick={onClose}>Cancel</button>
      <button class="g-btn" data-testid="store-installer-open" disabled={path.trim() === ''} onclick={read}>Open</button>
    </div>
  {:else if step.kind === 'reading'}
    <h3>Reading the installer…</h3>
    <div class="reading" data-testid="store-installer-reading"><span class="spin"></span>{readingLine(step.path)}</div>
  {:else if view?.kind === 'refused'}
    <h3>{REFUSED_HEADING}</h3>
    <div class="refuse" data-testid="store-installer-refused">
      <span><b>{view.title}</b><p>{view.body}</p></span>
    </div>
    <div class="foot">
      <span></span>
      <button class="g-btn secondary" data-testid="store-installer-another" onclick={another}>Choose another file…</button>
      <button class="g-btn" onclick={onClose}>OK</button>
    </div>
  {:else if view?.kind === 'found' && step.kind === 'done'}
    <h3>{view.heading}</h3>
    <div class="found" data-testid="store-installer-found">
      <StoreCoverSm cover={step.insp.cover} />
      <span>
        <b>{view.title}</b>
        <span class="note">{view.what}</span>
        <span class="note">{view.from}</span>
      </span>
    </div>
    <p class="note" data-testid="store-installer-how">{view.how}</p>
    {#if view.already}<p class="warn-line" data-testid="store-installer-already">{view.already}</p>{/if}
    {#if view.showCompat}
      <label class="st-field">
        <span>Compatibility tool</span>
        <select class="g-field" bind:value={compat}>
          {#each compatChoices as t (t)}<option value={t}>{t}</option>{/each}
        </select>
      </label>
    {/if}
    {#if view.showFolder}
      <div class="st-field">
        <span>Install folder</span>
        <span class="inline"><code>{installDir}</code><button class="small-btn" type="button" onclick={changeFolder}>Change…</button></span>
      </div>
    {/if}
    {#if view.joins}<p class="note">{view.joins}</p>{/if}
    <div class="foot">
      <span class="note">{spaceNote(step.insp.bytes, null)}</span>
      <button class="g-btn secondary" onclick={onClose}>Cancel</button>
      <button class="g-btn" data-testid="store-installer-install" disabled={busy} onclick={install}>{view.button}</button>
    </div>
  {/if}
</StoreDialog>

<style>
  .reading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 4px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .spin {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
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

  .refuse {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border-left: 3px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 8%, transparent);
  }

  .refuse b {
    color: var(--text-h);
    display: block;
    margin-bottom: 2px;
  }

  .refuse p {
    margin: 0;
    font-size: 13px;
    color: var(--text);
  }

  .found {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .found b {
    color: var(--text-h);
    font-size: 15px;
    display: block;
  }

  .found .note {
    display: block;
    font-size: 12px;
  }
</style>
