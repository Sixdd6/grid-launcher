<script lang="ts">
  // Compatibility tools panel (task-17-brief.md): the per-user default
  // wine/proton picker, grouped by kind/source, plus a catalog Install tab
  // for managed compat tools (GE-Proton and similar). Takes no props — it
  // reads the compatTools store directly and loads the install catalog
  // itself on mount, same shape as the Emulators install tab it sits beside.
  import { api, type CatalogEntry } from '../api';
  import { compatTools, refresh as refreshCompatTools } from '../stores/compatTools.svelte';
  import { downloads } from '../stores/downloads.svelte';
  import {
    compatToolLabel,
    compatToolTerminalSignature,
    groupCompatTools,
    liveCompatToolSourceIds,
    umuActionLabel,
    umuInstallBusy,
    umuStatusLabel,
  } from './compatTools';

  let catalog = $state<CatalogEntry[]>([]);
  let catalogLoading = $state(true);
  let catalogError = $state<string | null>(null);
  let installingSourceIds = $state<Set<string>>(new Set());

  let umuError = $state<string | null>(null);
  let umuRequesting = $state(false);

  let defaultError = $state<string | null>(null);
  let defaultPending = $state(false);

  let groups = $derived(groupCompatTools(compatTools.tools));
  // Flattened in the same grouped order the groups render in — `compat-default-<index>`
  // is this array's index, per task-17-brief.md's ambiguity resolution.
  let flattened = $derived(groups.flatMap((g) => g.tools));

  // Fix round 1: the catalog must re-fetch whenever a compat_tool install
  // reaches a terminal state (mirrors Emulators.svelte's own
  // emulatorTerminalSignature for its emulator catalog tab), and a row's
  // Install button must stay disabled for the whole background install, not
  // just while the initial `installCompatTool` call is in flight.
  let compatTerminalSignature = $derived(compatToolTerminalSignature(downloads.entries));
  let liveSourceIds = $derived(liveCompatToolSourceIds(downloads.entries));
  // The umu-run line's button stays busy for the whole background install
  // (its Downloads row is live), like a catalog row's.
  let umuRun = $derived(compatTools.umuRun);
  let umuAction = $derived(umuRun ? umuActionLabel(umuRun) : null);
  let umuBusy = $derived(umuRequesting || (umuRun !== null && umuInstallBusy(downloads.entries, umuRun.source_id)));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshCatalog() {
    catalogLoading = true;
    try {
      catalog = await api.listCompatToolCatalog();
      catalogError = null;
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      catalogLoading = false;
    }
  }

  // Runs once at mount (signature starts as '') and again whenever it
  // changes — i.e. whenever a compat_tool drawer entry reaches a terminal
  // status — so the Install/Installed buttons never go stale mid-install.
  $effect(() => {
    const signature = compatTerminalSignature;
    void signature;
    refreshCatalog();
  });

  function testKeyFor(sourceId: string): string {
    return sourceId.replaceAll('/', '-');
  }

  async function handleDefaultChange(path: string) {
    defaultError = null;
    defaultPending = true;
    try {
      await api.setDefaultCompatTool(path);
      await refreshCompatTools();
    } catch (err) {
      defaultError = errorMessage(err);
    } finally {
      defaultPending = false;
    }
  }

  async function handleUmuInstall() {
    umuError = null;
    umuRequesting = true;
    try {
      await api.installUmuLauncher();
    } catch (err) {
      umuError = errorMessage(err);
    } finally {
      umuRequesting = false;
    }
  }

  async function handleInstallClick(sourceId: string) {
    catalogError = null;
    installingSourceIds = new Set(installingSourceIds).add(sourceId);
    try {
      await api.installCompatTool(sourceId);
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      const next = new Set(installingSourceIds);
      next.delete(sourceId);
      installingSourceIds = next;
    }
  }
</script>

<section class="compat-section" data-testid="compat-tools-section">
  <h3>Compatibility tools</h3>

  {#if defaultError}<p data-testid="compat-error" class="error" role="alert">{defaultError}</p>{/if}

  {#if umuRun}
    <div class="umu">
      <div class="umu-row">
        <span data-testid="compat-umu-status" class="umu-status">{umuStatusLabel(umuRun)}</span>
        {#if umuAction}
          <button data-testid="compat-umu-install" disabled={umuBusy} onclick={handleUmuInstall}>
            {umuBusy ? 'Installing…' : umuAction}
          </button>
        {/if}
      </div>
      <p class="muted">
        Proton games run through umu-run. The first Proton launch downloads the Steam Linux Runtime (about
        200 MB), so it takes longer than later ones.
      </p>
      {#if umuError}<p data-testid="compat-umu-error" class="error" role="alert">{umuError}</p>{/if}
    </div>
  {/if}

  {#if flattened.length === 0}
    <p data-testid="compat-empty" class="muted">No compatibility tools installed</p>
  {:else}
    {#each groups as group (group.title)}
      <div class="compat-group">
        <h4>{group.title}</h4>
        <ul class="compat-list">
          {#each group.tools as tool (tool.path)}
            {@const index = flattened.indexOf(tool)}
            <li class="compat-row">
              <label>
                <input
                  type="radio"
                  name="compat-default"
                  data-testid={`compat-default-${index}`}
                  checked={tool.path === compatTools.defaultTool}
                  disabled={defaultPending}
                  onchange={() => handleDefaultChange(tool.path)}
                />
                <span>{compatToolLabel(tool)}</span>
              </label>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  {/if}

  <div class="compat-catalog">
    <h4>Install</h4>
    {#if catalogError}<p class="error" role="alert">{catalogError}</p>{/if}
    {#if catalogLoading}
      <p class="muted">Loading…</p>
    {:else if catalog.length === 0}
      <p class="muted">No compatibility tools found.</p>
    {:else}
      <ul class="catalog-list">
        {#each catalog as entry (entry.source_id)}
          {@const testKey = testKeyFor(entry.source_id)}
          {@const busy = installingSourceIds.has(entry.source_id) || liveSourceIds.has(entry.source_id)}
          <li class="catalog-row">
            <div class="row-text">
              <span class="name">{entry.name}</span>
              <span class="meta">{entry.provider} • {entry.tag}</span>
            </div>
            {#if entry.installed}
              <button data-testid={`compat-catalog-installed-${testKey}`} disabled>Installed</button>
            {:else}
              <button
                data-testid={`compat-catalog-install-${testKey}`}
                disabled={busy}
                onclick={() => handleInstallClick(entry.source_id)}
              >
                {busy ? 'Installing…' : 'Install'}
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .compat-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 14px;
  }

  h4 {
    margin: 0;
    color: var(--text-h);
    font-size: 12px;
  }

  .muted {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .umu {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .umu-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--border);
  }

  .umu-status {
    color: var(--text-h);
    font-size: 13px;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .compat-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .compat-list,
  .catalog-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .compat-row label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-h);
    cursor: pointer;
  }

  .compat-row input[type='radio'] {
    flex: none;
  }

  .compat-catalog {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 8px;
  }

  .catalog-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--border);
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    color: var(--text-h);
    font-weight: 500;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    color: var(--text);
    font-size: 12px;
  }

  .umu-row button,
  .catalog-row button {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
    white-space: nowrap;
  }

  .umu-row button:disabled,
  .catalog-row button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
