<script lang="ts">
  // RomM › Installed's one-line banner (RommStorefront README "RomM ›
  // Installed"; prototype `rmBanner`): Can't reach RomM · Retry (Retrying…),
  // or RomM sign-in expired · Sign in again…, which opens the sign-in
  // dialog. None while not connected: the rail's `off` flag and the header
  // chip say it, as for a store.
  import type { RommStatus } from '../../api';
  import RommSignInDialog from '../RommSignInDialog.svelte';
  import { retryRomm, rommRetry } from '../retry.svelte';
  import { rommBanner } from '../words';

  let { status }: { status: RommStatus } = $props();

  let banner = $derived(rommBanner(status));
  let signingIn = $state(false);
  let busy = $derived(rommRetry.running || status.busy);

  function act() {
    if (banner === null) return;
    if (banner.kind === 'expired') signingIn = true;
    else void retryRomm();
  }
</script>

{#if banner !== null}
  <div data-testid="romm-banner" data-kind={banner.kind} class="banner" role="status">
    <span><b>{banner.strong}</b> · {banner.text}</span>
    <button
      data-testid="romm-banner-action"
      class="g-btn"
      class:secondary={banner.kind === 'unreachable'}
      disabled={banner.kind === 'unreachable' && busy}
      onclick={act}
    >
      {busy && banner.busyAction ? banner.busyAction : banner.action}
    </button>
  </div>
{/if}

{#if signingIn}
  <RommSignInDialog mode="again" from="settings" onClose={() => (signingIn = false)} />
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
