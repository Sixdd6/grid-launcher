<script lang="ts">
  // Settings › Connection (design §10): "server URL, token status, reconnect,
  // disconnect". Reads the session store; the two actions are the same
  // functions the server menu calls. Nothing here can render a secret —
  // the store never holds one.
  import { disconnect, retry, session } from '../stores/session.svelte';
  import { api } from '../api';
  import { credentialStatusLabel, OPEN_CONFIG_FOLDER_LABEL, reconnectEnabled, serverLine } from './connection';

  let configFolderError = $state<string | null>(null);

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

{#if !session.connected && session.lastError}
  <p data-testid="settings-connection-error" class="error" role="alert">{session.lastError}</p>
{/if}

<div class="actions">
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
</style>
