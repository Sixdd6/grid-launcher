<script lang="ts">
  // The RomM sign-in form (RommStorefront README, "Sign-in dialog
  // (RommSignInForm)"; prototype `rmSignInFields` / `rmSubmit`): Server URL,
  // Use an API token, Username (password mode, or read-only in Sign in
  // again), API token / Password, the help line, the red error line, then
  // Cancel and the submit button. The card's dialog (Connect, Edit, Sign in
  // again) and the welcome's RomM row both use it; the title and lede are
  // the caller's (`signInCopy`).
  //
  // The secret never outlives a submit: it is handed to the connect and
  // cleared in the same tick, so it does not sit in `$state` for the network
  // round trip, and a failed connect asks for it again. It is never
  // prefilled: the backend never returns one.
  import { untrack } from 'svelte';
  import type { RommStatus } from '../api';
  import { connectRomm, romm, type ConnectFrom } from '../stores/romm.svelte';
  import { canSignIn, signInError, signInStart, usernameToSend, type SignInStart } from './signIn';
  import { SIGN_IN_FIELDS, signInCopy, type SignInMode } from './words';

  let {
    mode,
    from,
    start = undefined,
    testId = 'romm-signin',
    submitLabel = undefined,
    autofocus = true,
    onConnected = undefined,
    onCancel = undefined,
    connect = connectRomm,
  }: {
    /** Connect, Edit (filled in) or Sign in again (server and user read-only). */
    mode: SignInMode;
    /** Where Choose your systems returns after a connect that needs setup
     *  (`connectRomm`'s `from`): `'settings'` (the card) or `'welcome'`. */
    from: ConnectFrom;
    /** What the fields start with; defaults to `signInStart(romm.status, mode)`.
     *  Read once, when the form mounts. */
    start?: SignInStart;
    /** The test id prefix: `<testId>-url`, `-use-token`, `-username`,
     *  `-secret`, `-error`, `-cancel`, `-submit`, and `<testId>` on the form. */
    testId?: string;
    /** The submit button's word; defaults to the mode's (`signInCopy`). */
    submitLabel?: string;
    /** Puts the caret in the first field to fill when the form mounts. */
    autofocus?: boolean;
    /** After a successful connect, with the new status. */
    onConnected?: (status: RommStatus) => void;
    /** Shows Cancel, which calls it. */
    onCancel?: () => void;
    /** The connect call; replaced in tests. Resolves to the backend's error
     *  text, or `null` on success. */
    connect?: (serverUrl: string, username: string, secret: string, useToken: boolean, from: ConnectFrom) => Promise<string | null>;
  } = $props();

  const initial = untrack(() => start ?? signInStart(romm.status, mode));
  const storedUsername = untrack(() => romm.status.username || initial.username);

  let serverUrl = $state(initial.serverUrl);
  let username = $state(initial.username);
  let useToken = $state(initial.useToken);
  let secret = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);

  let urlEl = $state<HTMLInputElement | null>(null);
  let secretEl = $state<HTMLInputElement | null>(null);

  let readOnly = $derived(initial.accountReadOnly);
  let ready = $derived(canSignIn({ serverUrl, username, secret, useToken }));
  let label = $derived(busy ? SIGN_IN_FIELDS.busy : (submitLabel ?? signInCopy(mode).button));

  $effect(() => {
    if (!autofocus) return;
    untrack(() => {
      if (readOnly || serverUrl.trim() !== '') secretEl?.focus();
      else urlEl?.focus();
    });
  });

  // Never leave a typed secret behind when the form goes away.
  $effect(() => () => {
    secret = '';
  });

  function setUseToken(on: boolean) {
    useToken = on;
    error = null;
  }

  async function submit() {
    if (!ready || busy) return;
    busy = true;
    error = null;
    const url = serverUrl.trim();
    const user = usernameToSend({ username, useToken }, mode, storedUsername);
    const token = useToken;
    const pending = connect(url, user, secret, token, from);
    secret = ''; // never keep the plain secret in frontend state
    let raw: string | null;
    try {
      raw = await pending;
    } finally {
      busy = false;
    }
    if (raw === null) {
      onConnected?.(romm.status);
      return;
    }
    error = signInError(raw, url, token);
    secretEl?.focus();
  }
</script>

<form
  class="signin"
  data-testid={testId}
  onsubmit={(e) => {
    e.preventDefault();
    void submit();
  }}
>
  <label class="st-field">
    <span>{SIGN_IN_FIELDS.serverUrl}</span>
    <input
      class="g-field"
      data-testid={`${testId}-url`}
      bind:this={urlEl}
      bind:value={serverUrl}
      placeholder={SIGN_IN_FIELDS.serverPlaceholder}
      autocomplete="off"
      spellcheck="false"
      disabled={readOnly || busy}
    />
  </label>
  {#if !readOnly}
    <label class="check">
      <input
        type="checkbox"
        data-testid={`${testId}-use-token`}
        checked={useToken}
        disabled={busy}
        onchange={(e) => setUseToken((e.currentTarget as HTMLInputElement).checked)}
      />
      {SIGN_IN_FIELDS.useToken}
    </label>
  {/if}
  {#if readOnly || !useToken}
    <label class="st-field">
      <span>{SIGN_IN_FIELDS.username}</span>
      <input
        class="g-field"
        data-testid={`${testId}-username`}
        bind:value={username}
        autocomplete="username"
        spellcheck="false"
        disabled={readOnly || busy}
      />
    </label>
  {/if}
  <label class="st-field">
    <span>{useToken ? SIGN_IN_FIELDS.token : SIGN_IN_FIELDS.password}</span>
    <input
      class="g-field"
      type="password"
      data-testid={`${testId}-secret`}
      bind:this={secretEl}
      bind:value={secret}
      autocomplete="off"
      disabled={busy}
    />
  </label>
  <span class="help">{useToken ? SIGN_IN_FIELDS.tokenHelp : SIGN_IN_FIELDS.passwordHelp}</span>
  {#if error}<p class="errline" role="alert" data-testid={`${testId}-error`}>{error}</p>{/if}
  <div class="foot">
    {#if onCancel}
      <button type="button" class="g-btn secondary" data-testid={`${testId}-cancel`} onclick={() => onCancel?.()}>Cancel</button>
    {/if}
    <button type="submit" class="g-btn" data-testid={`${testId}-submit`} disabled={!ready || busy}>{label}</button>
  </div>
</form>

<style>
  .signin {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .st-field {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }

  .st-field > span {
    color: var(--text-muted);
  }

  .st-field input {
    width: 100%;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .check input {
    margin: 0;
    accent-color: var(--primary);
  }

  .help {
    display: block;
    margin-top: -4px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .errline {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
  }

  .foot {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
</style>
