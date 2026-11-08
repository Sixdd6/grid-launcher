<script lang="ts">
  import {
    api,
    type CatalogEntry,
    type CompatTool,
    type Platform,
    type PlatformSetup,
    type SetupStarted,
  } from '../api';
  import Icon from '../Icon.svelte';
  import SystemRow from '../systems/SystemRow.svelte';
  import { gamesText, shortName, testKeyOf } from '../systems/rows';
  import { protonInstallSource, protonPicker } from '../systems/proton';
  import { isWindowsHost } from '../emulators/compatTools';
  import { isWindowsPlatform } from '../details/actions';
  import { platformLabel } from '../server/platformLabel';
  import { pushToast } from '../stores/toasts.svelte';
  import {
    canStart,
    choices,
    choosePicker,
    compatChoice,
    installCount,
    pick,
    primaryLabel,
    protonPreselect,
    rowState,
    sortRows,
    toggle,
    toInstall,
    type Selection,
  } from './choices';

  // First-run setup, after Connect: Choose your systems (ProposedApp README,
  // "Fresh start"). The same rows as Settings › Systems (SystemRow in tick
  // mode), none ticked; ticking one pre-selects GRID's recommendation. On
  // Linux a Windows row picks the default Proton version, GE-Proton
  // pre-selected. `onDone` hands the app on to the shell, with what was
  // started and the name shown for each install, or `null` when skipped.
  // Later changes happen in Settings › Systems.
  let {
    onDone,
  }: { onDone: (started: SetupStarted | null, names: Record<string, string>) => void } = $props();

  const windowsHost = isWindowsHost(navigator.platform);

  let rows = $state.raw<PlatformSetup[] | null>(null);
  let loadError = $state<string | null>(null);
  let selection = $state.raw<Selection>({});
  let tools = $state.raw<CompatTool[]>([]);
  let defaultTool = $state('');
  let catalog = $state.raw<CatalogEntry[]>([]);
  let platforms = $state.raw<Platform[]>([]);
  /** The Windows row's value; `null` while unticked. */
  let proton = $state<string | null>(null);
  let windowsLoaded = $state(false);
  let busy = $state(false);
  let startError = $state<string | null>(null);

  async function load() {
    loadError = null;
    try {
      rows = sortRows(await api.setupPlatformOptions());
    } catch (e) {
      loadError = String(e);
    }
  }

  // Best effort: without them the Windows row just offers less.
  async function loadWindows() {
    if (windowsHost) return;
    const [t, c, p] = await Promise.allSettled([
      api.listCompatTools(),
      api.listCompatToolCatalog(),
      api.listPlatforms(),
    ]);
    if (t.status === 'fulfilled') {
      tools = t.value.tools;
      defaultTool = t.value.default_tool;
    }
    if (c.status === 'fulfilled') catalog = c.value;
    if (p.status === 'fulfilled') platforms = p.value;
    windowsLoaded = true;
  }

  $effect(() => {
    load();
    loadWindows();
  });

  let protonOptions = $derived(protonPicker(tools, catalog, defaultTool));
  let protonDefault = $derived(protonPreselect(protonOptions));
  let windowsGames = $derived(
    platforms
      .filter((p) => isWindowsPlatform(p.slug, platformLabel(p)))
      .reduce<number | null>((n, p) => (n ?? 0) + p.rom_count, null),
  );
  let ticked = $derived(Object.keys(selection).length);
  let button = $derived({
    installs: rows === null ? 0 : installCount(rows, selection, proton),
    ticked,
    proton,
  });

  function tickWindows() {
    proton = proton === null ? protonDefault : null;
  }

  /** The name each install is shown by on the setup card. */
  function installNames(): Record<string, string> {
    const names: Record<string, string> = {};
    for (const option of toInstall(rows ?? [], selection)) names[option.source_id] = shortName(option.name);
    const source = proton === null ? null : protonInstallSource(proton);
    if (source !== null) names[source] = catalog.find((e) => e.source_id === source)?.name ?? source;
    return names;
  }

  async function install() {
    busy = true;
    startError = null;
    try {
      const names = installNames();
      const started = await api.startPlatformSetup(choices(selection), compatChoice(proton));
      if (started.compat_set && started.queued.length === 0) {
        const tool = protonOptions.installed.find((o) => o.value === proton);
        pushToast(`Windows games will use ${tool?.label ?? 'the Proton version you picked'}`);
      }
      onDone(started, names);
    } catch (e) {
      startError = String(e);
    } finally {
      busy = false;
    }
  }

  async function skip() {
    busy = true;
    try {
      await api.skipPlatformSetup();
    } catch {
      // Not recorded: the chooser comes back on the next start, which is the
      // honest outcome of a skip that could not be saved.
    } finally {
      busy = false;
      onDone(null, {});
    }
  }
</script>

<section class="setup" data-testid="setup-view">
  <header>
    <span class="mark"><Icon name="grid" size={28} /></span>
    <h1>Choose your systems</h1>
    <p class="lede">Tick the systems you play. GRID installs an emulator for each and sets it up.</p>
  </header>

  {#if loadError !== null}
    <p class="notice" role="status">Your systems could not be loaded: {loadError}</p>
    <div class="row-actions">
      <button class="secondary" onclick={load}>Retry</button>
    </div>
  {:else if rows === null}
    <p class="muted" role="status">Loading your systems…</p>
  {:else}
    {#if rows.length === 0}
      <p class="notice" role="status">Your server has no systems that need an emulator.</p>
    {/if}
    <ul class="sys-list">
      {#each rows as row (row.label)}
        {@const state = rowState(row)}
        {@const unavailable = state === 'unavailable'}
        {@const isTicked = row.label in selection}
        {@const groups = choosePicker(row)}
        <SystemRow
          testKey={testKeyOf(row.slug || row.label)}
          label={row.label}
          meta={gamesText(row.rom_count)}
          open={false}
          pickerLabel={`Emulator for ${row.label}`}
          selected={selection[row.label] ?? ''}
          installed={groups.installed}
          download={groups.download}
          pickerDisabled={busy}
          tick={{
            checked: isTicked,
            disabled: unavailable || busy,
            onTick: () => (selection = toggle(selection, row)),
          }}
          note={unavailable ? 'No emulator available' : !isTicked && state === 'installed' ? 'Installed' : null}
          onPick={(value) => (selection = pick(selection, row, value))}
        />
      {/each}
      {#if !windowsHost}
        <SystemRow
          testKey="windows"
          label="Windows"
          meta={windowsGames === null ? 'runs through Proton' : `${gamesText(windowsGames)} · runs through Proton`}
          open={false}
          pickerLabel="Proton version for Windows games"
          selected={proton ?? ''}
          installed={protonOptions.installed}
          download={protonOptions.download}
          pickerDisabled={busy}
          tick={{ checked: proton !== null, disabled: protonDefault === null || busy, onTick: tickWindows }}
          note={windowsLoaded && protonDefault === null && proton === null ? 'No Proton version available' : null}
          onPick={(value) => (proton = value)}
        />
      {/if}
    </ul>
  {/if}

  <footer>
    {#if startError}<p class="error" role="alert" data-testid="setup-error">{startError}</p>{/if}
    <div class="row-actions">
      <button class="secondary" data-testid="setup-skip" disabled={busy} onclick={skip}>
        Skip for now
      </button>
      <button class="primary" data-testid="setup-install" disabled={busy || !canStart(button)} onclick={install}>
        {busy ? 'Starting…' : primaryLabel(button)}
      </button>
    </div>
    <p class="muted small">You can change these later in Settings › Systems.</p>
  </footer>
</section>

<style>
  /* Design system "First open" / prototype fresh.css: a centred column
     (wider than Connect's, for the rows), 32px above, 16px between blocks;
     one primary button. The list scrolls and the footer stays put. */
  .setup {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(720px, 100%);
    height: 100%;
    margin: 0 auto;
    padding: 32px 24px 24px;
    box-sizing: border-box;
    min-height: 0;
  }

  header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
  }

  .mark {
    display: flex;
    color: var(--primary);
  }

  h1 {
    font-size: 22px;
    font-weight: 600;
    color: var(--text-h);
    margin: 0;
  }

  .lede {
    margin: 0;
    font-size: 14px;
    color: var(--text-muted);
  }

  .sys-list {
    /* Fits its rows, so the buttons sit right under a short list; a long
       one scrolls inside the column. */
    flex: 0 1 auto;
    min-height: 0;
    overflow-y: auto;
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  footer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    cursor: pointer;
  }

  .primary {
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    transition: background var(--m-fast) ease;
  }

  .primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .secondary {
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
  }

  .secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .notice {
    margin: 0;
    padding: 10px 12px;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
    font-size: 14px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 14px;
  }

  .muted.small {
    font-size: 12px;
    text-align: right;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 14px;
  }
</style>
