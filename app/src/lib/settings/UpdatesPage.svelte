<script lang="ts">
  // Settings › Updates (design §10): "app version, last check, release link,
  // 'check-only' note". The status line has three states, driven by the
  // backend's `checked_at`: not checked, up to date (with the relative check
  // time), or the notice. The notice line and its two buttons keep the ids
  // plan 1 gave them; Dismiss hides the badge, not this entry.
  import { api } from '../api';
  import { appUpdate, dismiss, refreshAppUpdateStatus } from '../stores/appUpdate.svelte';
  import { CHECK_ONLY_NOTE, updateStatusLine, versionLine } from './updates';

  let { active = true }: { active?: boolean } = $props();

  let version = $state('');
  // Re-read whenever the view comes forward so "5 min ago" is honest at the
  // moment the user looks, without a ticking timer.
  let now = $state(Date.now());

  $effect(() => {
    if (!active) return;
    now = Date.now();
    // The backend emits its event only for a newer release, so a check that
    // found nothing newer reaches this page only through a pull.
    refreshAppUpdateStatus();
    api
      .appVersion()
      .then((v) => {
        version = v;
      })
      .catch(() => {
        // `versionLine('')` already says the version is unknown.
      });
  });

  function openRelease() {
    const stored = appUpdate.stored;
    if (stored === null) return;
    api.openReleasePage(stored.url).catch(() => {
      // The opener refuses anything outside the repo's releases prefix.
    });
  }
</script>

<p data-testid="settings-updates-version" class="g-line">{versionLine(version)}</p>

{#if appUpdate.stored}
  <p data-testid="app-update-notice" class="g-line update-line">
    {updateStatusLine(appUpdate.stored, appUpdate.checkedAt, now)}
    <button class="g-btn" data-testid="app-update-open" onclick={openRelease}>Open release</button>
    {#if appUpdate.notice}
      <button data-testid="app-update-dismiss" class="g-btn secondary" onclick={dismiss}>Dismiss</button>
    {/if}
  </p>
{:else}
  <p data-testid="settings-updates-status" class="g-line">{updateStatusLine(null, appUpdate.checkedAt, now)}</p>
{/if}

<p data-testid="settings-updates-note" class="g-note">{CHECK_ONLY_NOTE}</p>

<style>
  .update-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }
</style>
