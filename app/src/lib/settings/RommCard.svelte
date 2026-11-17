<script lang="ts">
  // Settings › Game stores › RomM (RommStorefront README, "RomM card";
  // prototype `rmCard`): the RomM mark, the sub-line and the pill; then per
  // state the warning, the fields and the actions (`rommCard.ts`), the
  // in-card Disconnect confirm, and the sign-in dialog for Connect RomM…,
  // Edit… and Sign in again…. A connect from here that needs setup opens
  // Choose your systems, which returns to this pane (`connectRomm`,
  // `session.setupReturn = 'settings'`).
  import { api } from '../api';
  import Icon from '../Icon.svelte';
  import type { RailKey } from '../library/rail';
  import { jumpToRail } from '../library/selection.svelte';
  import RommMark from '../romm/RommMark.svelte';
  import RommSignInDialog from '../romm/RommSignInDialog.svelte';
  import { isEmulatorsShelf } from '../romm/platformKey';
  import { retryRomm, rommRetry } from '../romm/retry.svelte';
  import { ROMM_DISCONNECT_CONFIRM, rommToasts, type SignInMode } from '../romm/words';
  import { navigation } from '../stores/navigation.svelte';
  import { disconnectRomm, romm } from '../stores/romm.svelte';
  import { pushToast, TOAST_DURATION_MS } from '../stores/toasts.svelte';
  import { rommCardView, type RommCardActionId } from './rommCard';

  let { active = true }: { active?: boolean } = $props();

  let confirming = $state(false);
  let dialog = $state<SignInMode | null>(null);
  let disconnecting = $state(false);
  let onServer = $state<number | null>(null);

  let status = $derived(romm.status);
  let connected = $derived(romm.status.connection === 'connected');
  let view = $derived(rommCardView(status, onServer, rommRetry.running));

  // Leaving the pane drops the confirm and closes the dialog (its typed
  // secret goes with it).
  $effect(() => {
    if (!active) {
      confirming = false;
      dialog = null;
    }
  });

  // The Games field's server count, read each time the pane comes forward
  // while RomM is connected.
  $effect(() => {
    if (!active || !connected) return;
    let cancelled = false;
    api
      .listPlatforms()
      .then((platforms) => {
        if (cancelled) return;
        onServer = platforms.filter((p) => !isEmulatorsShelf(p)).reduce((n, p) => n + Math.max(0, p.rom_count), 0);
      })
      .catch(() => {
        // The status (Can't reach, Sign-in expired) says why; keep the old count.
      });
    return () => {
      cancelled = true;
    };
  });

  async function openInBrowser() {
    try {
      await api.openServerPage();
    } catch (err) {
      pushToast(err instanceof Error ? err.message : String(err), 'error', TOAST_DURATION_MS);
    }
  }

  function showInLibrary() {
    // `romm:installed` joins `RailKey` with the Library's RomM group (plan 0.3).
    jumpToRail('romm:installed' as RailKey);
    navigation.go('library');
  }

  async function disconnect() {
    disconnecting = true;
    const err = await disconnectRomm();
    disconnecting = false;
    confirming = false;
    if (err === null) pushToast(rommToasts.disconnected, 'success', TOAST_DURATION_MS);
    else pushToast(err, 'error', TOAST_DURATION_MS);
  }

  function run(id: RommCardActionId) {
    switch (id) {
      case 'show':
        showInLibrary();
        break;
      case 'retry':
      case 'reconnect':
        void retryRomm();
        break;
      case 'signin':
        dialog = 'again';
        break;
      case 'edit':
        dialog = 'edit';
        break;
      case 'connect':
        dialog = 'connect';
        break;
      case 'open':
        void openInBrowser();
        break;
      case 'disconnect':
        confirming = true;
        break;
    }
  }
</script>

<article class="st-store" class:off={view.off} data-testid="store-card-romm" data-state={view.state}>
  <header class="st-store-head">
    <RommMark size="lg" />
    <div class="st-store-id">
      <b>RomM</b>
      <span class="g-note" data-testid="store-card-account-romm">{view.account}</span>
    </div>
    <span class="pill {view.pill.tone}" data-testid="store-card-pill-romm">
      {#if view.pill.tone === 'ok'}<Icon name="check" size={12} />{:else if view.pill.tone === 'busy'}<span class="spin"></span>{/if}{view.pill.text}
    </span>
  </header>

  {#if confirming}
    <div class="confirm" data-testid="romm-disconnect-confirm">
      <p><b>{ROMM_DISCONNECT_CONFIRM.title}</b> {ROMM_DISCONNECT_CONFIRM.text}</p>
      <div class="g-actions">
        <button class="g-btn danger" data-testid="romm-disconnect-go" disabled={disconnecting} onclick={disconnect}
          >{ROMM_DISCONNECT_CONFIRM.confirm}</button
        >
        <button class="g-btn secondary" data-testid="romm-disconnect-cancel" disabled={disconnecting} onclick={() => (confirming = false)}
          >{ROMM_DISCONNECT_CONFIRM.cancel}</button
        >
      </div>
    </div>
  {:else}
    {#if view.lede !== null}
      <p class="g-note st-lede" data-testid="romm-card-lede">{view.lede}</p>
    {/if}
    {#if view.warning !== null}
      <p class="warnline" data-testid="romm-card-warning"><b>{view.warning.strong}</b> {view.warning.text}</p>
    {/if}
    {#if view.fields.length > 0}
      <dl class="g-kv">
        {#each view.fields as f (f.key)}
          <div>
            <dt>{f.label}</dt>
            <dd data-testid={`romm-card-field-${f.key}`}>
              {#if f.link}
                <button class="linkish" onclick={openInBrowser}>{f.value}</button>
              {:else}
                {f.value}
              {/if}
            </dd>
          </div>
        {/each}
      </dl>
    {/if}
    <div class="g-actions">
      {#each view.actions as a (a.id)}
        <button
          class="g-btn"
          class:secondary={!a.primary}
          class:destructive={a.destructive}
          data-testid={`romm-card-${a.id}`}
          disabled={a.disabled}
          onclick={() => run(a.id)}>{a.label}</button
        >
      {/each}
    </div>
  {/if}
</article>

{#if dialog !== null}
  <RommSignInDialog mode={dialog} from="settings" onClose={() => (dialog = null)} />
{/if}

<style>
  .st-lede {
    max-width: 640px;
  }

  .st-store {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .st-store.off {
    border-style: dashed;
    border-color: var(--line-strong);
  }

  .st-store-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .st-store-id {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-right: auto;
    min-width: 0;
  }

  .st-store-id b {
    color: var(--text-h);
    font-size: 14px;
  }

  /* The prototype's `.pill-s`. */
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 10px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .pill.ok {
    color: var(--text-h);
  }

  .pill.ok :global(svg) {
    color: var(--primary);
  }

  .pill.warn {
    background: var(--warning);
    border-color: var(--warning);
    color: #1a1a12;
  }

  .st-store :global(.g-kv > div) {
    grid-template-columns: 150px minmax(0, 1fr);
    padding: 6px 0;
  }

  .warnline {
    margin: 0;
    max-width: 640px;
    padding: 8px 12px;
    border-radius: var(--r-row);
    border-left: 3px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    font-size: 13px;
    color: var(--text);
  }

  .warnline b {
    color: var(--text-h);
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 6%, transparent);
  }

  .confirm p {
    margin: 0;
    font-size: 13px;
    color: var(--text);
  }

  .confirm b {
    color: var(--text-h);
  }

  .g-btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
  }

  .g-btn.secondary.destructive {
    color: var(--danger);
  }

  .linkish {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: 13px;
    color: var(--primary);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .spin {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    border-top-color: var(--primary);
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
</style>
