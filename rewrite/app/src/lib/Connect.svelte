<script lang="ts">
  import { session, connect } from './stores/session.svelte';
  let serverUrl = $state('');
  let username = $state('');
  let secret = $state('');
  let useToken = $state(true);
</script>

<form
  class="connect"
  onsubmit={(e) => {
    e.preventDefault();
    connect(serverUrl, username, secret, useToken);
    secret = ''; // never keep the plain secret in frontend state
  }}
>
  <h1>Connect to RomM</h1>
  <label>Server URL <input bind:value={serverUrl} placeholder="https://romm.example" required /></label>
  <label>Username <input bind:value={username} autocomplete="username" required={!useToken} /></label>
  <label>
    {useToken ? 'API token' : 'Password'}
    <input bind:value={secret} type="password" autocomplete="current-password" required />
  </label>
  <label class="mode"><input type="checkbox" bind:checked={useToken} /> Use API token</label>
  <button disabled={session.busy}>{session.busy ? 'Connecting…' : 'Connect'}</button>
  {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
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

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 14px;
  }
</style>
