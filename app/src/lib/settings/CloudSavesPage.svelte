<script lang="ts">
  // Settings › Cloud saves (design §10: "current cloud settings form"), moved
  // out of Emulators.svelte (task-19-brief.md). The refresh is gated on the
  // Settings view being visible.
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
    class="g-form"
    onsubmit={(e) => {
      e.preventDefault();
      handleCloudSettingsSave();
    }}
  >
    <label class="g-check">
      <input
        data-testid="cloud-settings-download-on-launch"
        type="checkbox"
        bind:checked={cloudSettings.download_on_launch}
      />
      Restore cloud saves before launch
    </label>
    <label class="g-check">
      <input
        data-testid="cloud-settings-upload-on-exit"
        type="checkbox"
        bind:checked={cloudSettings.upload_on_exit}
      />
      Upload cloud saves after exit
    </label>
    <label class="g-check">
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
        class="g-field narrow"
        data-testid="cloud-settings-upload-delay"
        type="number"
        min="0"
        max="60"
        bind:value={cloudSettings.upload_delay_seconds}
      />
    </label>
    <label>
      Save retention limit (0 = unlimited)
      <input
        class="g-field narrow"
        data-testid="cloud-settings-retention-limit"
        type="number"
        min="0"
        bind:value={cloudSettings.retention_limit}
      />
    </label>
    <!-- grid-launcher.py:1733-1738. The Python wording's emulator-only
         clause is dropped here: the rewrite auto-syncs Windows/PC games
         too (grid-core/src/cloud/ops/upload.rs:117-118,
         ops/restore.rs:120-121 delegate to the native flows). -->
    <p data-testid="cloud-settings-autosync-hint" class="g-note">
      Auto-sync uses the latest server save record only.
    </p>
    {#if cloudSettingsError}<p data-testid="cloud-settings-error" class="g-error" role="alert">{cloudSettingsError}</p>{/if}
    {#if cloudSettingsSavedLine}<p class="g-note">{cloudSettingsSavedLine}</p>{/if}
    <div class="g-actions">
      <button class="g-btn" data-testid="cloud-settings-save" type="submit" disabled={cloudSettingsPending}>
        {cloudSettingsPending ? 'Saving…' : 'Save'}
      </button>
    </div>
  </form>
{:else if cloudSettingsError}
  <p data-testid="cloud-settings-error" class="g-error" role="alert">{cloudSettingsError}</p>
{:else}
  <p class="g-note">Loading…</p>
{/if}
