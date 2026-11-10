<script lang="ts">
  import { type SetupStarted } from '../api';
  import Icon from '../Icon.svelte';
  import SystemRow from '../systems/SystemRow.svelte';
  import { gamesText, testKeyOf } from '../systems/rows';
  import { isWindowsHost } from '../emulators/compatTools';
  import { canStart, choosePicker, primaryLabel, rowState } from './choices';
  import { ChooseSystems } from './chooseSystems.svelte';

  // First-run setup, after Connect: Choose your systems (ProposedApp README,
  // "Fresh start"). The same rows as Settings › Systems (SystemRow in tick
  // mode), none ticked; ticking one pre-selects GRID's recommendation. On
  // Linux a Windows row picks the default Proton version, GE-Proton
  // pre-selected. `onDone` hands the app on to the shell, with what was
  // started and the name shown for each install, or `null` when skipped.
  // Later changes happen in Settings › Systems. The data and the calls are
  // `chooseSystems.svelte.ts`; the aliases keep the markup reading as before.
  let {
    onDone,
  }: { onDone: (started: SetupStarted | null, names: Record<string, string>) => void } = $props();

  const windowsHost = isWindowsHost(navigator.platform);
  const cs = new ChooseSystems(windowsHost);

  let rows = $derived(cs.rows);
  let loadError = $derived(cs.loadError);
  let selection = $derived(cs.selection);
  let proton = $derived(cs.proton);
  let windowsLoaded = $derived(cs.windowsLoaded);
  let busy = $derived(cs.busy);
  let startError = $derived(cs.startError);
  let protonOptions = $derived(cs.protonOptions);
  let protonDefault = $derived(cs.protonDefault);
  let windowsGames = $derived(cs.windowsGames);
  let button = $derived(cs.button);

  function load() {
    return cs.load();
  }

  $effect(() => {
    cs.load();
    cs.loadWindows();
  });

  function tickWindows() {
    cs.tickWindows();
  }

  async function install() {
    const done = await cs.install();
    if (done !== null) onDone(done.started, done.names);
  }

  async function skip() {
    await cs.skip();
    onDone(null, {});
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
      <button class="g-btn secondary" onclick={load}>Retry</button>
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
            onTick: () => cs.tick(row),
          }}
          note={unavailable ? 'No emulator available' : !isTicked && state === 'installed' ? 'Installed' : null}
          onPick={(value) => cs.pick(row, value)}
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
          onPick={(value) => cs.pickProton(value)}
        />
      {/if}
    </ul>
  {/if}

  <footer>
    {#if startError}<p class="error" role="alert" data-testid="setup-error">{startError}</p>{/if}
    <div class="row-actions">
      <button class="g-btn secondary" data-testid="setup-skip" disabled={busy} onclick={skip}>
        Skip for now
      </button>
      <button class="g-btn" data-testid="setup-install" disabled={busy || !canStart(button)} onclick={install}>
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
