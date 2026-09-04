<script lang="ts">
  // Settings › RetroAchievements (design §10: "current form"), moved out of
  // Emulators.svelte (task-12-brief.md).
  import { api, type RaFanOutRow, type RaStatus } from '../api';
  import { canSubmit, fanOutSummary, statusLabel } from '../emulators/retroachievements';

  let { active = true }: { active?: boolean } = $props();

  let raStatus = $state<RaStatus | null>(null);
  let raUsername = $state('');
  // The token field is write-only: it starts empty on every mount and is
  // never bound to a value read back from the backend, which never returns
  // the token in the first place (RaStatus carries only `token_present`).
  let raToken = $state('');
  let raError = $state<string | null>(null);
  let raResultLine = $state<string | null>(null);
  let raSavePending = $state(false);
  let raClearPending = $state(false);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshRaStatus() {
    try {
      raStatus = await api.getRetroachievementsStatus();
      raUsername = raStatus.username;
    } catch (err) {
      raError = errorMessage(err);
    }
  }

  async function handleRaSave() {
    if (!canSubmit(raUsername, raToken)) return;
    raError = null;
    raResultLine = null;
    raSavePending = true;
    try {
      const rows: RaFanOutRow[] = await api.setRetroachievementsCredentials(raUsername, raToken);
      raToken = '';
      raResultLine = fanOutSummary(rows);
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raSavePending = false;
    }
  }

  async function handleRaClear() {
    raError = null;
    raResultLine = null;
    raClearPending = true;
    try {
      await api.clearRetroachievementsCredentials();
      raToken = '';
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raClearPending = false;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshRaStatus();
  });
</script>

<p class="muted" data-testid="ra-status">{statusLabel(raStatus)}</p>
<form
  onsubmit={(e) => {
    e.preventDefault();
    handleRaSave();
  }}
>
  <label>
    Username
    <input data-testid="ra-username" bind:value={raUsername} autocomplete="username" />
  </label>
  <label>
    Token
    <input
      data-testid="ra-token"
      type="password"
      bind:value={raToken}
      autocomplete="new-password"
    />
  </label>
  {#if raError}<p data-testid="ra-error" class="error" role="alert">{raError}</p>{/if}
  {#if raResultLine}<p class="hint">{raResultLine}</p>{/if}
  <div class="form-actions">
    <button
      data-testid="ra-save"
      type="submit"
      disabled={raSavePending || !canSubmit(raUsername, raToken)}
    >
      {raSavePending ? 'Saving…' : 'Save'}
    </button>
    <button
      data-testid="ra-clear"
      type="button"
      onclick={handleRaClear}
      disabled={raClearPending}
    >
      {raClearPending ? 'Clearing…' : 'Clear'}
    </button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .form-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .form-actions button[type='button'] {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
