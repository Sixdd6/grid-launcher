<script lang="ts">
  // Settings › RetroAchievements (design §10: "current form"), moved out of
  // Emulators.svelte (task-12-brief.md).
  import { type RaStatus } from '../api';
  import { canLogin, canSubmit, statusLabel } from '../emulators/retroachievements';
  import { clearRaCredentials, fetchRaStatus, raLogin, saveRaToken } from './raActions';

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
  // Write-only, like the token field: never seeded, never read back, blanked
  // the moment the login resolves either way.
  let raPassword = $state('');
  let raLoginPending = $state(false);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshRaStatus() {
    try {
      raStatus = await fetchRaStatus();
      raUsername = raStatus.username;
    } catch (err) {
      raError = errorMessage(err);
    }
  }

  // The commands, their cards and the RomM username sync are raActions.ts's.
  async function handleRaSave() {
    if (!canSubmit(raUsername, raToken)) return;
    raError = null;
    raResultLine = null;
    raSavePending = true;
    try {
      const line = await saveRaToken(raUsername, raToken);
      raToken = '';
      raResultLine = line;
      await refreshRaStatus();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raSavePending = false;
    }
  }

  async function handleRaLogin() {
    if (!canLogin(raUsername, raPassword)) {
      // Sends only the "fill in both fields" card; the form stays as it is.
      void raLogin(raUsername, raPassword);
      return;
    }
    raError = null;
    raResultLine = null;
    raLoginPending = true;
    const result = await raLogin(raUsername, raPassword);
    raPassword = '';
    if (result.kind === 'ok') {
      raToken = '';
      raResultLine = result.line;
      await refreshRaStatus();
    } else if (result.kind === 'failed') {
      raError = result.error;
    }
    raLoginPending = false;
  }

  async function handleRaClear() {
    raError = null;
    raResultLine = null;
    raClearPending = true;
    try {
      await clearRaCredentials();
      raPassword = '';
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

  // Panes never unmount (Settings.svelte switches with `hidden`), so a
  // typed-but-unsubmitted password would otherwise sit in state and the DOM
  // indefinitely. Mirrors ConnectionPage.svelte's `closeEdit` on `!active`.
  $effect(() => {
    if (!active) {
      raPassword = '';
      raToken = '';
    }
  });
</script>

<p class="g-note" data-testid="ra-status">{statusLabel(raStatus)}</p>
<form
  class="g-form"
  onsubmit={(e) => {
    e.preventDefault();
    handleRaSave();
  }}
>
  <label>
    Username
    <input class="g-field" data-testid="ra-username" bind:value={raUsername} autocomplete="username" />
  </label>
  <label>
    Password
    <input
      class="g-field"
      data-testid="ra-password"
      type="password"
      bind:value={raPassword}
      autocomplete="current-password"
    />
  </label>
  <label>
    Token
    <input
      class="g-field"
      data-testid="ra-token"
      type="password"
      bind:value={raToken}
      autocomplete="new-password"
    />
  </label>
  {#if raError}<p data-testid="ra-error" class="g-error" role="alert">{raError}</p>{/if}
  {#if raResultLine}<p class="g-note">{raResultLine}</p>{/if}
  <div class="g-actions">
    <button
      class="g-btn secondary"
      data-testid="ra-login"
      type="button"
      onclick={handleRaLogin}
      disabled={raLoginPending}
    >
      {raLoginPending ? 'Logging in…' : 'Log In'}
    </button>
    <button
      class="g-btn"
      data-testid="ra-save"
      type="submit"
      disabled={raSavePending || !canSubmit(raUsername, raToken)}
    >
      {raSavePending ? 'Saving…' : 'Save'}
    </button>
    <button
      class="g-btn secondary"
      data-testid="ra-clear"
      type="button"
      onclick={handleRaClear}
      disabled={raClearPending}
    >
      {raClearPending ? 'Clearing…' : 'Clear'}
    </button>
  </div>
</form>
