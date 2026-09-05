<script lang="ts">
  // Settings › Cloud saves (design §10: "current cloud settings form"), moved
  // out of Emulators.svelte (task-19-brief.md). The refresh is gated on the
  // Settings view being visible, as the Emulators view gated it before.
  import { api, type CloudSettings } from '../api';

  let { active = true }: { active?: boolean } = $props();

  let cloudSettings = $state<CloudSettings | null>(null);
  let cloudSettingsError = $state<string | null>(null);
  let cloudSettingsSavedLine = $state<string | null>(null);
  let cloudSettingsPending = $state(false);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshCloudSettings() {
    try {
      cloudSettings = await api.cloudSettings();
      cloudSettingsError = null;
    } catch (err) {
      cloudSettingsError = errorMessage(err);
    }
  }

  async function handleCloudSettingsSave() {
    if (!cloudSettings) return;
    cloudSettingsError = null;
    cloudSettingsSavedLine = null;
    cloudSettingsPending = true;
    try {
      await api.setCloudSettings(cloudSettings);
      cloudSettingsSavedLine = 'Saved.';
      await refreshCloudSettings();
    } catch (err) {
      cloudSettingsError = errorMessage(err);
    } finally {
      cloudSettingsPending = false;
    }
  }

  $effect(() => {
    if (!active) return;
    refreshCloudSettings();
  });
</script>

{#if cloudSettings}
  <form
    onsubmit={(e) => {
      e.preventDefault();
      handleCloudSettingsSave();
    }}
  >
    <label class="checkbox">
      <input
        data-testid="cloud-settings-download-on-launch"
        type="checkbox"
        bind:checked={cloudSettings.download_on_launch}
      />
      Restore cloud saves before launch
    </label>
    <label class="checkbox">
      <input
        data-testid="cloud-settings-upload-on-exit"
        type="checkbox"
        bind:checked={cloudSettings.upload_on_exit}
      />
      Upload cloud saves after exit
    </label>
    <label class="checkbox">
      <input
        data-testid="cloud-settings-skip-if-local-newer"
        type="checkbox"
        bind:checked={cloudSettings.skip_if_local_newer}
      />
      Skip download when the local save is newer
    </label>
    <label>
      Upload delay (seconds)
      <input
        data-testid="cloud-settings-upload-delay"
        type="number"
        min="0"
        max="60"
        bind:value={cloudSettings.upload_delay_seconds}
      />
    </label>
    <label>
      Save retention limit
      <input
        data-testid="cloud-settings-retention-limit"
        type="number"
        min="1"
        bind:value={cloudSettings.retention_limit}
      />
    </label>
    <!-- grid-launcher.py:1733-1738, verbatim. -->
    <p data-testid="cloud-settings-autosync-hint" class="hint">
      Auto-sync applies to emulator-based games and uses the latest server save record only.
    </p>
    {#if cloudSettingsError}<p data-testid="cloud-settings-error" class="error" role="alert">{cloudSettingsError}</p>{/if}
    {#if cloudSettingsSavedLine}<p class="hint">{cloudSettingsSavedLine}</p>{/if}
    <div class="form-actions">
      <button data-testid="cloud-settings-save" type="submit" disabled={cloudSettingsPending}>
        {cloudSettingsPending ? 'Saving…' : 'Save'}
      </button>
    </div>
  </form>
{:else if cloudSettingsError}
  <p data-testid="cloud-settings-error" class="error" role="alert">{cloudSettingsError}</p>
{:else}
  <p class="muted">Loading…</p>
{/if}

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

  label.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input[type='checkbox'] {
    width: auto;
    padding: 0;
    accent-color: var(--primary);
  }

  input[type='number'] {
    width: 100px;
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

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
