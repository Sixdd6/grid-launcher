<script lang="ts">
  // Settings › Systems (ProposedApp README, "One row per system on the
  // server" to "Add an emulator manually"): every emulator choice, install,
  // update and the Windows (Proton) setup in one page. One row per server
  // system with an emulator picker that is also the installer and a status;
  // systems with no emulator come first, open. The data and every command
  // are `systemsStore.svelte.ts`; `emulators/*.ts` holds the shared emulator
  // rules; the row model is `rows.ts`. This file holds the layout, which
  // rows are open, the ⋯ menu and the settings sheet.
  import { tick, untrack } from 'svelte';
  import { type EmulatorEntry } from '../api';
  import { NO_CORE_VALUE, platformCoreSelect } from '../emulators/defaults';
  import { isWindowsHost } from '../emulators/compatTools';
  import { dynamicEmulatorNotes, emulatorNotes } from '../emulators/notes';
  import { firmwareAction } from '../emulators/firmware';
  import { emulatorUpdates } from '../stores/emulatorUpdates.svelte';
  import EmulatorForm from '../emulators/EmulatorForm.svelte';
  import Icon from '../Icon.svelte';
  import SystemRow from './SystemRow.svelte';
  import EmulatorBlock from './EmulatorBlock.svelte';
  import RowMenu from './RowMenu.svelte';
  import WindowsRow from './WindowsRow.svelte';
  import { ps3FirmwareView } from './ps3Firmware';
  import { SystemsStore } from './systemsStore.svelte';
  import {
    alsoPlays,
    alsoPlaysText,
    canPlay,
    companionParent,
    companionsOf,
    gamesText,
    installRecommendedLabel,
    missingCompanionText,
    missingText,
    otherEmulatorNote,
    pickerGroups,
    shortName,
    testKeyOf,
    versionText,
    type Companion,
    type SystemRow as Row,
  } from './rows';

  let { active = true }: { active?: boolean } = $props();

  const windowsHost = isWindowsHost(navigator.platform);

  // --- Data --------------------------------------------------------------
  // The aliases keep the markup reading as before.
  const s = new SystemsStore(() => active);
  let defaults = $derived(s.defaults);
  let companions = $derived(s.companions);
  let coreOptions = $derived(s.coreOptions);
  let profiles = $derived(s.profiles);
  let facts = $derived(s.facts);
  let rpcs3Firmware = $derived(s.rpcs3Firmware);
  let loaded = $derived(s.loaded);
  let loadError = $derived(s.loadError);
  let rows = $derived(s.rows);
  let live = $derived(s.live);
  let installingRow = $derived(s.installingRow);
  let missing = $derived(s.missing);
  let others = $derived(s.others);
  let windowsGames = $derived(s.windowsGames);
  let firmwarePending = $derived(s.firmwarePending);
  let busyRows = $derived(s.busyRows);
  let launching = $derived(s.launching);
  let updating = $derived(s.updating);
  let companionBusy = $derived(s.companionBusy);
  let checking = $derived(s.checking);
  let removing = $derived(s.removing);

  // Comes forward, or an emulator install reached its end: everything the
  // rows read may have changed (an entry added, a default applied).
  $effect(() => {
    void s.installSignature;
    if (!active) return;
    untrack(() => {
      void s.refreshAll();
    });
  });

  // The once-per-session update check, the first time the page has its
  // emulator list. Never re-run by a scroll or a re-render.
  $effect(() => {
    if (!active || !loaded) return;
    untrack(() => {
      void s.checkOnce();
    });
  });

  $effect(() => {
    if (!active) return;
    untrack(() => s.loadProfiles());
  });

  // The compatibility and core answers depend on the platforms asked about
  // and on the emulator list.
  $effect(() => {
    void s.compatibilityKey;
    s.loadCompatibility(untrack(() => s.platforms.map((p) => ({ name: p.name, slug: p.slug }))));
  });

  // The Eden notes' file probes and the RPCS3 firmware check, re-read when
  // the entry list changes, when the page is shown again, and when a
  // firmware download completes (the background PS3 firmware fetch may just
  // have put a PS3UPDAT.PUP in place).
  $effect(() => {
    void s.emulatorNames;
    void s.firmwareCompleted;
    if (!active) return;
    s.loadFacts(untrack(() => s.emulators.map((e) => e.name)));
  });

  function ps3Installed(name: string): boolean {
    return s.ps3Installed(name);
  }

  function installPs3Firmware(name: string) {
    return s.installPs3Firmware(name);
  }

  // Setup's defaults land after each picked emulator's install.
  $effect(() => s.watchSetupDefaults());

  // --- Firmware ----------------------------------------------------------
  $effect(() => {
    void s.firmwareSignature;
    if (!active) return;
    for (const req of untrack(() => s.firmwareRequests)) s.refreshFirmwareStatus(req);
  });

  $effect(() => s.watchFirmwarePasses());

  function firmwareLine(row: Row) {
    return s.firmwareLine(row);
  }

  function installFirmware(row: Row) {
    s.installFirmware(row);
  }

  // --- Rows --------------------------------------------------------------
  let openRows = $state<Record<string, boolean>>({});

  function isOpen(row: Row): boolean {
    return row.emulator === '' || openRows[row.key] === true;
  }

  // The Server header's chips: open this page on one system's row, opened
  // and scrolled into view. Held until the rows have loaded.
  let revealKey = $state<string | null>(null);
  let listEl = $state<HTMLElement | null>(null);

  /** Opens `system`'s row (its display label, or its server name) and
   *  scrolls it into view once the page has its rows. */
  export function reveal(system: string) {
    revealKey = system;
  }

  $effect(() => {
    const want = revealKey;
    if (want === null || !active || !loaded) return;
    const row = rows.find((r) => r.key === want || r.name === want);
    if (!row) return;
    untrack(() => {
      revealKey = null;
      openRows = { ...openRows, [row.key]: true };
      const key = testKeyOf(row.slug || row.label);
      void tick().then(() => {
        listEl?.querySelector(`[data-testid="sys-row-${key}"]`)?.scrollIntoView({ block: 'start' });
      });
    });
  });

  function rowStatus(row: Row) {
    return s.rowStatus(row);
  }

  function installRecommended(row: Row) {
    void s.installRecommended(row);
  }

  function installAllRecommended() {
    void s.installAllRecommended();
  }

  async function onPick(row: Row, value: string, select: HTMLSelectElement) {
    const result = await s.pick(row, value);
    if (result.opened) openRows = { ...openRows, [row.key]: true };
    select.value = result.selected;
  }

  function onCoreChange(row: Row, core: string) {
    return s.changeCore(row, core);
  }

  // --- The emulator line --------------------------------------------------
  function entryFor(name: string): EmulatorEntry | undefined {
    return s.entryFor(name);
  }

  function openEmulator(name: string) {
    return s.openEmulator(name);
  }

  function updateEmulator(name: string) {
    return s.updateEmulator(name);
  }

  function installCompanion(c: Companion) {
    return s.installCompanion(c);
  }

  function checkForUpdates() {
    return s.checkForUpdates();
  }

  // --- The ⋯ menu ----------------------------------------------------------
  let menu = $state<{ where: string; name: string; anchor: HTMLElement } | null>(null);

  function toggleMenu(where: string, name: string, anchor: HTMLElement) {
    menu = menu?.where === where ? null : { where, name, anchor };
  }

  async function showFolder(name: string) {
    menu = null;
    await s.showFolder(name);
  }

  async function removeEmulator(name: string) {
    if (!(await s.removeEmulator(name))) return;
    if (sheet?.mode === 'edit' && sheet.entry.name === name) sheet = null;
    menu = null;
    await s.refreshAll();
  }

  // --- The settings sheet ---------------------------------------------------
  type Sheet = { mode: 'add' } | { mode: 'edit'; entry: EmulatorEntry };
  let sheet = $state<Sheet | null>(null);

  function editEmulator(name: string) {
    menu = null;
    const entry = entryFor(name);
    if (entry) sheet = { mode: 'edit', entry };
  }

  async function afterSave() {
    sheet = null;
    // A manual add sets no system's emulator (only an install or the user's
    // own pick does); the lists just learn about it.
    await s.refreshAll();
  }
</script>

{#snippet emulatorLine(name: string, where: string, exceptKey: string | null)}
  {@const check = emulatorUpdates.check(name)}
  {@const elsewhere = exceptKey === null ? [] : alsoPlays(name, rows, exceptKey)}
  <EmulatorBlock
    {name}
    testKey={testKeyOf(where)}
    version={versionText(entryFor(name), check)}
    updateTo={check && emulatorUpdates.hasUpdate(name) ? check.available_display : null}
    note={alsoPlaysText(elsewhere)}
    updating={updating === name}
    launching={launching === name}
    menuOpen={menu?.where === where}
    onUpdate={() => updateEmulator(name)}
    onOpen={() => openEmulator(name)}
    onMore={(anchor) => toggleMenu(where, name, anchor)}
  />
{/snippet}

{#snippet companionLines(row: Row)}
  {#each companionsOf(row.emulator, companions) as c (c.profile)}
    {@const where = `companion:${row.key}:${c.profile}`}
    {@const ckey = testKeyOf(`companion-${row.key}-${c.profile}`)}
    <div class="companion" data-testid={`sys-companion-${ckey}`}>
      {#if c.entry !== null}
        {@const name = c.entry}
        {@const check = emulatorUpdates.check(name)}
        <EmulatorBlock
          {name}
          testKey={ckey}
          kicker="Companion"
          version={versionText(entryFor(name), check)}
          updateTo={check && emulatorUpdates.hasUpdate(name) ? check.available_display : null}
          updating={updating === name}
          launching={launching === name}
          menuOpen={menu?.where === where}
          onUpdate={() => updateEmulator(name)}
          onOpen={() => openEmulator(name)}
          onMore={(anchor) => toggleMenu(where, name, anchor)}
        />
      {:else}
        {@const busy = c.sourceId !== null && (companionBusy[c.sourceId] === true || live.has(c.sourceId))}
        <div class="fix sub">
          <span data-testid={`sys-companion-missing-${ckey}`}>
            <span class="kicker">Companion</span>
            {busy ? `Installing ${c.profile}…` : missingCompanionText(c, row.emulator)}
          </span>
          {#if c.sourceId !== null}
            <button
              class="small-btn primary"
              data-testid={`sys-companion-install-${ckey}`}
              disabled={busy}
              onclick={() => void installCompanion(c)}
            >
              {busy ? 'Installing…' : 'Install'}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
{/snippet}

{#snippet tips(name: string, key: string)}
  {@const notes = [...emulatorNotes(name), ...dynamicEmulatorNotes(name, facts.get(name))]}
  {#if notes.length > 0}
    <details class="tips" data-testid={`sys-tips-${key}`}>
      <summary>Setup tips</summary>
      <ul>
        {#each notes as note (note.key)}
          <li>{note.text}</li>
        {/each}
      </ul>
    </details>
  {/if}
{/snippet}

{#snippet ps3Firmware(name: string, key: string)}
  {@const view = ps3FirmwareView(rpcs3Firmware.get(name))}
  {#if view?.kind === 'installed'}
    <div class="check-row" data-testid={`sys-ps3-firmware-${key}`}>
      <span class="st ok"><Icon name="check" size={12} /></span>
      <span class="grow" data-testid={`sys-ps3-firmware-installed-${key}`}>{view.text}</span>
    </div>
  {:else if view?.kind === 'install'}
    <div class="check-row" data-testid={`sys-ps3-firmware-${key}`}>
      <span class="st warn">!</span>
      <span class="grow">{view.text}</span>
      <button
        class="small-btn primary"
        data-testid={`sys-ps3-firmware-install-${key}`}
        onclick={() => void installPs3Firmware(name)}
      >
        Install
      </button>
    </div>
  {/if}
{/snippet}

<div class="systems" data-testid="systems-page">
  <div class="pane-head">
    <div class="title">
      <h2 class="g-pane-title">Systems</h2>
      <p class="lede">
        Each system on your server needs an emulator to play its games. Pick one here; GRID installs, updates and sets
        it up.
      </p>
    </div>
    <div class="head-acts">
      <button class="g-btn secondary" data-testid="systems-check-updates" disabled={checking} onclick={checkForUpdates}>
        {checking ? 'Checking…' : 'Check for updates'}
      </button>
      <button class="g-btn secondary" data-testid="systems-add-manual" onclick={() => (sheet = { mode: 'add' })}>
        Add an emulator manually
      </button>
      {#if missing.length > 0}
        <button class="g-btn" data-testid="systems-install-recommended" onclick={installAllRecommended}>
          {installRecommendedLabel(missing.length)}
        </button>
      {/if}
    </div>
  </div>

  {#if sheet !== null}
    <section class="sheet" data-testid="systems-sheet" aria-label="Emulator settings">
      <div class="sheet-head">
        <h3>{sheet.mode === 'add' ? 'Add an emulator you already have' : `${sheet.entry.name} settings`}</h3>
        <button class="icon-btn" aria-label="Close" data-testid="systems-sheet-close" onclick={() => (sheet = null)}>
          <Icon name="close" size={16} />
        </button>
      </div>
      {#key sheet.mode === 'edit' ? sheet.entry.name : ''}
        <EmulatorForm
          mode={sheet.mode}
          entry={sheet.mode === 'edit' ? sheet.entry : null}
          {profiles}
          onSaved={afterSave}
          onCancel={() => (sheet = null)}
        />
      {/key}
    </section>
  {/if}

  {#if loadError}<p class="g-error" role="alert">{loadError}</p>{/if}

  {#if !loaded}
    <p class="note">Loading…</p>
  {:else}
    <ul class="sys-list" data-testid="systems-list" bind:this={listEl}>
      {#each rows as row (row.key)}
        {@const key = testKeyOf(row.slug || row.label)}
        {@const groups = pickerGroups(row)}
        {@const st = rowStatus(row)}
        {@const fw = firmwareLine(row)}
        {@const core = platformCoreSelect(defaults, row.key, row.emulator, coreOptions[row.name] ?? [])}
        <SystemRow
          testKey={key}
          label={row.label}
          meta={gamesText(row.romCount)}
          open={isOpen(row)}
          attention={row.emulator === ''}
          canToggle={row.emulator !== ''}
          pickerLabel={`Emulator for ${row.label}`}
          selected={groups.selected}
          noneLabel="None"
          installed={groups.installed}
          download={groups.download}
          pickerDisabled={busyRows[row.key] === true || installingRow(row)}
          status={st.status}
          statusText={st.text}
          onToggle={() => (openRows = { ...openRows, [row.key]: !isOpen(row) })}
          onPick={(value, select) => onPick(row, value, select)}
        >
          {#snippet body()}
            {#if row.emulator === ''}
              {#if row.recommended === null}
                <p class="note" data-testid={`sys-missing-${key}`}>{missingText(row)}</p>
              {:else if st.status === 'installing'}
                <p class="note" data-testid={`sys-missing-${key}`}>Installing {shortName(row.recommended.name)}…</p>
              {:else}
                <div class="fix">
                  <span data-testid={`sys-missing-${key}`}>{missingText(row)}</span>
                  <button class="small-btn primary" data-testid={`sys-install-${key}`} onclick={() => installRecommended(row)}>
                    Install {shortName(row.recommended.name)}
                  </button>
                </div>
              {/if}
            {:else}
              {@render emulatorLine(row.emulator, row.key, row.key)}
              {@render companionLines(row)}
              {#if core.visible}
                <label class="dfield">
                  <span>Core</span>
                  <select
                    class="g-field"
                    data-testid={`sys-core-${key}`}
                    value={core.selected}
                    disabled={core.disabled}
                    onchange={(e) => onCoreChange(row, e.currentTarget.value)}
                  >
                    {#if core.disabled}
                      <option value={NO_CORE_VALUE}>No installed core</option>
                    {:else}
                      {#each core.options as id (id)}
                        <option value={id}>{id}</option>
                      {/each}
                    {/if}
                  </select>
                </label>
              {/if}
              {#if fw && !ps3Installed(row.emulator)}
                {@const action = firmwareAction(fw, firmwarePending[fw.key] === true, false)}
                {#if action !== 'unavailable'}
                  <div class="check-row" data-testid={`sys-firmware-${key}`}>
                    {#if action === 'installed'}
                      <span class="st ok"><Icon name="check" size={12} /></span>
                    {:else}
                      <span class="st warn">!</span>
                    {/if}
                    <span class="grow">{fw.label} <span class="muted-s">· from your server</span></span>
                    {#if action === 'installed'}
                      <span class="muted-s" data-testid={`sys-firmware-installed-${key}`}>Installed</span>
                    {:else}
                      <button
                        class="small-btn primary"
                        data-testid={`sys-firmware-install-${key}`}
                        disabled={action === 'requested'}
                        onclick={() => installFirmware(row)}
                      >
                        {action === 'requested' ? 'Installing…' : 'Install'}
                      </button>
                    {/if}
                  </div>
                {/if}
              {/if}
              {@render ps3Firmware(row.emulator, key)}
              {@render tips(row.emulator, key)}
            {/if}
          {/snippet}
        </SystemRow>
      {/each}
      {#if !windowsHost}
        <WindowsRow {active} gameCount={windowsGames} />
      {/if}
    </ul>

    <section class="sub" aria-label="Other emulators">
      <h3>Other emulators</h3>
      {#if others.length === 0}
        <p class="note" data-testid="systems-other-empty">None. Emulators that aren't picked for a system show here.</p>
      {:else}
        <ul class="sys-list" data-testid="systems-other-list">
          {#each others as entry (entry.name)}
            {@const key = testKeyOf(`other-${entry.name}`)}
            <li class="sysrow" data-testid={`sys-other-${testKeyOf(entry.name)}`}>
              <div class="other-body">
                {@render emulatorLine(entry.name, `other-${entry.name}`, null)}
                <p class="meta2" data-testid={`sys-other-note-${testKeyOf(entry.name)}`}>
                  {otherEmulatorNote(canPlay(entry.name, rows), companionParent(entry.name, companions))}
                </p>
                {@render ps3Firmware(entry.name, key)}
                {@render tips(entry.name, key)}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</div>

{#if menu !== null}
  {@const m = menu}
  <RowMenu
    anchor={m.anchor}
    name={m.name}
    {removing}
    onSettings={() => editEmulator(m.name)}
    onShowFolder={() => showFolder(m.name)}
    onRemove={() => removeEmulator(m.name)}
    onClose={() => (menu = null)}
  />
{/if}

<style>
  .systems {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  /* The design prototype's header: the title and the lede on the left, the
     actions top-right beside the title. */
  .pane-head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .title {
    flex: 1 1 auto;
    min-width: 0;
  }

  .lede {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .head-acts {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    flex: none;
  }

  .sheet {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--overlay);
    box-shadow: var(--shadow-lift);
  }

  .sheet-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h3 {
    margin: 0;
    font-size: 14px;
    color: var(--text-h);
  }

  .sheet-head h3 {
    font-size: 15px;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .icon-btn:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .sys-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .sysrow {
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .other-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px 12px;
  }

  .sub {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 6px;
  }

  .note {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .meta2 {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .muted-s {
    font-size: 12px;
    color: var(--text-muted);
  }

  .fix {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 12px;
    font-size: 13px;
    color: var(--text);
  }

  .fix > span {
    flex: 1 1 200px;
  }

  /* A missing companion: indented under its parent's line, like the
     companion's own line (EmulatorBlock's .sub). */
  .fix.sub {
    padding-left: 12px;
    border-left: 2px solid var(--line);
  }

  .kicker {
    margin-right: 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    color: var(--text-muted);
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

  .grow {
    flex: 1 1 auto;
    min-width: 0;
  }

  .st {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    font-size: 11px;
    font-weight: 700;
  }

  .st.ok {
    color: var(--on-primary);
    background: var(--primary);
  }

  .st.warn {
    color: #1a1a12;
    background: var(--warning);
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

  .small-btn.primary {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--on-primary);
  }

  .small-btn.primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .dfield {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .dfield select {
    max-width: 260px;
  }

  .tips summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 0;
  }

  .tips summary:hover,
  .tips[open] summary {
    color: var(--text-h);
  }

  .tips ul {
    margin: 6px 0 2px;
    padding-left: 18px;
    font-size: 13px;
    color: var(--text);
  }
</style>
