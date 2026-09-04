<script lang="ts">
  // Settings › Updates (design §10): "app version, last check, release link,
  // 'check-only' note". The status line has three states, driven by the
  // backend's `checked_at`: not checked, up to date (with the relative check
  // time), or the notice. The notice line and its two buttons keep the ids
  // plan 1 gave them; Dismiss hides the badge, not this entry.
  import { api } from '../api';
  import { appUpdate, dismiss } from '../stores/appUpdate.svelte';
  import { CHECK_ONLY_NOTE, updateStatusLine, versionLine } from './updates';

  let { active = true }: { active?: boolean } = $props();

  let version = $state('');
  // Re-read whenever the view comes forward so "5 min ago" is honest at the
  // moment the user looks, without a ticking timer.
  let now = $state(Date.now());

  $effect(() => {
    if (!active) return;
    now = Date.now();
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

<p data-testid="settings-updates-version" class="line">{versionLine(version)}</p>

{#if appUpdate.stored}
  <p data-testid="app-update-notice" class="update-line">
    {updateStatusLine(appUpdate.stored, appUpdate.checkedAt, now)}
    <button data-testid="app-update-open" onclick={openRelease}>Open release</button>
    {#if appUpdate.notice}
      <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
    {/if}
  </p>
{:else}
  <p data-testid="settings-updates-status" class="line">{updateStatusLine(null, appUpdate.checkedAt, now)}</p>
{/if}

<p data-testid="settings-updates-note" class="muted">{CHECK_ONLY_NOTE}</p>

<style>
  .line {
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .muted {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    max-width: 60ch;
  }

  .update-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .update-line button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .update-line button.secondary {
    border-color: transparent;
    color: var(--text-muted);
  }
</style>
