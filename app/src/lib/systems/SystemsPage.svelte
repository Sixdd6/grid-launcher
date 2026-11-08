<script lang="ts">
  // Settings › Systems (ProposedApp README, "One row per system on the
  // server" to "Add an emulator manually"): every emulator choice, install,
  // update and the Windows (Proton) setup in one page. One row per server
  // system with an emulator picker that is also the installer and a status;
  // systems with no emulator come first, open. The data is what the
  // Emulators view reads today (`emulators/*.ts` holds the shared rules);
  // the row model is `rows.ts`.
  import { listen } from '@tauri-apps/api/event';
  import { untrack } from 'svelte';
  import {
    api,
    FIRMWARE_PASS_FINISHED_EVENT,
    SETUP_DEFAULTS_EVENT,
    type EmulatorEntry,
    type EmulatorFacts,
    type FirmwarePassFinished,
    type LaunchDefaults,
    type Platform,
    type PlatformFirmwareStatus,
    type PlatformSetup,
    type ProfileSummary,
    type SetupChoice,
    type SetupStarted,
  } from '../api';
  import { downloads } from '../stores/downloads.svelte';
  import { pushToast } from '../stores/toasts.svelte';
  import { SKIPPED_TOAST_MS, skippedRemovalText } from '../uninstallSkips';
  import { emulatorTerminalSignature as terminalSignature } from '../emulators/installSignal';
  import { defaultEmulatorKey, NO_CORE_VALUE, platformCoreSelect } from '../emulators/defaults';
  import { isWindowsHost } from '../emulators/compatTools';
  import { dynamicEmulatorNotes, emulatorNotes } from '../emulators/notes';
  import {
    emulatorFirmwareRows,
    firmwareAction,
    firmwareFinishedToast,
    firmwareStartedToast,
    type EmulatorFirmwareRow,
    type FirmwareRequest,
  } from '../emulators/firmware';
  import { checkOncePerSession, emulatorUpdates, recheck } from '../stores/emulatorUpdates.svelte';
  import { isWindowsPlatform } from '../details/actions';
  import { platformLabel } from '../server/platformLabel';
  import EmulatorForm from '../emulators/EmulatorForm.svelte';
  import Icon from '../Icon.svelte';
  import SystemRow from './SystemRow.svelte';
  import EmulatorBlock from './EmulatorBlock.svelte';
  import RowMenu from './RowMenu.svelte';
  import WindowsRow from './WindowsRow.svelte';
  import {
    alsoPlays,
    alsoPlaysText,
    buildSystemRows,
    canPlay,
    gamesText,
    installRecommendedLabel,
    isInstalling,
    liveEmulatorSources,
    missingRecommended,
    missingText,
    otherEmulatorNote,
    otherEmulators,
    parsePick,
    pickerGroups,
    shortName,
    statusOf,
    systemFirmwareRequests,
    testKeyOf,
    versionText,
    type SystemRow as Row,
  } from './rows';

  let { active = true }: { active?: boolean } = $props();

  const windowsHost = isWindowsHost(navigator.platform);

  // --- Data --------------------------------------------------------------
  let platforms = $state<Platform[]>([]);
  let setupRows = $state<PlatformSetup[]>([]);
  let defaults = $state<LaunchDefaults | null>(null);
  let emulators = $state<EmulatorEntry[]>([]);
  let compatible = $state<Record<string, string[]>>({});
  let coreOptions = $state<Record<string, string[]>>({});
  let pendingLabels = $state<string[]>([]);
  let profiles = $state<ProfileSummary[]>([]);
  let facts = $state<Map<string, EmulatorFacts>>(new Map());
  let rpcs3Pup = $state<Map<string, string | null>>(new Map());
  let loaded = $state(false);
  let loadError = $state<string | null>(null);

  let rows = $derived(buildSystemRows(setupRows, platforms, defaults, compatible));
  let live = $derived(liveEmulatorSources(downloads.entries));
  let installingRow = $derived((row: Row) => isInstalling(row, pendingLabels, live));
  let missing = $derived(missingRecommended(rows, installingRow));
  let others = $derived(otherEmulators(emulators, rows));
  let windowsGames = $derived(
    platforms
      .filter((p) => isWindowsPlatform(p.slug, platformLabel(p)))
      .reduce<number | null>((n, p) => (n ?? 0) + p.rom_count, null),
  );

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshEmulators() {
    try {
      emulators = await api.listEmulators();
    } catch (err) {
      loadError = errorMessage(err);
    }
  }

  async function refreshDefaults() {
    try {
      defaults = await api.getLaunchDefaults();
    } catch (err) {
      loadError = errorMessage(err);
    }
  }

  async function refreshSystems() {
    try {
      const [p, s] = await Promise.all([api.listPlatforms(), api.setupPlatformOptions()]);
      platforms = p;
      setupRows = s;
      loadError = null;
    } catch (err) {
      loadError = errorMessage(err);
    }
  }

  async function refreshPending() {
    try {
      pendingLabels = (await api.setupStatus()).pending;
    } catch {
      // Best effort: rows just don't say Installing… from it.
    }
  }

  async function refreshAll() {
    await Promise.all([refreshEmulators(), refreshDefaults(), refreshSystems(), refreshPending()]);
    loaded = true;
  }

  // Comes forward, or an emulator install reached its end: everything the
  // rows read may have changed (an entry added, a default applied).
  let installSignature = $derived(terminalSignature(downloads.entries));
  $effect(() => {
    void installSignature;
    if (!active) return;
    untrack(() => {
      void refreshAll();
    });
  });

  // The once-per-session update check, the first time the page has its
  // emulator list. Never re-run by a scroll or a re-render.
  $effect(() => {
    if (!active || !loaded) return;
    untrack(() => {
      void checkOncePerSession(emulators);
    });
  });

  $effect(() => {
    if (!active) return;
    untrack(() => {
      api
        .listProfiles()
        .then((p) => (profiles = p))
        .catch(() => {});
    });
  });

  // The compatibility and core answers depend on the platforms asked about
  // and on the emulator list.
  let compatibilityKey = $derived(
    JSON.stringify([platforms.map((p) => [p.name, p.slug]), emulators.map((e) => e.name)]),
  );
  $effect(() => {
    void compatibilityKey;
    const refs = untrack(() => platforms.map((p) => ({ name: p.name, slug: p.slug })));
    if (refs.length === 0) return;
    api
      .compatibleEmulators(refs)
      .then((c) => (compatible = c))
      .catch(() => {});
    api
      .retroarchCoreOptions(refs)
      .then((c) => (coreOptions = c))
      .catch(() => {});
  });

  // The Eden notes' file probes and the RPCS3 firmware package check.
  let emulatorNames = $derived(emulators.map((e) => e.name).join('\n'));
  $effect(() => {
    void emulatorNames;
    if (!active) return;
    const names = untrack(() => emulators.map((e) => e.name));
    void Promise.all(
      names.map(async (name) => {
        try {
          return [name, await api.emulatorFacts(name)] as const;
        } catch {
          return null;
        }
      }),
    ).then((answers) => (facts = new Map(answers.filter((a) => a !== null))));
    void Promise.all(
      names
        .filter((n) => n.toLowerCase().includes('rpcs3'))
        .map(async (name) => {
          try {
            return [name, (await api.rpcs3FirmwareStatus(name)).pup_path] as const;
          } catch {
            return null;
          }
        }),
    ).then((answers) => (rpcs3Pup = new Map(answers.filter((a) => a !== null))));
  });

  // Setup's defaults land after each picked emulator's install.
  $effect(() => {
    const pending = listen(SETUP_DEFAULTS_EVENT, () => {
      void refreshDefaults();
      void refreshPending();
    });
    return () => {
      void pending.then((off) => off());
    };
  });

  // --- Firmware ----------------------------------------------------------
  let firmwareStatus = $state<Record<string, PlatformFirmwareStatus>>({});
  let firmwarePending = $state<Record<string, true>>({});
  let firmwareRequests = $derived(systemFirmwareRequests(rows));
  let firmwareSignature = $derived(firmwareRequests.map((r) => r.key).join(','));

  function refreshFirmwareStatus(req: FirmwareRequest) {
    api
      .platformFirmwareStatus(req.platform.id, defaultEmulatorKey(req.platform), req.emulator)
      .then((status) => (firmwareStatus = { ...firmwareStatus, [req.key]: status }))
      .catch(() => {
        const { [req.key]: _gone, ...rest } = firmwareStatus;
        firmwareStatus = rest;
      });
  }

  $effect(() => {
    void firmwareSignature;
    if (!active) return;
    for (const req of untrack(() => firmwareRequests)) refreshFirmwareStatus(req);
  });

  $effect(() => {
    const pending = listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (event) => {
      const { platform_id: id, ok, failed } = event.payload;
      for (const req of firmwareRequests.filter((r) => r.platform.id === id)) {
        // Only an Install this page started says how it ended.
        if (firmwarePending[req.key] === true) {
          const { [req.key]: _done, ...rest } = firmwarePending;
          firmwarePending = rest;
          const toast = firmwareFinishedToast(defaultEmulatorKey(req.platform), req.emulator, ok, failed);
          pushToast(toast.text, toast.level);
        }
        refreshFirmwareStatus(req);
      }
    });
    return () => {
      void pending.then((off) => off());
    };
  });

  function firmwareLine(row: Row): EmulatorFirmwareRow | null {
    if (row.emulator === '' || row.platform === null) return null;
    const id = row.platform.id;
    return (
      emulatorFirmwareRows(row.emulator, platforms, compatible, defaults, firmwareStatus).find(
        (f) => f.platformId === id,
      ) ?? null
    );
  }

  function installFirmware(row: Row) {
    const req = firmwareRequests.find((r) => r.platform.id === row.platform?.id && r.emulator === row.emulator);
    if (!req) return;
    const name = defaultEmulatorKey(req.platform);
    firmwarePending = { ...firmwarePending, [req.key]: true };
    pushToast(firmwareStartedToast(name, req.emulator));
    api.installFirmwareForPlatform(req.platform.id, name, req.emulator).catch((err) => {
      const { [req.key]: _failed, ...rest } = firmwarePending;
      firmwarePending = rest;
      pushToast(`Could not install ${name} firmware: ${errorMessage(err)}`, 'error');
    });
  }

  // --- Rows --------------------------------------------------------------
  let openRows = $state<Record<string, boolean>>({});
  let busyRows = $state<Record<string, true>>({});

  function isOpen(row: Row): boolean {
    return row.emulator === '' || openRows[row.key] === true;
  }

  function rowStatus(row: Row) {
    const fw = firmwareLine(row);
    const action = fw ? firmwareAction(fw, firmwarePending[fw.key] === true, false) : null;
    const status = statusOf({
      emulator: row.emulator,
      installing: installingRow(row) || busyRows[row.key] === true,
      firmware: action,
      updateAvailable: row.emulator !== '' && emulatorUpdates.hasUpdate(row.emulator),
    });
    const text = status === 'installing' && action === 'requested' && !installingRow(row) ? 'Installing firmware…' : null;
    return { status, text };
  }

  function started(result: SetupStarted) {
    for (const e of result.errors) pushToast(e, 'error');
  }

  async function install(choices: SetupChoice[], what: string) {
    if (choices.length === 0) return;
    const marks = Object.fromEntries(choices.map((c) => [c.platform, true as const]));
    busyRows = { ...busyRows, ...marks };
    try {
      const result = await api.installForSystems(choices);
      started(result);
      if (result.queued.length > 0) pushToast(what);
      await Promise.all([refreshPending(), refreshDefaults()]);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      const next = { ...busyRows };
      for (const c of choices) delete next[c.platform];
      busyRows = next;
    }
  }

  function installRecommended(row: Row) {
    if (row.recommended === null) return;
    void install(
      [{ platform: row.key, source_id: row.recommended.source_id }],
      `Installing ${shortName(row.recommended.name)} for ${row.label}…`,
    );
  }

  function installAllRecommended() {
    const choices = missing;
    void install(choices, `Installing the recommended emulator for ${choices.length === 1 ? '1 system' : `${choices.length} systems`}…`);
  }

  async function onPick(row: Row, value: string, select: HTMLSelectElement) {
    const pick = parsePick(value);
    if (pick.kind === 'install') {
      const option = row.downloads.find((o) => o.source_id === pick.sourceId);
      await install(
        [{ platform: row.key, source_id: pick.sourceId }],
        `Installing ${shortName(option?.name ?? pick.sourceId)} for ${row.label}…`,
      );
    } else {
      busyRows = { ...busyRows, [row.key]: true };
      try {
        await api.setDefaultEmulator(row.key, pick.kind === 'use' ? pick.name : '');
        await Promise.all([refreshDefaults(), refreshPending()]);
        if (pick.kind === 'use') openRows = { ...openRows, [row.key]: true };
      } catch (err) {
        pushToast(errorMessage(err), 'error');
      } finally {
        const { [row.key]: _done, ...rest } = busyRows;
        busyRows = rest;
      }
    }
    // An install pick is not the system's emulator yet: show the real one.
    const current = rows.find((r) => r.key === row.key);
    if (current) select.value = pickerGroups(current).selected;
  }

  async function onCoreChange(row: Row, core: string) {
    try {
      await api.setRetroarchCore(row.key, core);
      await refreshDefaults();
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    }
  }

  // --- The emulator line --------------------------------------------------
  let launching = $state<string | null>(null);
  let updating = $state<string | null>(null);

  function entryFor(name: string): EmulatorEntry | undefined {
    return emulators.find((e) => e.name === name);
  }

  async function openEmulator(name: string) {
    launching = name;
    try {
      const warning = await api.launchEmulator(name);
      if (warning) pushToast(warning, 'error');
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      launching = null;
    }
  }

  async function updateEmulator(name: string) {
    updating = name;
    try {
      await api.updateEmulator(name);
      pushToast(`Updating ${name}…`);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      updating = null;
    }
  }

  let checking = $derived(emulatorUpdates.running);

  async function checkForUpdates() {
    await recheck(emulators);
    const count = emulators.filter((e) => emulatorUpdates.hasUpdate(e.name)).length;
    pushToast(
      count === 0 ? 'Every emulator is up to date.' : `${count === 1 ? '1 update' : `${count} updates`} available.`,
    );
  }

  // --- The ⋯ menu ----------------------------------------------------------
  let menu = $state<{ where: string; name: string; anchor: HTMLElement } | null>(null);
  let removing = $state(false);

  function toggleMenu(where: string, name: string, anchor: HTMLElement) {
    menu = menu?.where === where ? null : { where, name, anchor };
  }

  async function showFolder(name: string) {
    menu = null;
    try {
      await api.showEmulatorFolder(name);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    }
  }

  async function removeEmulator(name: string) {
    removing = true;
    try {
      const report = await api.deleteEmulator(name);
      pushToast(skippedRemovalText(report?.skipped), 'error', SKIPPED_TOAST_MS);
      pushToast(`${name} removed`);
      if (sheet?.mode === 'edit' && sheet.entry.name === name) sheet = null;
      menu = null;
      await refreshAll();
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      removing = false;
    }
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
    await refreshAll();
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
  {#if rpcs3Pup.get(name)}
    <div class="check-row" data-testid={`sys-ps3-firmware-${key}`}>
      <span class="st warn">!</span>
      <span class="grow">PS3 firmware downloaded — click Install to activate it.</span>
      <button
        class="small-btn primary"
        onclick={() =>
          api
            .installPs3Firmware(name)
            .then((ok) =>
              pushToast(
                ok
                  ? 'PS3 firmware installation started — follow the RPCS3 dialog to complete.'
                  : 'Could not launch RPCS3 to install firmware. Check the emulator path.',
                ok ? 'success' : 'error',
              ),
            )
            .catch((err) => pushToast(errorMessage(err), 'error'))}
      >
        Install
      </button>
    </div>
  {/if}
{/snippet}

<div class="systems" data-testid="systems-page">
  <div class="pane-head">
    <p class="lede">
      Each system on your server needs an emulator to play its games. Pick one here; GRID installs, updates and sets
      it up.
    </p>
    <div class="head-acts">
      <button class="btn secondary" data-testid="systems-check-updates" disabled={checking} onclick={checkForUpdates}>
        {checking ? 'Checking…' : 'Check for updates'}
      </button>
      <button class="btn secondary" data-testid="systems-add-manual" onclick={() => (sheet = { mode: 'add' })}>
        Add an emulator manually
      </button>
      {#if missing.length > 0}
        <button class="btn" data-testid="systems-install-recommended" onclick={installAllRecommended}>
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
          variant="sheet"
          mode={sheet.mode}
          entry={sheet.mode === 'edit' ? sheet.entry : null}
          {profiles}
          onSaved={afterSave}
          onCancel={() => (sheet = null)}
        />
      {/key}
    </section>
  {/if}

  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

  {#if !loaded}
    <p class="muted">Loading…</p>
  {:else}
    <ul class="sys-list" data-testid="systems-list">
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
              {#if core.visible}
                <label class="dfield">
                  <span>Core</span>
                  <select
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
              {#if fw}
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
                <p class="meta2">{otherEmulatorNote(canPlay(entry.name, rows))}</p>
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

  .pane-head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .lede {
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
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

  .btn {
    font: inherit;
    font-size: 13px;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: 1px solid var(--primary);
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
    white-space: nowrap;
  }

  .btn:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .btn.secondary {
    border-color: var(--border);
    background: transparent;
    color: var(--text-h);
  }

  .btn.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .sheet {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 560px;
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

  .note,
  .muted {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .error {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
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
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
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
