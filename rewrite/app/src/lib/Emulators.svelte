<script lang="ts">
  import {
    api,
    type CatalogEntry,
    type EmulatorEntry,
    type LaunchDefaults,
    type Platform,
    type ProfileSummary,
    type RaFanOutRow,
    type RaStatus,
  } from './api';
  import { downloads } from './stores/downloads.svelte';
  import {
    filterCatalogEntries,
    matchProfileByName,
    shouldAutoFillFromName,
  } from './emulators/catalog';
  import { NO_DEFAULT_VALUE, resolveDefaultEmulatorValue } from './emulators/defaults';
  import { canSubmit, fanOutSummary, statusLabel } from './emulators/retroachievements';

  let { onClose }: { onClose: () => void } = $props();

  let emulators = $state<EmulatorEntry[]>([]);
  let listLoading = $state(true);
  let listError = $state<string | null>(null);
  let deleteError = $state<string | null>(null);

  let platforms = $state<Platform[]>([]);
  let defaults = $state<LaunchDefaults | null>(null);
  let defaultsError = $state<string | null>(null);

  let profiles = $state<ProfileSummary[]>([]);

  // Tagged rather than a bare string sentinel: a string-based 'new' marker
  // would make an emulator literally named "new" impossible to edit (its
  // name would collide with the add-mode sentinel and saveEmulator's
  // originalName arg would come out blank, so the save gets rejected as a
  // duplicate against itself). `name` is the entry's current name, used as
  // saveEmulator's originalName so a rename can find & replace itself.
  // `entry` is the row being edited, kept whole so the fields the form does
  // not show (install provenance, autoconfig save/ignore paths) are written
  // back untouched instead of being dropped on save.
  type Editing = { mode: 'add' } | { mode: 'edit'; name: string; entry: EmulatorEntry } | null;
  let editing = $state<Editing>(null);
  // Only meaningful while editing.mode === 'add' — edit mode always shows
  // the manual form directly (there is no "install this again" flow for an
  // already-configured entry).
  let addTab = $state<'install' | 'manual'>('install');
  let formName = $state('');
  let formPath = $state('');
  let formArgs = $state('');
  let formError = $state<string | null>(null);
  let formPending = $state(false);
  let autofillMatch = $state<ProfileSummary | null>(null);

  let confirmingDelete = $state<string | null>(null);
  let deletePending = $state<string | null>(null);

  // RetroAchievements block state (task-12-brief.md).
  let raStatus = $state<RaStatus | null>(null);
  let raUsername = $state('');
  // The token field is write-only: it starts empty on every mount and is
  // never bound to a value read back from the backend, which never returns
  // the token in the first place (RaStatus carries only `token_present`).
  let raToken = $state('');
  let raError = $state<string | null>(null);
  let raResultLine = $state<string | null>(null);
  let raSavePending = $state(false);
  let raClearPending = $state(false);

  // Install tab state.
  let catalog = $state<CatalogEntry[]>([]);
  let catalogLoading = $state(true);
  let catalogError = $state<string | null>(null);
  let catalogSearch = $state('');
  let installingSourceIds = $state<Set<string>>(new Set());
  let filteredCatalog = $derived(filterCatalogEntries(catalogSearch, catalog));

  // Signature of every emulator-job download that has reached a terminal
  // status — read inside the effect below so a fresh terminal entry (an
  // install completing, failing, or getting cancelled) triggers a catalog
  // re-fetch. Approximate on purpose (task-7-brief.md): any terminal
  // emulator entry is enough of a signal, not just the one just installed.
  let emulatorTerminalSignature = $derived(
    downloads.entries
      .filter((e) => e.job === 'emulator' && ['completed', 'failed', 'cancelled'].includes(e.status))
      .map((e) => `${e.id}:${e.status}`)
      .join(',')
  );

  let panelEl = $state<HTMLElement | null>(null);

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    refreshEmulators();
    refreshPlatformsAndDefaults();
    refreshProfiles();
    refreshRaStatus();
  });

  // Loads (or reloads) the catalog whenever the Install tab becomes the
  // visible tab, and again whenever an emulator download reaches a terminal
  // status while it is visible.
  $effect(() => {
    const signature = emulatorTerminalSignature;
    void signature;
    if (editing?.mode === 'add' && addTab === 'install') {
      refreshCatalog();
    }
  });

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  function sanitizeName(name: string): string {
    return name.toLowerCase().replace(/\s+/g, '-');
  }

  async function refreshEmulators() {
    listLoading = true;
    try {
      emulators = await api.listEmulators();
      listError = null;
    } catch (err) {
      listError = errorMessage(err);
    } finally {
      listLoading = false;
    }
  }

  async function refreshPlatformsAndDefaults() {
    try {
      const [p, d] = await Promise.all([api.listPlatforms(), api.getLaunchDefaults()]);
      platforms = p;
      defaults = d;
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshDefaults() {
    try {
      defaults = await api.getLaunchDefaults();
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshProfiles() {
    try {
      profiles = await api.listProfiles();
    } catch {
      // Best-effort only — both auto-fills just won't find a match.
    }
  }

  async function refreshCatalog() {
    catalogLoading = true;
    try {
      catalog = await api.listEmulatorCatalog();
      catalogError = null;
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      catalogLoading = false;
    }
  }

  function openAdd() {
    editing = { mode: 'add' };
    addTab = 'install';
    formName = '';
    formPath = '';
    formArgs = '';
    formError = null;
    catalogError = null;
    catalogSearch = '';
    autofillMatch = null;
    confirmingDelete = null;
  }

  function openEdit(entry: EmulatorEntry) {
    editing = { mode: 'edit', name: entry.name, entry };
    formName = entry.name;
    formPath = entry.path;
    formArgs = entry.args;
    formError = null;
    autofillMatch = null;
    confirmingDelete = null;
  }

  function closeForm() {
    editing = null;
    formError = null;
    autofillMatch = null;
  }

  async function handleInstallClick(sourceId: string) {
    catalogError = null;
    installingSourceIds = new Set(installingSourceIds).add(sourceId);
    try {
      await api.installEmulator(sourceId);
    } catch (err) {
      catalogError = errorMessage(err);
    } finally {
      const next = new Set(installingSourceIds);
      next.delete(sourceId);
      installingSourceIds = next;
    }
  }

  function testKeyFor(sourceId: string): string {
    return sourceId.replaceAll('/', '-');
  }

  async function autoFillFromPath() {
    if (formName.trim() !== '' || formArgs.trim() !== '') return;
    const path = formPath.trim();
    if (!path) return;
    try {
      const profile = await api.matchProfile(path);
      if (profile) {
        formName = profile.name;
        formArgs = profile.args;
      }
    } catch {
      // Best-effort autofill only — leave the form as typed on failure.
    }
  }

  // Manual-add auto-fill from the typed NAME (task-7-brief.md): add mode
  // only, and only when path and args are both still empty, so it never
  // clobbers a manually typed or path-derived value and never touches an
  // entry being edited. Fires on blur/change of the name field, which the
  // edit form shares.
  function autoFillFromName() {
    if (!shouldAutoFillFromName(editing?.mode ?? null, formPath, formArgs)) {
      autofillMatch = null;
      return;
    }
    const match = matchProfileByName(formName, profiles);
    autofillMatch = match;
    if (match) {
      formArgs = match.args;
    }
  }

  function onPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      autoFillFromPath();
    }
  }

  async function saveForm() {
    if (editing === null) return;
    const originalName = editing.mode === 'add' ? '' : editing.name;
    // Backend stores the name as-given; trim client-side so a name typed
    // with stray whitespace doesn't get persisted verbatim.
    const entry: EmulatorEntry = {
      ...(editing.mode === 'edit' ? editing.entry : {}),
      name: formName.trim(),
      path: formPath,
      args: formArgs,
    };
    formError = null;
    formPending = true;
    try {
      await api.saveEmulator(originalName, entry);
      closeForm();
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      formError = errorMessage(err);
    } finally {
      formPending = false;
    }
  }

  async function handleDeleteClick(name: string) {
    if (confirmingDelete !== name) {
      confirmingDelete = name;
      deleteError = null;
      return;
    }
    deleteError = null;
    deletePending = name;
    try {
      await api.deleteEmulator(name);
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      deleteError = errorMessage(err);
    } finally {
      deletePending = null;
      confirmingDelete = null;
    }
  }

  function defaultFor(platformName: string): string {
    return resolveDefaultEmulatorValue(defaults, platformName, emulators);
  }

  async function handleDefaultChange(platformName: string, value: string) {
    try {
      await api.setDefaultEmulator(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshRaStatus() {
    try {
      raStatus = await api.getRetroachievementsStatus();
      raUsername = raStatus.username;
    } catch (err) {
      raError = errorMessage(err);
    }
  }

  async function handleRaSave() {
    if (!canSubmit(raUsername, raToken)) return;
    raError = null;
    raResultLine = null;
    raSavePending = true;
    try {
      const rows: RaFanOutRow[] = await api.setRetroachievementsCredentials(raUsername, raToken);
      raToken = '';
      raResultLine = fanOutSummary(rows);
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raSavePending = false;
    }
  }

  async function handleRaClear() {
    raError = null;
    raResultLine = null;
    raClearPending = true;
    try {
      await api.clearRetroachievementsCredentials();
      raToken = '';
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raClearPending = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="emulators-panel"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label="Emulators"
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="emulators-close" class="close" onclick={onClose} aria-label="Close">×</button>
    <h2>Emulators</h2>

    <section class="list-section">
      <div class="section-header">
        <h3>Installed emulators</h3>
        <button data-testid="emulator-add" class="add-btn" onclick={openAdd}>+ Add emulator</button>
      </div>

      {#if listLoading}
        <p class="muted">Loading…</p>
      {:else if listError}
        <p class="error" role="alert">{listError}</p>
      {:else}
        {#if deleteError}
          <p class="error" role="alert">{deleteError}</p>
        {/if}
        {#if emulators.length === 0}
          <p class="muted">No emulators configured.</p>
        {:else}
          <ul class="emulator-list">
            {#each emulators as e (e.name)}
              <li data-testid={`emulator-row-${sanitizeName(e.name)}`} class="emulator-row">
                <div class="row-text">
                  <span class="name">{e.name}</span>
                  <span class="path" title={e.path}>{e.path}</span>
                  {#if e.args}<span class="args">{e.args}</span>{/if}
                </div>
                <div class="row-actions">
                  <button data-testid={`emulator-edit-${sanitizeName(e.name)}`} onclick={() => openEdit(e)}>Edit</button>
                  <button
                    data-testid={`emulator-delete-${sanitizeName(e.name)}`}
                    class:confirm={confirmingDelete === e.name}
                    disabled={deletePending === e.name}
                    onclick={() => handleDeleteClick(e.name)}
                  >
                    {confirmingDelete === e.name ? 'Confirm delete' : 'Delete'}
                  </button>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    </section>

    {#if editing !== null}
      <section class="form-section">
        <h3>{editing.mode === 'add' ? 'Add emulator' : 'Edit emulator'}</h3>

        {#if editing.mode === 'add'}
          <div class="tabs" role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={addTab === 'install'}
              class:active={addTab === 'install'}
              data-testid="emu-add-tab-install"
              onclick={() => (addTab = 'install')}
            >
              Install
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={addTab === 'manual'}
              class:active={addTab === 'manual'}
              data-testid="emu-add-tab-manual"
              onclick={() => (addTab = 'manual')}
            >
              Manual
            </button>
          </div>
        {/if}

        {#if editing.mode === 'add' && addTab === 'install'}
          <div class="catalog-tab">
            <input
              data-testid="emu-catalog-search"
              class="catalog-search"
              type="search"
              placeholder="Search emulators…"
              bind:value={catalogSearch}
              aria-label="Search emulators"
            />
            {#if catalogError}<p class="error" role="alert">{catalogError}</p>{/if}
            {#if catalogLoading}
              <p class="muted">Loading…</p>
            {:else if filteredCatalog.length === 0}
              <p class="muted">No emulators found.</p>
            {:else}
              <ul class="catalog-list">
                {#each filteredCatalog as entry (entry.source_id)}
                  {@const testKey = testKeyFor(entry.source_id)}
                  <li class="catalog-row">
                    <div class="row-text">
                      <span class="name">{entry.name}</span>
                      <span class="meta">{entry.provider} • {entry.tag}</span>
                    </div>
                    {#if entry.installed}
                      <button data-testid={`emu-catalog-installed-${testKey}`} disabled>Installed</button>
                    {:else}
                      <button
                        data-testid={`emu-catalog-install-${testKey}`}
                        disabled={installingSourceIds.has(entry.source_id)}
                        onclick={() => handleInstallClick(entry.source_id)}
                      >
                        {installingSourceIds.has(entry.source_id) ? 'Installing…' : 'Install'}
                      </button>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {:else}
          <form
            onsubmit={(e) => {
              e.preventDefault();
              saveForm();
            }}
          >
            <label>
              Name
              <input
                data-testid="emu-form-name"
                bind:value={formName}
                onblur={autoFillFromName}
                oninput={autoFillFromName}
                required
              />
            </label>
            {#if autofillMatch}
              <p data-testid="emu-autofill-hint" class="hint">Matched profile: {autofillMatch.name}</p>
            {/if}
            <label>
              Executable path
              <input data-testid="emu-form-path" bind:value={formPath} onblur={autoFillFromPath} onkeydown={onPathKeydown} />
            </label>
            <label>Arguments <input data-testid="emu-form-args" bind:value={formArgs} /></label>
            {#if formError}<p data-testid="emu-form-error" class="error" role="alert">{formError}</p>{/if}
            <div class="form-actions">
              <button data-testid="emu-form-save" type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
              <button data-testid="emu-form-cancel" type="button" onclick={closeForm} disabled={formPending}>Cancel</button>
            </div>
          </form>
        {/if}
      </section>
    {/if}

    <section class="defaults-section">
      <h3>Per-platform defaults</h3>
      {#if defaultsError}
        <p class="error" role="alert">{defaultsError}</p>
      {/if}
      {#if platforms.length === 0}
        <p class="muted">No platforms available.</p>
      {:else}
        <ul class="defaults-list">
          {#each platforms as p (p.id)}
            {@const selectId = `default-emulator-${p.id}`}
            <li class="defaults-row">
              <label class="platform-name" for={selectId}>{p.name}</label>
              <select
                data-testid={`default-select-${p.id}`}
                id={selectId}
                value={defaultFor(p.name)}
                onchange={(e) => handleDefaultChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
              >
                <option value={NO_DEFAULT_VALUE}>(none)</option>
                {#each emulators as em (em.name)}
                  <option value={em.name}>{em.name}</option>
                {/each}
              </select>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="ra-section">
      <h3>RetroAchievements</h3>
      <p class="muted" data-testid="ra-status">{statusLabel(raStatus)}</p>
      <form
        onsubmit={(e) => {
          e.preventDefault();
          handleRaSave();
        }}
      >
        <label>
          Username
          <input data-testid="ra-username" bind:value={raUsername} autocomplete="username" />
        </label>
        <label>
          Token
          <input
            data-testid="ra-token"
            type="password"
            bind:value={raToken}
            autocomplete="new-password"
          />
        </label>
        {#if raError}<p data-testid="ra-error" class="error" role="alert">{raError}</p>{/if}
        {#if raResultLine}<p class="hint">{raResultLine}</p>{/if}
        <div class="form-actions">
          <button
            data-testid="ra-save"
            type="submit"
            disabled={raSavePending || !canSubmit(raUsername, raToken)}
          >
            {raSavePending ? 'Saving…' : 'Save'}
          </button>
          <button
            data-testid="ra-clear"
            type="button"
            onclick={handleRaClear}
            disabled={raClearPending}
          >
            {raClearPending ? 'Clearing…' : 'Clear'}
          </button>
        </div>
      </form>
    </section>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(560px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    overflow-x: hidden;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .close:hover,
  .close:focus-visible {
    background: var(--border);
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 18px;
    padding-right: 28px;
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 14px;
  }

  .muted {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 13px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .list-section,
  .form-section,
  .defaults-section,
  .ra-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .ra-section form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .ra-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  .ra-section input {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .ra-section input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .list-section {
    border-top: none;
    padding-top: 0;
  }

  .add-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .add-btn:hover {
    background: var(--border);
  }

  .emulator-list,
  .defaults-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .emulator-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 8px;
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

  .path {
    color: var(--text);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .args {
    color: var(--text);
    font-size: 11px;
    opacity: 0.8;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .row-actions button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .row-actions button.confirm {
    background: #e5484d;
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .form-section form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .form-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  .form-section input {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .form-section input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text);
    opacity: 0.8;
  }

  .tabs {
    display: flex;
    gap: 4px;
  }

  .tabs button {
    font: inherit;
    font-size: 13px;
    padding: 6px 12px;
    border-radius: 6px 6px 0 0;
    border: 1px solid var(--border);
    border-bottom: none;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .tabs button.active {
    background: var(--border);
    color: var(--text-h);
  }

  .catalog-tab {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .catalog-search {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .catalog-search:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .catalog-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 260px;
    overflow-y: auto;
  }

  .catalog-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--border);
  }

  .catalog-row .meta {
    color: var(--text);
    font-size: 12px;
  }

  .catalog-row button {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .catalog-row button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .form-actions button[type='button'] {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .defaults-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .platform-name {
    color: var(--text-h);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .defaults-row select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
    max-width: 200px;
  }
</style>
