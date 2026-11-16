<script lang="ts">
  // A needs-RomM panel (RommStorefront README, "Cloud saves and
  // RetroAchievements", "Details (RomM games)"; prototype `rmNeedsHTML`,
  // `.rm-needs`): the RomM mark, the title, why RomM can't do it right now
  // plus the feature's own line, and the action that fixes the state —
  // Open Game stores (not connected), Retry (can't reach) or Sign in again…
  // (expired). While GRID is still connecting there is no action.
  import { navigation } from '../stores/navigation.svelte';
  import { romm } from '../stores/romm.svelte';
  import RommMark from './RommMark.svelte';
  import RommSignInDialog from './RommSignInDialog.svelte';
  import { retryRomm, rommRetry } from './retry.svelte';
  import { rommFixAction, rommWhy } from './words';

  let {
    title,
    text,
    testId,
    onOpenGameStores = undefined,
  }: {
    title: string;
    /** The feature's line, after the why. */
    text: string;
    testId: string;
    /** Called before Open Game stores navigates (Details closes itself). */
    onOpenGameStores?: () => void;
  } = $props();

  let fix = $derived(rommFixAction(romm.status));
  let signingIn = $state(false);

  function openGameStores() {
    onOpenGameStores?.();
    navigation.goSettings('stores');
  }
</script>

<div class="rm-needs" role="note" data-testid={testId}>
  <RommMark size="lg" />
  <div>
    <b data-testid={`${testId}-title`}>{title}</b>
    <p data-testid={`${testId}-text`}>{rommWhy(romm.status)} {text}</p>
  </div>
  {#if fix !== null}
    <div class="acts">
      {#if fix === 'Retry'}
        <button class="g-btn secondary" data-testid={`${testId}-retry`} disabled={rommRetry.running || romm.status.busy} onclick={() => void retryRomm()}
          >{rommRetry.running ? 'Retrying…' : 'Retry'}</button
        >
      {:else if fix === 'Sign in again…'}
        <button class="g-btn" data-testid={`${testId}-signin`} onclick={() => (signingIn = true)}>Sign in again…</button>
      {:else}
        <button class="g-btn secondary" data-testid={`${testId}-open-stores`} onclick={openGameStores}>Open Game stores</button>
      {/if}
    </div>
  {/if}
</div>

{#if signingIn}
  <RommSignInDialog mode="again" onClose={() => (signingIn = false)} />
{/if}

<style>
  .rm-needs {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    border-left: 3px solid var(--warning);
    background: var(--surface);
  }

  .rm-needs > div:not(.acts) {
    flex: 1 1 auto;
    min-width: 0;
  }

  .rm-needs b {
    display: block;
    color: var(--text-h);
    font-weight: 600;
  }

  .rm-needs p {
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .acts {
    flex: none;
    display: flex;
    gap: 8px;
  }

  .acts .g-btn {
    padding: 6px 14px;
  }
</style>
