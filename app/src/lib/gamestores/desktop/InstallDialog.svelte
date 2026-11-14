<script lang="ts">
  // The store install dialog (GameStores README, "Install dialog"; prototype
  // `stInstallHTML`). Words come from `gamestores/install.ts`.
  import { api, type StoreGame, type StoreInstallOptions } from '../../api';
  import { pickFolder } from '../../pickers';
  import { compatTools } from '../../stores/compatTools.svelte';
  import { COMPAT_HELP, folderHelp, initialChoice, installDialogView, installRequest, optionsForStore, type InstallChoice } from '../install';
  import { storeName } from '../marks';
  import { dlcShortTitle, sizeGb } from '../text';
  import StoreCoverSm from './StoreCoverSm.svelte';
  import StoreDialog from './StoreDialog.svelte';
  import StoreMark from './StoreMark.svelte';
  import { queueInstall } from './ui.svelte';

  let { game, onClose }: { game: StoreGame; onClose: () => void } = $props();

  let opts = $state<StoreInstallOptions | null>(null);
  let choice = $state<InstallChoice | null>(null);
  let loadError = $state<string | null>(null);
  let busy = $state(false);

  $effect(() => {
    const key = game.key;
    let cancelled = false;
    api
      .storeInstallOptions(key)
      .then((o) => {
        if (cancelled) return;
        const shown = optionsForStore(game.source, o);
        opts = shown;
        choice = initialChoice(shown);
      })
      .catch((err) => {
        if (!cancelled) loadError = err instanceof Error ? err.message : String(err);
      });
    return () => {
      cancelled = true;
    };
  });

  let name = $derived(storeName(game.source));
  let view = $derived(opts && choice ? installDialogView(game, name, opts, choice) : null);
  let compatChoices = $derived.by(() => {
    const names = compatTools.tools.map((t) => t.name);
    const current = choice?.compatTool ?? '';
    return current !== '' && !names.includes(current) ? [current, ...names] : names;
  });

  async function changeFolder() {
    const picked = await pickFolder(`Install folder for ${game.title}`);
    if (picked && choice) choice = { ...choice, installDir: picked };
  }

  function toggleDlc(id: string, on: boolean) {
    if (!choice) return;
    choice = { ...choice, dlcIds: on ? [...choice.dlcIds, id] : choice.dlcIds.filter((x) => x !== id) };
  }

  async function install() {
    if (!choice || !view || view.tooBig) return;
    busy = true;
    const req = installRequest(game, choice);
    const ok = await queueInstall(game, () => api.storeInstall(req));
    busy = false;
    if (ok) onClose();
  }
</script>

<StoreDialog testId="store-install-dialog" label={`Install ${game.title}`} {onClose}>
  <div class="st-dlg-head">
    <StoreCoverSm cover={game.cover} />
    <span>
      <h3>Install {game.title}</h3>
      <span class="note" data-testid="store-install-sub"><StoreMark source={game.source} />{view ? view.sub : `${name} · ${sizeGb(game.download_bytes)} download`}</span>
    </span>
  </div>

  {#if loadError}
    <p class="error" role="alert">{loadError}</p>
  {:else if !view || !choice || !opts}
    <p class="note">Asking {name} for the install options…</p>
  {:else}
    {#if view.versions.length > 0}
      <div class="sub">
        <h4>Version</h4>
        <div class="opts" role="radiogroup" aria-label="Version">
          {#each view.versions as v (v.platform)}
            <label class="opt" class:on={choice.platform === v.platform} data-testid={`store-install-version-${v.platform}`}>
              <input
                type="radio"
                name="st-os"
                value={v.platform}
                checked={choice.platform === v.platform}
                onchange={() => choice && (choice = { ...choice, platform: v.platform })}
              />
              <span class="opt-main"><b>{v.label}</b><span class="note">{v.note}</span></span>
              {#if v.platform === 'linux'}<span class="opt-free">Recommended</span>{/if}
            </label>
          {/each}
        </div>
      </div>
    {/if}

    <div class="sub">
      <h4>Options</h4>
      {#if view.showCompat}
        <label class="st-field">
          <span>Compatibility tool</span>
          <select
            class="g-field"
            data-testid="store-install-compat"
            value={choice.compatTool}
            onchange={(e) => choice && (choice = { ...choice, compatTool: (e.currentTarget as HTMLSelectElement).value })}
          >
            {#each compatChoices as t (t)}<option value={t}>{t}</option>{/each}
          </select>
        </label>
        <p class="help">{COMPAT_HELP}</p>
      {/if}
      {#if view.showLanguage}
        <label class="st-field">
          <span>Language</span>
          <select
            class="g-field"
            data-testid="store-install-language"
            value={choice.language}
            onchange={(e) => choice && (choice = { ...choice, language: (e.currentTarget as HTMLSelectElement).value })}
          >
            {#each opts.languages as l (l)}<option value={l}>{l}</option>{/each}
          </select>
        </label>
      {/if}
      <div class="st-field">
        <span>Install folder</span>
        <span class="inline">
          <code data-testid="store-install-folder">{choice.installDir}</code>
          <button class="small-btn" type="button" onclick={changeFolder}>Change…</button>
        </span>
      </div>
      <p class="help">{folderHelp(name)}</p>
    </div>

    {#if opts.dlc.length > 0}
      <div class="sub">
        <h4>DLC you own</h4>
        {#each opts.dlc as d (d.id)}
          <label class="check" data-testid={`store-install-dlc-${d.id}`}>
            <input type="checkbox" checked={choice.dlcIds.includes(d.id)} onchange={(e) => toggleDlc(d.id, (e.currentTarget as HTMLInputElement).checked)} />
            <span><b>{dlcShortTitle(d.title, game.title)}</b><span class="note">{sizeGb(d.download_bytes)}</span></span>
          </label>
        {/each}
      </div>
    {/if}

    <p class="note" class:space-warn={view.tooBig} data-testid="store-install-space">{view.spaceLine}</p>
  {/if}

  <div class="foot">
    <span class="note" data-testid="store-install-download">{view ? view.footNote : ''}</span>
    <button class="g-btn secondary" onclick={onClose}>Cancel</button>
    <button class="g-btn" data-testid="store-install-go" disabled={!view || view.tooBig || busy} onclick={install}>
      {view ? view.button : 'Install'}
    </button>
  </div>
</StoreDialog>
