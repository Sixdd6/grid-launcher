<script lang="ts">
  import { slide } from 'svelte/transition';
  import { downloads } from './stores/downloads.svelte';
  import { api, type DownloadEntry } from './api';
  import { aggregate, actionFor, entryDetail, percent } from './downloads/format';

  let open = $state(false);
  let errors = $state<Record<number, string>>({});
  let pending = $state<Record<number, boolean>>({});

  function toggle() {
    open = !open;
  }

  function toggleOnKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggle();
    }
  }

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function runAction(id: number, action: () => Promise<void>) {
    const { [id]: _dropped, ...rest } = errors;
    errors = rest;
    pending = { ...pending, [id]: true };
    try {
      await action();
    } catch (err) {
      errors = { ...errors, [id]: errorMessage(err) };
    } finally {
      const { [id]: _cleared, ...remaining } = pending;
      pending = remaining;
    }
  }

  const cancel = (id: number) => runAction(id, () => api.cancelInstall(id));
  const retry = (id: number) => runAction(id, () => api.retryInstall(id));
  const dismiss = (id: number) => runAction(id, () => api.dismissDownload(id));

  type Progress = { pct: number; indeterminate: boolean };

  function rowProgress(e: DownloadEntry): Progress {
    switch (e.status) {
      case 'downloading':
      case 'cancelling':
        return e.total_bytes > 0
          ? { pct: percent(e.downloaded_bytes, e.total_bytes), indeterminate: false }
          : { pct: 0, indeterminate: true };
      case 'installing':
        return e.install_total_bytes > 0
          ? { pct: percent(e.install_processed_bytes, e.install_total_bytes), indeterminate: false }
          : { pct: 0, indeterminate: true };
      case 'completed':
        return { pct: 100, indeterminate: false };
      case 'queued':
        return { pct: 0, indeterminate: false };
      default: // failed, cancelled
        return {
          pct: e.total_bytes > 0 ? percent(e.downloaded_bytes, e.total_bytes) : 0,
          indeterminate: false,
        };
    }
  }

  function footerProgress(): Progress | null {
    if (!downloads.hasLive) return null;
    const entries = downloads.entries;
    const active = entries.find((e) => e.status === 'downloading');
    if (active) return rowProgress(active);
    const installing = entries.find((e) => e.status === 'installing');
    if (installing) return rowProgress(installing);
    return { pct: 0, indeterminate: true }; // queued/cancelling only: nothing measurable yet
  }

  let footerLabel = $derived(aggregate(downloads.entries) || 'Downloads');
  let bar = $derived(footerProgress());
</script>

<footer
  class="downloads-footer"
  role="button"
  tabindex="0"
  aria-expanded={open}
  onclick={toggle}
  onkeydown={toggleOnKey}
>
  <span class="label">{footerLabel}</span>
  {#if bar}
    <span class="bar-track" class:indeterminate={bar.indeterminate}>
      <span class="bar-fill" style={bar.indeterminate ? '' : `width: ${bar.pct}%`}></span>
    </span>
  {/if}
</footer>

{#if open}
  <div class="drawer" transition:slide={{ duration: 160 }}>
    {#if downloads.entries.length === 0}
      <p class="empty">No downloads yet</p>
    {:else}
      {#each downloads.entries as e (e.id)}
        {@const action = actionFor(e.status)}
        {@const progress = rowProgress(e)}
        <div class="row">
          <div class="row-main">
            <div class="row-text">
              <span class="title">{e.title}</span>
              <span class="platform">{e.platform}</span>
              <span class="detail">{entryDetail(e)}</span>
              {#if errors[e.id]}
                <p class="row-error">{errors[e.id]}</p>
              {/if}
            </div>
            <div class="row-actions">
              {#if action === 'cancel'}
                <button disabled={pending[e.id]} onclick={() => cancel(e.id)}>Cancel</button>
              {:else if action === 'retry-dismiss'}
                <button disabled={pending[e.id]} onclick={() => retry(e.id)}>Retry</button>
                <button disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {:else if action === 'dismiss'}
                <button disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {/if}
            </div>
          </div>
          <span class="bar-track row-bar-track" class:indeterminate={progress.indeterminate}>
            <span class="bar-fill" style={progress.indeterminate ? '' : `width: ${progress.pct}%`}></span>
          </span>
        </div>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .downloads-footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    height: 36px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 16px;
    background: var(--bg);
    border-top: 1px solid var(--border);
    color: var(--text);
    font-size: 13px;
    cursor: pointer;
    z-index: 10;
  }

  .downloads-footer:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar-track {
    position: relative;
    flex: 0 0 auto;
    width: 96px;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .row-bar-track {
    width: 100%;
    flex: none;
    margin-top: 6px;
  }

  .bar-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 200ms ease;
  }

  .bar-track.indeterminate .bar-fill {
    width: 35% !important;
    animation: indeterminate 1.1s ease-in-out infinite;
  }

  @keyframes indeterminate {
    0% {
      left: -35%;
    }
    100% {
      left: 100%;
    }
  }

  .drawer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 36px;
    max-height: 40vh;
    overflow-x: hidden;
    overflow-y: auto;
    box-sizing: border-box;
    background: var(--bg);
    border-top: 1px solid var(--border);
    z-index: 9;
  }

  .empty {
    margin: 0;
    padding: 20px 16px;
    color: var(--text);
    font-size: 14px;
    text-align: center;
  }

  .row {
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
  }

  .row:last-child {
    border-bottom: none;
  }

  .row-main {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .title {
    color: var(--text-h);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .platform,
  .detail {
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-error {
    margin: 2px 0 0;
    font-size: 12px;
    color: #e5484d;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .row-actions button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
