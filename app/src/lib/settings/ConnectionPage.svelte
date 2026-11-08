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

  let { active = true }: { active?: boolean } = $props();

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
    // A failed edit-connect leaves `session.error` set; clear it so the old
    // message does not greet the user on reopen.
    session.error = null;
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

  // Panes and views switch with `hidden` and never unmount (Settings.svelte,
  // Shell.svelte), so nothing else would drop a typed secret when the user
  // navigates away mid-edit. `closeEdit` clears it.
  $effect(() => {
    if (!active) closeEdit();
  });

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

<dl class="g-kv">
  <div>
    <dt>Server</dt>
    <dd data-testid="settings-connection-url">{serverLine(session.serverUrl)}</dd>
  </div>
  <div>
    <dt>User</dt>
    <dd data-testid="settings-connection-user">{session.username.trim() === '' ? 'Not set' : session.username}</dd>
  </div>
  <div>
    <dt>Credential</dt>
    <dd data-testid="settings-connection-credential">{credentialStatusLabel(session.connected)}</dd>
  </div>
  <div>
    <dt>Status</dt>
    <dd data-testid="settings-connection-status">
      <span class="dot" class:online={session.connected} aria-hidden="true"></span>
      {session.connected ? 'Connected' : 'Not connected'}
    </dd>
  </div>
</dl>

{#if editing}
  <form
    id="settings-connection-edit-form"
    data-testid="settings-connection-edit-form"
    class="g-form"
    onsubmit={(e) => {
      e.preventDefault();
      submitEdit();
    }}
  >
    <label>
      Server URL
      <input class="g-field" data-testid="settings-connection-server-url" bind:value={editServerUrl} required />
    </label>
    {#if !editUseToken}
      <label>
        Username
        <input class="g-field" data-testid="settings-connection-username" bind:value={editUsername} autocomplete="username" />
      </label>
    {/if}
    <label>
      {editUseToken ? 'API Token' : 'Password'}
      <input
        class="g-field"
        data-testid="settings-connection-secret"
        type="password"
        bind:value={editSecret}
        autocomplete="new-password"
        required
      />
    </label>
    <label class="g-check">
      <input data-testid="settings-connection-use-token" type="checkbox" bind:checked={editUseToken} />
      Use API token
    </label>
    <div class="g-actions">
      <button
        class="g-btn"
        data-testid="settings-connection-save"
        type="submit"
        disabled={session.busy || !canConnect(editServerUrl, editSecret)}
      >
        {session.busy ? 'Connecting…' : 'Connect'}
      </button>
      <button data-testid="settings-connection-cancel" type="button" class="g-btn secondary" onclick={closeEdit}>
        Cancel
      </button>
    </div>
    {#if session.error}
      <p data-testid="settings-connection-edit-error" class="g-error" role="alert">{session.error}</p>
    {/if}
  </form>
{/if}

{#if !session.connected && session.lastError}
  <p data-testid="settings-connection-error" class="g-error" role="alert">{session.lastError}</p>
{/if}

<div class="g-actions">
  <button
    data-testid="settings-connection-edit"
    class="g-btn secondary"
    aria-expanded={editing}
    onclick={() => (editing ? closeEdit() : openEdit())}
  >
    {editing ? 'Close editor' : 'Edit connection'}
  </button>
  <button
    class="g-btn"
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
    class="g-btn secondary"
    onclick={() => {
      disconnect();
    }}
  >
    Disconnect
  </button>
  <button
    data-testid="settings-open-config-folder"
    class="g-btn secondary"
    onclick={() => {
      handleOpenConfigFolder();
    }}
  >
    {OPEN_CONFIG_FOLDER_LABEL}
  </button>
</div>

{#if configFolderError}
  <p data-testid="settings-config-folder-error" class="g-error" role="alert">{configFolderError}</p>
{/if}

<style>
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--danger);
  }

  .dot.online {
    background: var(--success);
  }
</style>
