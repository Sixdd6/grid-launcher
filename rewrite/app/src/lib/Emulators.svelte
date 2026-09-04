<script lang="ts">
  // The Emulators view (design §9, D-UI-5): a 220px category rail and one
  // pane per category — Installed, Add from catalog, Platform defaults,
  // Compat tools (Linux only). All four panes stay mounted and switch with
  // `hidden`, the rule the shell applies to views: the catalog's refresh on
  // a finished install and the defaults' compatibility fetch keep running
  // whichever pane is in front. Each pane's column caps at 1100px (D-UI-7).
  import {
    api,
    type CatalogEntry,
    type EmulatorEntry,
    type LaunchDefaults,
    type Platform,
    type PlatformRef,
    type ProfileSummary,
  } from './api';
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import { downloads } from './stores/downloads.svelte';
  import { compatTools } from './stores/compatTools.svelte';
  import { filterCatalogEntries } from './emulators/catalog';
  import {
    NO_CORE_VALUE,
    NO_DEFAULT_VALUE,
    platformCoreSelect,
    platformDefaultSelect,
  } from './emulators/defaults';
  import { isWindowsHost } from './emulators/compatTools';
  import {
    emulatorPageLabel,
    emulatorRailEntries,
    formPlacement,
    pageAfterSave,
    safeEmulatorPage,
    type AddTab,
    type EmulatorPage,
    type EmulatorPageCounts,
  } from './emulators/pages';
  import CompatTools from './emulators/CompatTools.svelte';
  import EmulatorForm from './emulators/EmulatorForm.svelte';

  // Mounted for the whole session now that Emulators is a view, so the
  // refresh below is gated on being the visible view: navigating away and
  // back re-runs `list_platforms`, which is what makes a cleared default
  // survive (the emulators spec's "(none)" case).
  let { active = true }: { active?: boolean } = $props();

  // The app's own OS, not the server's platform field (isNativePlatform) —
  // gates whether the Compat tools pane (wine/proton, which Windows-only
  // content has nothing to do with) exists at all.
  const windowsHost = isWindowsHost(navigator.platform);

  let page = $state<EmulatorPage>('installed');

  /** Programmatic page selection: the Server header's default-emulator chip
   *  routes to Platform defaults (design §6). */
  export function show(next: EmulatorPage) {
    page = safeEmulatorPage(next, windowsHost);
  }

  let emulators = $state<EmulatorEntry[]>([]);
  let listLoading = $state(true);
  let listError = $state<string | null>(null);
  let deleteError = $state<string | null>(null);

  let platforms = $state<Platform[]>([]);
  let defaults = $state<LaunchDefaults | null>(null);
  let defaultsError = $state<string | null>(null);

  // The backend's `compatible_emulators` answer, keyed by platform NAME (the
  // same string `default_emulators` is keyed by). The per-platform default
  // select offers only these, so an emulator is never offered for a platform
  // its profile does not support.
  let compatible = $state<Record<string, string[]>>({});
  // Its own error slot, never `defaultsError`: this fetch re-runs on every
  // platform/emulator change and would otherwise clear a real defaults error.
  let compatibleError = $state<string | null>(null);

  // The backend's `retroarch_core_options` answer, keyed by platform NAME.
  // Fetched on the same trigger set as `compatible`, because both depend on
  // the emulator list and on which core files are on disk.
  let coreOptions = $state<Record<string, string[]>>({});

  let profiles = $state<ProfileSummary[]>([]);

  // The Installed pane's edit sheet (design §9: "Edit opens the manual form
  // inline as a sheet on the right of the pane"). `name` is the entry's
  // current name, used as saveEmulator's originalName so a rename can find
  // & replace itself; `entry` is the row being edited, kept whole so the
  // fields the form does not show are written back untouched.
  let editing = $state<{ name: string; entry: EmulatorEntry } | null>(null);
  // The catalog pane's two tabs: the catalog rows, or the manual add form.
  let addTab = $state<AddTab>('install');
  let placement = $derived(formPlacement(page, editing !== null, addTab));

  let confirmingDelete = $state<string | null>(null);
  let deletePending = $state<string | null>(null);

  // Catalog pane state.
  let catalog = $state<CatalogEntry[]>([]);
  let catalogLoading = $state(true);
  let catalogError = $state<string | null>(null);
  let catalogSearch = $state('');
  let searchEl = $state<HTMLInputElement | null>(null);
  let installingSourceIds = $state<Set<string>>(new Set());
  let filteredCatalog = $derived(filterCatalogEntries(catalogSearch, catalog));

  // Signature of every emulator-job download that has reached a terminal
  // status — read inside the effects below so a fresh terminal entry (an
  // install completing, failing, or getting cancelled) triggers a catalog
  // re-fetch. Approximate on purpose (task-7-brief.md): any terminal
  // emulator entry is enough of a signal, not just the one just installed.
  let emulatorTerminalSignature = $derived(
    downloads.entries
      .filter((e) => e.job === 'emulator' && ['completed', 'failed', 'cancelled'].includes(e.status))
      .map((e) => `${e.id}:${e.status}`)
      .join(',')
  );

  // RPCS3 PS3 firmware note/button (task-17-brief.md). Keyed by emulator
  // entry name; `null` means the status was queried and no PS3UPDAT.PUP is
  // present yet, `undefined` means it hasn't been queried yet — either way
  // the note/button stay hidden until a query resolves with a non-empty path.
  let rpcs3Status = $state<Map<string, string | null>>(new Map());
  let ps3InstallPending = $state<Set<string>>(new Set());
  let ps3Toast = $state<{ entryName: string; ok: boolean; text: string } | null>(null);

  // Re-queried whenever a `firmware`-kind drawer entry reaches 'completed'
  // (task-17-brief.md): the background firmware installer finishing means a
  // freshly-downloaded PS3UPDAT.PUP may now be sitting next to RPCS3.
  let firmwareCompletedSignature = $derived(
    downloads.entries
      .filter((e) => e.kind === 'firmware' && e.status === 'completed')
      .map((e) => `${e.id}:${e.status}`)
      .join(',')
  );

  let counts = $derived<EmulatorPageCounts>({
    installed: emulators.length,
    catalog: catalog.length,
    defaults: platforms.length,
    compat: compatTools.tools.length,
  });

  let railRows = $derived(
    emulatorRailEntries(counts, page, windowsHost).map(
      (e): RailPaneEntry<EmulatorPage> => ({
        key: e.key,
        testId: e.testId,
        countTestId: e.countTestId,
        label: e.label,
        count: e.count,
        selected: e.selected,
        heading: e.heading,
      }),
    ),
  );

  function isRpcs3(name: string): boolean {
    return name.toLowerCase().includes('rpcs3');
  }

  async function refreshRpcs3StatusFor(name: string) {
    try {
      const status = await api.rpcs3FirmwareStatus(name);
      const next = new Map(rpcs3Status);
      next.set(name, status.pup_path);
      rpcs3Status = next;
    } catch {
      // Best-effort only — leave the prior status (or none) on failure.
    }
  }

  async function refreshAllRpcs3Status() {
    await Promise.all(emulators.filter((e) => isRpcs3(e.name)).map((e) => refreshRpcs3StatusFor(e.name)));
  }

  $effect(() => {
    const signature = firmwareCompletedSignature;
    void signature;
    refreshAllRpcs3Status();
  });

  async function handleInstallPs3Firmware(name: string) {
    ps3Toast = null;
    ps3InstallPending = new Set(ps3InstallPending).add(name);
    try {
      const ok = await api.installPs3Firmware(name);
      ps3Toast = ok
        ? {
            entryName: name,
            ok: true,
            text: 'PS3 firmware installation started — follow the RPCS3 dialog to complete.',
          }
        : {
            entryName: name,
            ok: false,
            text: 'Could not launch RPCS3 to install firmware. Check the emulator path.',
          };
    } catch {
      ps3Toast = {
        entryName: name,
        ok: false,
        text: 'Could not launch RPCS3 to install firmware. Check the emulator path.',
      };
    } finally {
      const next = new Set(ps3InstallPending);
      next.delete(name);
      ps3InstallPending = next;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshEmulators();
    refreshPlatformsAndDefaults();
    refreshProfiles();
  });

  // Both inputs of the compatibility and core answers: the platforms they
  // are asked about, and the emulator list the backend draws them from.
  // Reading both here is what makes a freshly added (or installed) emulator
  // show up in the per-platform selects without a reload.
  let compatibilityInputs = $derived({
    platformRefs: platforms.map((p) => ({ name: p.name, slug: p.slug })),
    emulatorNames: emulators.map((e) => e.name).join(','),
  });

  $effect(() => {
    const { platformRefs, emulatorNames } = compatibilityInputs;
    void emulatorNames;
    refreshCompatible(platformRefs);
    refreshCoreOptions(platformRefs);
  });

  // An emulator install reaching a terminal status can have ADDED an entry,
  // so the entry list and the stored defaults are both stale (the
  // compatibility effect above then re-runs off the new emulator list).
  $effect(() => {
    const signature = emulatorTerminalSignature;
    void signature;
    // Also fires once at mount, duplicating the mount effect's two fetches —
    // cheap, and it keeps the refresh rule free of a first-run special case.
    refreshEmulators();
    refreshDefaults();
  });

  // The catalog loads when the view comes forward (its count sits on the
  // rail from the first look) and reloads whenever an emulator download
  // reaches a terminal status, so Install/Installed never goes stale.
  $effect(() => {
    const signature = emulatorTerminalSignature;
    void signature;
    if (!active) return;
    refreshCatalog();
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
      void refreshAllRpcs3Status();
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

  async function refreshCompatible(refs: PlatformRef[]) {
    if (refs.length === 0) {
      compatible = {};
      return;
    }
    try {
      compatible = await api.compatibleEmulators(refs);
      compatibleError = null;
    } catch (err) {
      compatibleError = errorMessage(err);
    }
  }

  async function refreshCoreOptions(refs: PlatformRef[]) {
    if (refs.length === 0) {
      coreOptions = {};
      return;
    }
    try {
      coreOptions = await api.retroarchCoreOptions(refs);
      compatibleError = null;
    } catch (err) {
      // Shares the compatibility error slot (design §3.4) so a core-options
      // failure cannot clear a real defaults error.
      compatibleError = errorMessage(err);
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

  /** `emulator-add`: the Add from catalog page, on its Catalog tab. */
  function openAdd() {
    page = 'catalog';
    addTab = 'install';
    catalogError = null;
    catalogSearch = '';
    confirmingDelete = null;
  }

  function openEdit(entry: EmulatorEntry) {
    page = 'installed';
    editing = { name: entry.name, entry };
    confirmingDelete = null;
  }

  function closeSheet() {
    editing = null;
  }

  async function afterEditSave() {
    closeSheet();
    await refreshEmulators();
    await refreshDefaults();
    page = pageAfterSave('edit');
  }

  async function afterAddSave() {
    addTab = 'install';
    await refreshEmulators();
    await refreshDefaults();
    page = pageAfterSave('add');
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
      if (editing?.name === name) closeSheet();
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      deleteError = errorMessage(err);
    } finally {
      deletePending = null;
      confirmingDelete = null;
    }
  }

  function selectFor(platformName: string) {
    return platformDefaultSelect(defaults, platformName, compatible[platformName] ?? []);
  }

  function coreSelectFor(platformName: string, selectedEmulator: string) {
    return platformCoreSelect(
      defaults,
      platformName,
      selectedEmulator,
      coreOptions[platformName] ?? []
    );
  }

  async function handleDefaultChange(platformName: string, value: string) {
    try {
      await api.setDefaultEmulator(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function handleCoreChange(platformName: string, value: string) {
    try {
      await api.setRetroarchCore(platformName, value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }
</script>

<section data-testid="emulators-view" class="emulators" aria-label="Emulators">
  <RailPane entries={railRows} testId="emulators-rail" ariaLabel="Emulator categories" onSelect={(k) => (page = k)} />

  <div class="panes">
    <!-- Installed -->
    <section data-testid="emu-page-installed" class="pane" hidden={page !== 'installed'} aria-label={emulatorPageLabel('installed')}>
      <div class="view-content pane-inner">
        <div class="section-header">
          <h2>{emulatorPageLabel('installed')}</h2>
          <button data-testid="emulator-add" class="add-btn" onclick={openAdd}>+ Add emulator</button>
        </div>

        <div class="installed-body" class:with-sheet={placement === 'sheet'}>
          <div class="list-column">
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
                    <li data-testid={`emulator-row-${sanitizeName(e.name)}`} class="emulator-row" class:editing={editing?.name === e.name}>
                      <div class="row-main">
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
                      </div>
                      {#if isRpcs3(e.name) && rpcs3Status.get(e.name)}
                        <div class="ps3-firmware">
                          <p data-testid={`emulator-ps3-firmware-note-${sanitizeName(e.name)}`} class="hint">
                            PS3 firmware downloaded — click Install to activate it.
                          </p>
                          <button
                            data-testid={`emulator-ps3-firmware-${sanitizeName(e.name)}`}
                            disabled={ps3InstallPending.has(e.name)}
                            onclick={() => handleInstallPs3Firmware(e.name)}
                          >
                            {ps3InstallPending.has(e.name) ? 'Installing…' : 'Install PS3 Firmware'}
                          </button>
                        </div>
                      {/if}
                      {#if ps3Toast && ps3Toast.entryName === e.name}
                        <p data-testid="emulator-ps3-firmware-toast" class={ps3Toast.ok ? 'hint' : 'error'}>
                          {ps3Toast.text}
                        </p>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </div>

          {#if placement === 'sheet' && editing}
            <aside data-testid="emu-edit-sheet" class="sheet" aria-label="Edit emulator">
              <h3>Edit emulator</h3>
              <!-- Keyed on the entry name: the form seeds its fields on
                   mount, so switching rows must remount it. -->
              {#key editing.name}
                <EmulatorForm mode="edit" entry={editing.entry} {profiles} onSaved={afterEditSave} onCancel={closeSheet} />
              {/key}
            </aside>
          {/if}
        </div>
      </div>
    </section>

    <!-- Add from catalog -->
    <section data-testid="emu-page-catalog" class="pane" hidden={page !== 'catalog'} aria-label={emulatorPageLabel('catalog')}>
      <div class="view-content pane-inner">
        <h2>{emulatorPageLabel('catalog')}</h2>

        <div class="tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={addTab === 'install'}
            class:active={addTab === 'install'}
            data-testid="emu-add-tab-install"
            onclick={() => (addTab = 'install')}
          >
            Catalog
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

        {#if placement === 'manual'}
          <div class="manual-form">
            <EmulatorForm mode="add" entry={null} {profiles} onSaved={afterAddSave} onCancel={() => (addTab = 'install')} />
          </div>
        {:else}
          <div class="catalog-tab">
            <input
              data-testid="emu-catalog-search"
              class="catalog-search"
              type="search"
              placeholder="Search emulators…"
              bind:this={searchEl}
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
        {/if}
      </div>
    </section>

    <!-- Platform defaults -->
    <section data-testid="emu-page-defaults" class="pane" hidden={page !== 'defaults'} aria-label={emulatorPageLabel('defaults')}>
      <div class="view-content pane-inner">
        <h2>{emulatorPageLabel('defaults')}</h2>
        {#if defaultsError}
          <p class="error" role="alert">{defaultsError}</p>
        {/if}
        <!-- Rendered separately from the defaults error so neither can hide the
             other: the compatibility fetch has its own failure mode. -->
        {#if compatibleError}
          <p class="error" role="alert">{compatibleError}</p>
        {/if}
        {#if platforms.length === 0}
          <p class="muted">No platforms available.</p>
        {:else}
          <ul class="defaults-list">
            {#each platforms as p (p.id)}
              {@const selectId = `default-emulator-${p.id}`}
              {@const choice = selectFor(p.name)}
              {@const coreId = `default-core-${p.id}`}
              {@const core = coreSelectFor(p.name, choice.selected)}
              <li class="defaults-card">
                <div class="defaults-card-header">
                  <label class="platform-name" for={selectId}>{p.name}</label>
                </div>
                <div class="defaults-field">
                  <span class="defaults-field-label">Emulator</span>
                  <!-- `default-select-<platformId>` is the per-platform select's
                       test id; its `id` (used by the label) is
                       `default-emulator-<platformId>`. -->
                  <select
                    data-testid={`default-select-${p.id}`}
                    id={selectId}
                    disabled={choice.disabled}
                    value={choice.selected}
                    onchange={(e) => handleDefaultChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
                  >
                    {#if choice.disabled}
                      <option value={NO_DEFAULT_VALUE}>No compatible emulator</option>
                    {:else}
                      <option value={NO_DEFAULT_VALUE}>(none)</option>
                      {#each choice.options as name (name)}
                        <option value={name}>{name}</option>
                      {/each}
                    {/if}
                  </select>
                </div>
                {#if core.visible}
                  <div class="defaults-field">
                    <label class="defaults-field-label" for={coreId}>Core</label>
                    <select
                      data-testid={`default-core-${p.id}`}
                      id={coreId}
                      disabled={core.disabled}
                      value={core.selected}
                      onchange={(e) => handleCoreChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
                    >
                      {#if core.disabled}
                        <option value={NO_CORE_VALUE}>No installed core</option>
                      {:else}
                        {#each core.options as id (id)}
                          <option value={id}>{id}</option>
                        {/each}
                      {/if}
                    </select>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </section>

    <!-- Compat tools (design §9: hidden on Windows) -->
    {#if !windowsHost}
      <section data-testid="emu-page-compat" class="pane" hidden={page !== 'compat'} aria-label={emulatorPageLabel('compat')}>
        <div class="view-content pane-inner">
          <h2>{emulatorPageLabel('compat')}</h2>
          <CompatTools />
        </div>
      </section>
    {/if}
  </div>
</section>

<style>
  .emulators {
    display: flex;
    align-items: stretch;
    height: 100%;
    min-height: 0;
  }

  .panes {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  /* No `display` on `.pane` itself: the `hidden` attribute's UA rule must
     win, and an author `display: flex` here would override it. */
  .pane {
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .pane[hidden] {
    display: none;
  }

  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 18px;
    font-weight: 600;
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 14px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .add-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .add-btn:hover {
    background: var(--surface);
  }

  /* Design §9: the edit sheet sits to the right of the list. */
  .installed-body {
    display: flex;
    align-items: flex-start;
    gap: 24px;
  }

  .list-column {
    flex: 1 1 auto;
    min-width: 0;
  }

  .sheet {
    flex: 0 0 360px;
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    background: var(--surface-2);
    animation: sheet-in var(--m-base) ease;
  }

  @keyframes sheet-in {
    from {
      opacity: 0;
      transform: translateX(16px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .emulator-list,
  .defaults-list,
  .catalog-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .emulator-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    border: 1px solid transparent;
    background: var(--surface);
    transition: border-color var(--m-fast) ease;
  }

  .emulator-row.editing {
    border-color: var(--primary);
  }

  .row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .ps3-firmware {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }

  .ps3-firmware .hint {
    flex: 1 1 auto;
    min-width: 0;
  }

  .ps3-firmware button,
  .row-actions button,
  .catalog-row button {
    flex: none;
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .ps3-firmware button:hover:not(:disabled),
  .row-actions button:hover:not(:disabled),
  .catalog-row button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .ps3-firmware button:disabled,
  .row-actions button:disabled,
  .catalog-row button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .row-actions button.confirm {
    background: var(--danger);
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
    color: var(--text-muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .args,
  .meta {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .tabs {
    display: flex;
    gap: 4px;
  }

  .tabs button {
    font: inherit;
    font-size: 13px;
    padding: 6px 12px;
    border-radius: var(--r-chip) var(--r-chip) 0 0;
    border: 1px solid var(--border);
    border-bottom: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--m-fast) ease, color var(--m-fast) ease;
  }

  .tabs button.active {
    background: var(--surface);
    color: var(--text-h);
  }

  .manual-form {
    max-width: 480px;
  }

  .catalog-tab {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .catalog-search {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .catalog-search:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .catalog-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .defaults-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .defaults-card-header {
    display: flex;
  }

  .platform-name {
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    white-space: normal;
  }

  .defaults-field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .defaults-field-label {
    color: var(--text-muted);
    font-size: 13px;
    flex-shrink: 0;
  }

  .defaults-card select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
