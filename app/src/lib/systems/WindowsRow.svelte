<script lang="ts">
  // Settings › Systems › Windows (Linux only, always shown there): a system
  // row whose picker is the default Proton version, grouped Installed
  // (system Wine, Steam's Proton, GRID's downloads) and Download (the
  // catalog, GE-Proton first and recommended). A Download pick installs it
  // and makes it the default when it finishes. Opening it shows the umu-run
  // line and the first-launch note.
  import { api, type CatalogEntry } from '../api';
  import { compatTools, refresh as refreshCompatTools } from '../stores/compatTools.svelte';
  import { downloads } from '../stores/downloads.svelte';
  import { pushToast } from '../stores/toasts.svelte';
  import {
    compatToolTerminalSignature,
    umuActionLabel,
    umuInstallBusy,
    umuStatusLabel,
  } from '../emulators/compatTools';
  import SystemRow from './SystemRow.svelte';
  import { protonInstalling, protonInstallSource, protonPicker, protonStatus } from './proton';

  let { active = true, gameCount = null }: { active?: boolean; gameCount?: number | null } = $props();

  let catalog = $state<CatalogEntry[]>([]);
  let open = $state(false);
  let pending = $state(false);
  let umuRequesting = $state(false);

  let picker = $derived(protonPicker(compatTools.tools, catalog, compatTools.defaultTool));
  let installing = $derived(protonInstalling(downloads.entries, catalog));
  let status = $derived(protonStatus(picker.unset, installing));
  let umuRun = $derived(compatTools.umuRun);
  let umuAction = $derived(umuRun ? umuActionLabel(umuRun) : null);
  let umuBusy = $derived(umuRequesting || (umuRun !== null && umuInstallBusy(downloads.entries, umuRun.source_id)));
  let terminalSignature = $derived(compatToolTerminalSignature(downloads.entries));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  // The catalog's Installed flags change when a compat-tool install ends.
  $effect(() => {
    void terminalSignature;
    if (!active) return;
    api
      .listCompatToolCatalog()
      .then((entries) => (catalog = entries))
      .catch(() => {
        // Best effort: the Download group just stays empty.
      });
  });

  async function onPick(value: string, select: HTMLSelectElement) {
    const source = protonInstallSource(value);
    pending = true;
    try {
      if (source === null) {
        await api.setDefaultCompatTool(value);
        await refreshCompatTools();
        const tool = picker.installed.find((o) => o.value === value);
        pushToast(`Windows games now use ${tool?.label ?? value}`);
      } else {
        const entry = catalog.find((e) => e.source_id === source);
        await api.installCompatToolAsDefault(source);
        pushToast(`Installing ${entry?.name ?? source}… Windows games use it once it is installed.`);
        await refreshCompatTools();
      }
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      pending = false;
      // An install pick is not the default yet: show the real one.
      select.value = picker.selected;
    }
  }

  async function installUmu() {
    umuRequesting = true;
    try {
      await api.installUmuLauncher();
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      umuRequesting = false;
    }
  }
</script>

<SystemRow
  testKey="windows"
  label="Windows"
  meta={gameCount === null ? 'runs through Proton' : `${gameCount === 1 ? '1 game' : `${gameCount} games`} · runs through Proton`}
  {open}
  pickerLabel="Proton version for Windows games"
  selected={picker.selected}
  placeholder={picker.unset ? 'Choose a Proton version' : null}
  installed={picker.installed}
  download={picker.download}
  pickerDisabled={pending || installing}
  {status}
  onToggle={() => (open = !open)}
  {onPick}
>
  {#snippet body()}
    <p class="note">Windows games run through Proton on Linux. Most people never need to change this.</p>
    {#if umuRun}
      <div class="check-row" data-testid="sys-umu">
        <span data-testid="compat-umu-status-systems">Proton launcher · {umuStatusLabel(umuRun)}</span>
        {#if umuAction}
          <button class="small-btn" data-testid="sys-umu-install" disabled={umuBusy} onclick={installUmu}>
            {umuBusy ? 'Installing…' : umuAction}
          </button>
        {/if}
      </div>
    {/if}
    <p class="note small">
      The first Windows game you launch downloads the Steam Linux Runtime (about 200 MB), so it takes longer than
      later ones.
    </p>
  {/snippet}
</SystemRow>

<style>
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .note.small {
    font-size: 12px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text);
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }

  .check-row span {
    flex: 1 1 auto;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
