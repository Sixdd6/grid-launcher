<script lang="ts">
  import { api } from './api';
  import { session, connect } from './stores/session.svelte';
  import { pickFolder } from './pickers';
  let serverUrl = $state('');
  let username = $state('');
  let secret = $state('');
  let useToken = $state(true);
  // `FirstRunDialog` asks for the library path alongside the server details
  // (dialogs.py:133-146). A folder picker is a separate plan; this is the
  // free-text half only.
  let libraryPath = $state('');
  // Set only for the `setLibraryPath` await below, which runs BEFORE
  // `connect` sets `session.busy`: without this, a double click in that gap
  // could fire two submits.
  let submitting = $state(false);

  async function browseLibraryPath() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) libraryPath = picked;
  }

  async function submit() {
    submitting = true;
    try {
      // Written BEFORE the connect, exactly as `FirstRunDialog`'s "Save and
      // Continue" persists all three values before the app tries the server
      // (grid-launcher.py:1689): a rejected credential must not lose the path
      // the user just typed. Safe in either order — `SessionManager::connect`
      // re-reads config.toml and overwrites only `server_url`/`username`
      // (crates/grid-core/src/session.rs:124-127).
      const path = libraryPath.trim();
      if (path !== '') {
        try {
          await api.setLibraryPath(path);
        } catch {
          // Best-effort: a path that cannot be stored must not block the
          // connect, and Settings can set it afterwards.
        }
      }
      // Fire-and-forget, same as before: `connect` sets `session.busy`
      // synchronously, and `secret` clears in the same tick rather than
      // staying in frontend state for the whole network round trip.
      // Token auth identifies the account by itself; the username input only
      // exists for Basic mode, so never send a stale one alongside a token.
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
  <h1>Connect to RomM</h1>
  <label>Server URL <input data-testid="connect-server-url" bind:value={serverUrl} placeholder="https://romm.example" required /></label>
  {#if !useToken}
    <label>Username <input data-testid="connect-username" bind:value={username} autocomplete="username" required /></label>
  {/if}
  <label>
    {useToken ? 'API token' : 'Password'}
    <input data-testid="connect-secret" bind:value={secret} type="password" autocomplete="current-password" required />
  </label>
  <label class="mode"><input data-testid="connect-use-token" type="checkbox" bind:checked={useToken} /> Use API token</label>
  <label>
    Library Path
    <span class="path-row">
      <input data-testid="connect-library-path" bind:value={libraryPath} placeholder="/home/you/Games" />
      <button
        type="button"
        data-testid="connect-library-path-browse"
        class="browse"
        onclick={browseLibraryPath}
      >
        Browse…
      </button>
    </span>
  </label>
  <button data-testid="connect-submit" disabled={session.busy || submitting}>{session.busy ? 'Connecting…' : 'Connect'}</button>
  {#if session.error}<p data-testid="connect-error" class="error" role="alert">{session.error}</p>{/if}
</form>

<style>
  .connect {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: min(360px, 100%);
    margin: 0 auto;
    padding: 32px 24px;
    box-sizing: border-box;
  }

  h1 {
    font-size: 28px;
    margin: 0 0 8px;
    text-align: center;
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

  input:not([type]),
  input[type='password'] {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  button {
    font: inherit;
    padding: 10px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .browse {
    padding: 8px 14px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    white-space: nowrap;
  }

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 14px;
  }
</style>
