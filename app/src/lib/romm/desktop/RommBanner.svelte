<script lang="ts">
  // RomM › Installed's one-line banner (RommStorefront README "RomM ›
  // Installed"; prototype `rmBanner`): Can't reach RomM · Retry, or RomM
  // sign-in expired · Sign in again…. None while not connected: the rail's
  // `off` flag and the header chip say it, as for a store.
  import type { RommStatus } from '../../api';
  import { navigation } from '../../stores/navigation.svelte';
  import { reconnectRomm } from '../../stores/romm.svelte';
  import { rommBanner } from '../words';

  let { status }: { status: RommStatus } = $props();

  let banner = $derived(rommBanner(status));
  let retrying = $state(false);

  async function act() {
    if (banner === null) return;
    if (banner.kind === 'expired') {
      // The sign-in dialog lives on the RomM card (Settings › Game stores).
      navigation.goSettings('stores');
      return;
    }
    retrying = true;
    try {
      await reconnectRomm();
    } finally {
      retrying = false;
    }
  }
</script>

{#if banner !== null}
  <div data-testid="romm-banner" data-kind={banner.kind} class="banner" role="status">
    <span><b>{banner.strong}</b> · {banner.text}</span>
    <button
      data-testid="romm-banner-action"
      class="g-btn"
      class:secondary={banner.kind === 'unreachable'}
      disabled={retrying || status.busy}
      onclick={act}
    >
      {(retrying || status.busy) && banner.busyAction ? banner.busyAction : banner.action}
    </button>
  </div>
{/if}

<style>
  /* Prototype `.rm-banner`: a warning edge, one line above the grid. */
  .banner {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    margin: var(--gutter) 0 0;
    padding: 8px 10px 8px 12px;
    border-radius: var(--r-row);
    border-left: 3px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 12%, var(--panel));
    color: var(--text);
    font-size: 13px;
  }

  b {
    color: var(--text-h);
    font-weight: 600;
  }

  .g-btn {
    margin-left: auto;
    flex: none;
    padding: 5px 12px;
    font-size: 12px;
  }
</style>
