<script lang="ts">
  import { api, type PlatformSetup, type SetupStarted } from '../api';
  import Icon from '../Icon.svelte';
  import {
    choices,
    optionLabel,
    pick,
    primaryLabel,
    rowState,
    sortRows,
    toggle,
    toInstall,
    type Selection,
  } from './choices';

  // First-run setup, step 2 (docs/superpowers/plans/2026-10-28-first-run-
  // emulator-setup.md): the server's platforms that need an emulator, none
  // ticked; ticking one pre-selects GRID's recommendation. `onDone` hands the
  // app on to the shell — with what was started, or `null` when skipped.
  // `later`: opened again from Emulators › Platform defaults. The secondary
  // button closes the picker without recording anything, and the lede says
  // what it does to platforms that already have an emulator.
  let {
    onDone,
    mode = 'first-run',
  }: { onDone: (started: SetupStarted | null) => void; mode?: 'first-run' | 'later' } = $props();

  let rows = $state.raw<PlatformSetup[] | null>(null);
  let loadError = $state<string | null>(null);
  let selection = $state.raw<Selection>({});
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

  $effect(() => {
    load();
  });

  let pending = $derived(rows === null ? [] : toInstall(rows, selection));
  let tickedCount = $derived(Object.keys(selection).length);

  async function install() {
    busy = true;
    startError = null;
    try {
      onDone(await api.startPlatformSetup(choices(selection)));
    } catch (e) {
      startError = String(e);
    } finally {
      busy = false;
    }
  }

  async function skip() {
    if (mode === 'later') {
      onDone(null);
      return;
    }
    busy = true;
    try {
      await api.skipPlatformSetup();
    } catch {
      // Not recorded: the picker comes back on the next start, which is the
      // honest outcome of a skip that could not be saved.
    } finally {
      busy = false;
      onDone(null);
    }
  }
</script>

<section class="setup" data-testid="setup-view">
  <header>
    <span class="mark"><Icon name="grid" size={28} /></span>
    <h1>{mode === 'later' ? 'Set up platforms' : 'Choose your platforms'}</h1>
    <p class="lede">
      {mode === 'later'
        ? 'Tick platforms to install their emulator. A ticked platform gets the emulator you pick as its default.'
        : 'Tick the platforms you play. GRID installs an emulator for each and sets it up.'}
    </p>
  </header>

  {#if loadError !== null}
    <p class="notice" role="status">Your platforms could not be loaded: {loadError}</p>
    <div class="row-actions">
      <button class="secondary" onclick={load}>Retry</button>
    </div>
  {:else if rows === null}
    <p class="muted" role="status">Loading your platforms…</p>
  {:else if rows.length === 0}
    <p class="notice" role="status">Your server has no platforms that need an emulator.</p>
  {:else}
    <ul class="list">
      {#each rows as row (row.label)}
        {@const state = rowState(row)}
        {@const ticked = row.label in selection}
        <li class="item" class:ticked class:unavailable={state === 'unavailable'} data-testid={`setup-row-${row.slug}`}>
          <label class="pick">
            <input
              type="checkbox"
              data-testid={`setup-check-${row.slug}`}
              checked={ticked}
              disabled={state === 'unavailable' || busy}
              onchange={() => (selection = toggle(selection, row))}
            />
            <span class="name">{row.label}</span>
            <span class="count">{row.rom_count} {row.rom_count === 1 ? 'game' : 'games'}</span>
          </label>
          {#if state === 'unavailable'}
            <span class="note" data-testid={`setup-unavailable-${row.slug}`}>No emulator available</span>
          {:else if ticked}
            <select
              data-testid={`setup-emulator-${row.slug}`}
              value={selection[row.label]}
              disabled={busy}
              onchange={(e) => (selection = pick(selection, row, e.currentTarget.value))}
            >
              {#each row.options as option (option.source_id)}
                <option value={option.source_id}>{optionLabel(option, row.core)}</option>
              {/each}
            </select>
          {:else if state === 'installed'}
            <span class="note" data-testid={`setup-installed-${row.slug}`}>Installed</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <footer>
    {#if startError}<p class="error" role="alert" data-testid="setup-error">{startError}</p>{/if}
    <div class="row-actions">
      <button class="secondary" data-testid="setup-skip" disabled={busy} onclick={skip}>
        {mode === 'later' ? 'Cancel' : 'Skip for now'}
      </button>
      <button class="primary" data-testid="setup-install" disabled={busy || tickedCount === 0} onclick={install}>
        {busy ? 'Starting…' : primaryLabel(pending.length, tickedCount)}
      </button>
    </div>
    {#if mode === 'first-run'}
      <p class="muted small">You can change these later in Emulators › Platform defaults.</p>
    {/if}
  </footer>
</section>

<style>
  /* Design system "First open": a centred column (wider than Connect's, for
     the list), 32px above, 16px between blocks; one primary button. The list
     scrolls and the footer stays put. */
  .setup {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(640px, 100%);
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
    font-size: 28px;
    margin: 0;
  }

  .lede {
    margin: 0;
    font-size: 14px;
    color: var(--text-muted);
  }

  .list {
    /* Fits its rows, so the buttons sit right under a short list; a long
       one scrolls inside the column. */
    flex: 0 1 auto;
    min-height: 0;
    overflow-y: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    border: 1px solid var(--border);
    border-radius: var(--r-row);
    background: var(--surface-2);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 4px 8px;
    border-radius: var(--r-row);
  }

  .item:hover:not(.unavailable),
  .item.ticked {
    background: var(--surface);
  }

  .pick {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }

  .unavailable .pick {
    cursor: default;
  }

  .pick input {
    width: 16px;
    height: 16px;
    margin: 0;
    flex: none;
    accent-color: var(--primary);
  }

  .name {
    font-size: 14px;
    color: var(--text-h);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unavailable .name {
    color: var(--text-muted);
  }

  .count {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
  }

  .note {
    flex: none;
    font-size: 12px;
    color: var(--text-muted);
  }

  select {
    flex: none;
    max-width: 260px;
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
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

  button:focus-visible,
  select:focus-visible,
  .pick input:focus-visible {
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
