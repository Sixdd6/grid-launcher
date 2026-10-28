<script lang="ts">
  import { api } from './api';
  import { session, connect } from './stores/session.svelte';
  import { pickFolder } from './pickers';
  import { untrack } from 'svelte';
  import { connectPrefill, type ConnectPrefill } from './connectPrefill';
  import { canConnect, libraryField } from './connectForm';
  import Icon from './Icon.svelte';

  // Seeded from the restored config (blank when there is none): a Python
  // import carries `server_url`/`username` across without a credential, so
  // the only thing left to type is the token. After a rejected credential
  // (Q6) the same fields come back with the credential kind and a reason
  // line. The secret is never seeded.
  let { prefill = connectPrefill(session) }: { prefill?: ConnectPrefill } = $props();
  // The form owns its fields after the first render: this component mounts
  // fresh each time the phase turns to 'none', so the initial value is the one.
  const initial = untrack(() => prefill);
  let serverUrl = $state(initial.serverUrl);
  let username = $state(initial.username);
  let secret = $state('');
  let useToken = $state(initial.useToken);
  let secretEl = $state<HTMLInputElement | null>(null);

  // A rejected credential is the one field left to fix: put the caret there.
  $effect(() => {
    if (initial.reason !== null) secretEl?.focus();
  });

  // The library folder (`connectForm.ts`): asked for, pre-filled with the
  // suggested folder and required only while none is stored — nothing can
  // install without one. `null` until the stored path is known, and when the
  // form does not ask, so the field never flashes on a reconnect.
  let libraryPath = $state<string | null>(null);
  $effect(() => {
    Promise.all([api.getLibraryPath(), api.suggestedLibraryPath()])
      .then(([stored, suggested]) => {
        const field = libraryField(stored, suggested);
        libraryPath = field.show ? field.initial : null;
      })
      .catch(() => {
        // Unknown: ask, with no suggestion. A folder typed here is stored
        // before connecting either way.
        libraryPath = '';
      });
  });

  // Set only for the `setLibraryPath` await below, which runs BEFORE
  // `connect` sets `session.busy`: without this, a double click in that gap
  // could fire two submits.
  let submitting = $state(false);
  let libraryError = $state<string | null>(null);

  let ready = $derived(canConnect({ serverUrl, username, secret, useToken, libraryPath }));

  async function browseLibraryPath() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) libraryPath = picked;
  }

  async function submit() {
    if (!ready) return;
    submitting = true;
    libraryError = null;
    try {
      // Written BEFORE the connect, exactly as `FirstRunDialog`'s "Save and
      // Continue" persists all three values before the app tries the server
      // (grid-launcher.py:1689): a rejected credential must not lose the path
      // the user just typed. Safe in either order — `SessionManager::connect`
      // re-reads config.toml and overwrites only `server_url`/`username`.
      if (libraryPath !== null) {
        try {
          await api.setLibraryPath(libraryPath.trim());
        } catch (e) {
          libraryError = `That folder can't be used: ${e}`;
          return;
        }
      }
      // Fire-and-forget: `connect` sets `session.busy` synchronously, and
      // `secret` clears in the same tick rather than staying in frontend
      // state for the whole network round trip. Token auth identifies the
      // account by itself; the username input only exists for Basic mode,
      // so never send a stale one alongside a token.
      connect(serverUrl, useToken ? '' : username, secret, useToken);
      secret = ''; // never keep the plain secret in frontend state
    } finally {
      submitting = false;
    }
  }
</script>

<form
  class="connect"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  <header>
    <span class="mark"><Icon name="grid" size={28} /></span>
    <h1>Connect to RomM</h1>
    <p class="lede">GRID plays the games on your RomM server.</p>
  </header>
  {#if initial.reason !== null}<p data-testid="connect-reason" class="reason" role="status">{initial.reason}</p>{/if}
  <label>Server URL <input data-testid="connect-server-url" bind:value={serverUrl} placeholder="https://romm.example" required /></label>
  {#if !useToken}
    <label>Username <input data-testid="connect-username" bind:value={username} autocomplete="username" required /></label>
  {/if}
  <label>
    {useToken ? 'API token' : 'Password'}
    <input data-testid="connect-secret" bind:this={secretEl} bind:value={secret} type="password" autocomplete="current-password" required />
  </label>
  <label class="mode"><input data-testid="connect-use-token" type="checkbox" bind:checked={useToken} /> Use API token</label>
  {#if libraryPath !== null}
    <label>
      Library folder
      <span class="path-row">
        <input data-testid="connect-library-path" bind:value={libraryPath} required />
        <button
          type="button"
          data-testid="connect-library-path-browse"
          class="browse"
          onclick={browseLibraryPath}
        >
          Browse…
        </button>
      </span>
      <span class="hint">Games, emulators and saves go here.</span>
    </label>
  {/if}
  <button data-testid="connect-submit" class="primary" disabled={!ready || session.busy || submitting}>
    {session.busy ? 'Connecting…' : 'Connect'}
  </button>
  {#if libraryError}<p data-testid="connect-library-error" class="error" role="alert">{libraryError}</p>{/if}
  {#if session.error}<p data-testid="connect-error" class="error" role="alert">{session.error}</p>{/if}
</form>

<style>
  /* Design system "First open": one column at most 360px wide, 32px above,
     16px between fields; one primary button, disabled until complete.
     `#app` is exactly one viewport tall, so the form scrolls itself rather
     than spilling on a short window. */
  .connect {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(360px, 100%);
    margin: 0 auto;
    padding: 32px 24px;
    box-sizing: border-box;
    min-height: 0;
    overflow-y: auto;
  }

  header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
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

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 14px;
    color: var(--text);
  }

  label.mode {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .path-row {
    display: flex;
    gap: 8px;
  }

  .path-row input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .hint {
    font-size: 12px;
    color: var(--text-muted);
  }

  input:not([type]),
  input[type='password'] {
    font: inherit;
    font-size: 13px;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  input:focus-visible,
  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  button {
    font: inherit;
    border-radius: var(--r-chip);
    cursor: pointer;
  }

  .primary {
    padding: 10px 16px;
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    transition: background var(--m-fast) ease;
  }

  .primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .browse {
    padding: 8px 14px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    white-space: nowrap;
  }

  .browse:hover {
    background: var(--surface);
  }

  /* A calm notice, not an error (design system Notice): the neutral
     `--surface` fill plus a `--primary` edge. Plain text colour keeps
     contrast high in both themes; `--danger` is reserved for `.error`. It is
     not focusable, so the secret field keeps focus. */
  .reason {
    margin: 0;
    padding: 10px 12px;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
    font-size: 14px;
    line-height: 1.4;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 14px;
  }
</style>
