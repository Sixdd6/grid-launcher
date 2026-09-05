<script lang="ts">
  // Settings › Connection (design §10): "server URL, token status, reconnect,
  // disconnect". Reads the session store; the two actions are the same
  // functions the server menu calls. Nothing here can render a secret —
  // the store never holds one.
  import { api } from '../api';
  import { connect, disconnect, retry, session } from '../stores/session.svelte';
  import {
    canConnect,
    credentialStatusLabel,
    OPEN_CONFIG_FOLDER_LABEL,
    reconnectEnabled,
    serverLine,
  } from './connection';

  let configFolderError = $state<string | null>(null);

  // The reference's Settings › Server Connection panel (grid-launcher.py:
  // 1601-1623): Server URL + API Token, then Connect. Collapsed by default —
  // the page's job is status; editing is the exception.
  let editing = $state(false);
  let editServerUrl = $state('');
  let editUsername = $state('');
  let editSecret = $state('');
  let editUseToken = $state(true);

  function openEdit() {
    // Seeded from the store's URL only. The secret is never seeded: the
    // store has never held one and the backend never returns one.
    editServerUrl = session.serverUrl;
    editUsername = session.username;
    editSecret = '';
    editUseToken = true;
    editing = true;
  }

  function closeEdit() {
    editing = false;
    editSecret = '';
  }

  async function submitEdit() {
    // Started, then cleared in the same tick, exactly as `Connect.svelte`
    // submits: the plain secret must not sit in frontend state for the whole
    // network round trip. The promise is kept only to decide the close.
    const pending = connect(editServerUrl, editUseToken ? '' : editUsername, editSecret, editUseToken);
    editSecret = ''; // never keep the plain secret in frontend state
    await pending;
    // `connect` reports through `session.error`; a run that set none
    // succeeded, so the form can close.
    if (session.error === null) editing = false;
  }

  async function handleOpenConfigFolder() {
    configFolderError = null;
    try {
      await api.openConfigFolder();
    } catch (err) {
      configFolderError = err instanceof Error ? err.message : String(err);
    }
  }
</script>

<dl class="rows">
  <div class="row">
    <dt>Server</dt>
    <dd data-testid="settings-connection-url">{serverLine(session.serverUrl)}</dd>
  </div>
  <div class="row">
    <dt>User</dt>
    <dd data-testid="settings-connection-user">{session.username.trim() === '' ? 'Not set' : session.username}</dd>
  </div>
  <div class="row">
    <dt>Credential</dt>
    <dd data-testid="settings-connection-credential">{credentialStatusLabel(session.connected)}</dd>
  </div>
  <div class="row">
    <dt>Status</dt>
    <dd data-testid="settings-connection-status">
      <span class="dot" class:online={session.connected} aria-hidden="true"></span>
      {session.connected ? 'Connected' : 'Not connected'}
    </dd>
  </div>
</dl>

{#if editing}
  <form
    data-testid="settings-connection-edit-form"
    class="edit"
    onsubmit={(e) => {
      e.preventDefault();
      submitEdit();
    }}
  >
    <label>
      Server URL
      <input data-testid="settings-connection-server-url" bind:value={editServerUrl} required />
    </label>
    {#if !editUseToken}
      <label>
        Username
        <input data-testid="settings-connection-username" bind:value={editUsername} autocomplete="username" />
      </label>
    {/if}
    <label>
      {editUseToken ? 'API Token' : 'Password'}
      <input
        data-testid="settings-connection-secret"
        type="password"
        bind:value={editSecret}
        autocomplete="new-password"
        required
      />
    </label>
    <label class="checkbox">
      <input data-testid="settings-connection-use-token" type="checkbox" bind:checked={editUseToken} />
      Use API token
    </label>
    <div class="actions">
      <button
        data-testid="settings-connection-save"
        type="submit"
        disabled={session.busy || !canConnect(editServerUrl, editSecret)}
      >
        {session.busy ? 'Connecting…' : 'Connect'}
      </button>
      <button data-testid="settings-connection-cancel" type="button" class="secondary" onclick={closeEdit}>
        Cancel
      </button>
    </div>
    {#if session.error}
      <p data-testid="settings-connection-edit-error" class="error" role="alert">{session.error}</p>
    {/if}
  </form>
{/if}

{#if !session.connected && session.lastError}
  <p data-testid="settings-connection-error" class="error" role="alert">{session.lastError}</p>
{/if}

<div class="actions">
  <button
    data-testid="settings-connection-edit"
    class="secondary"
    onclick={() => (editing ? closeEdit() : openEdit())}
  >
    {editing ? 'Close editor' : 'Edit connection'}
  </button>
  <button
    data-testid="settings-connection-reconnect"
    disabled={!reconnectEnabled(session.connected, session.busy)}
    onclick={() => {
      retry();
    }}
  >
    {session.busy ? 'Reconnecting…' : 'Reconnect'}
  </button>
  <button
    data-testid="settings-connection-disconnect"
    class="secondary"
    onclick={() => {
      disconnect();
    }}
  >
    Disconnect
  </button>
  <button
    data-testid="settings-open-config-folder"
    class="secondary"
    onclick={() => {
      handleOpenConfigFolder();
    }}
  >
    {OPEN_CONFIG_FOLDER_LABEL}
  </button>
</div>

{#if configFolderError}
  <p data-testid="settings-config-folder-error" class="error" role="alert">{configFolderError}</p>
{/if}

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
  }

  .row {
    display: flex;
    align-items: baseline;
    gap: 12px;
    font-size: 13px;
  }

  dt {
    flex: 0 0 180px;
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    min-width: 0;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--danger);
    vertical-align: middle;
  }

  .dot.online {
    background: var(--success);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .actions button.secondary {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .edit {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 420px;
    margin: 16px 0;
  }

  .edit label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  .edit label.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .edit input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .edit input[type='checkbox'] {
    width: auto;
    padding: 0;
    accent-color: var(--primary);
  }

  .edit input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
</style>
