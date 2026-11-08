<script lang="ts">
  // Settings › RetroAchievements (design §10: "current form"), moved out of
  // Emulators.svelte (task-12-brief.md).
  import { api, type RaFanOutRow, type RaStatus } from '../api';
  import {
    canLogin,
    canSubmit,
    CREDENTIALS_CLEARED_TOAST,
    fanOutSummary,
    LOGIN_MISSING_FIELDS_TOAST,
    loginFailedToast,
    loginToast,
    rommSyncNotice,
    statusLabel,
  } from '../emulators/retroachievements';
  import { pushToast } from '../stores/toasts.svelte';

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
      raStatus = await api.getRetroachievementsStatus();
      raUsername = raStatus.username;
    } catch (err) {
      raError = errorMessage(err);
    }
  }

  // U6: a saved username also goes to the RomM account (username only,
  // never the token). Fire and forget: the local save already succeeded,
  // and a RomM failure is a short notice, never an error on this form.
  function syncUsernameToRomm() {
    api
      .syncRaUsernameToRomm()
      .then((outcome) => {
        const notice = rommSyncNotice(outcome);
        if (notice) pushToast(notice, 'error');
      })
      .catch((err) => {
        const notice = rommSyncNotice({ kind: 'failed', error: errorMessage(err) });
        if (notice) pushToast(notice, 'error');
      });
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
      syncUsernameToRomm();
    } catch (err) {
      raError = errorMessage(err);
    } finally {
      raSavePending = false;
    }
  }

  async function handleRaLogin() {
    if (!canLogin(raUsername, raPassword)) {
      pushToast(LOGIN_MISSING_FIELDS_TOAST, 'error');
      return;
    }
    raError = null;
    raResultLine = null;
    raLoginPending = true;
    try {
      const result = await api.retroachievementsLogin(raUsername, raPassword);
      raPassword = '';
      raToken = '';
      raResultLine = fanOutSummary(result.fan_out);
      pushToast(loginToast(result.username));
      await refreshRaStatus();
      syncUsernameToRomm();
    } catch (err) {
      raPassword = '';
      const message = errorMessage(err);
      raError = message;
      pushToast(loginFailedToast(message), 'error');
    } finally {
      raLoginPending = false;
    }
  }

  async function handleRaClear() {
    raError = null;
    raResultLine = null;
    raClearPending = true;
    try {
      await api.clearRetroachievementsCredentials();
      raPassword = '';
      raToken = '';
      pushToast(CREDENTIALS_CLEARED_TOAST);
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
