<script lang="ts">
  // The "Install umu-launcher" action under a Proton launch error
  // (`isUmuMissing`, launchError.ts) in Library, Server and Details. It
  // queues the same install as Emulators › Compat tools; the install runs
  // as its own Downloads row, so the user launches again once it finishes.
  import { api } from './api';
  import { downloads } from './stores/downloads.svelte';
  import { umuInstallBusy } from './emulators/compatTools';

  let requested = $state(false);
  let pending = $state(false);
  let error = $state<string | null>(null);

  let busy = $derived(pending || umuInstallBusy(downloads.entries));

  async function install() {
    error = null;
    pending = true;
    try {
      await api.installUmuLauncher();
      requested = true;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      pending = false;
    }
  }
</script>

<div data-testid="umu-install-offer" class="offer">
  <button data-testid="umu-install-offer-button" disabled={busy} onclick={install}>
    {busy ? 'Installing umu-launcher…' : 'Install umu-launcher'}
  </button>
  {#if requested || busy}
    <span data-testid="umu-install-offer-note" class="note">
      umu-launcher is installing (see Downloads). Launch the game again when it finishes.
    </span>
  {/if}
  {#if error}<span data-testid="umu-install-offer-error" class="error" role="alert">{error}</span>{/if}
</div>

<style>
  .offer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  button {
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

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .note {
    color: var(--text);
    font-size: 13px;
  }

  .error {
    color: var(--danger);
    font-size: 13px;
  }
</style>
