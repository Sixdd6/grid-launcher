<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { api, type DownloadEntry } from './api';
  import { actionFor, entryDetail, kindLabel, percent } from './downloads/format';

  let errors = $state<Record<number, string>>({});
  let pending = $state<Record<number, boolean>>({});

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
</script>

<section class="downloads view-content" aria-label="Downloads">
  {#if downloads.entries.length === 0}
    <p class="empty">No downloads yet</p>
  {:else}
    {#each downloads.entries as e (e.id)}
      {@const action = actionFor(e.status, e.kind)}
      {@const progress = rowProgress(e)}
      <div data-testid={`download-row-${e.id}`} class="row">
        <div class="row-main">
          <div class="row-text">
            <span class="title-row">
              <span class="title">{e.title}</span>
              {#if kindLabel(e.kind)}
                <span data-testid={`download-kind-${e.id}`} class="kind">{kindLabel(e.kind)}</span>
              {/if}
            </span>
            <span class="platform">{e.platform}</span>
            <span data-testid={`download-detail-${e.id}`} class="detail">{entryDetail(e)}</span>
            {#if errors[e.id]}
              <p class="row-error">{errors[e.id]}</p>
            {/if}
          </div>
          <div class="row-actions">
            {#if action === 'cancel'}
              <button data-testid={`download-action-cancel-${e.id}`} disabled={pending[e.id]} onclick={() => cancel(e.id)}>Cancel</button>
            {:else if action === 'retry-dismiss'}
              <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e.id)}>Retry</button>
              <button data-testid={`download-action-dismiss-${e.id}`} disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
            {:else if action === 'dismiss'}
              <button data-testid={`download-action-dismiss-${e.id}`} disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
            {/if}
          </div>
        </div>
        <span class="bar-track row-bar-track" class:indeterminate={progress.indeterminate}>
          <span class="bar-fill" style={progress.indeterminate ? '' : `width: ${progress.pct}%`}></span>
        </span>
      </div>
    {/each}
  {/if}
</section>

<style>
  .downloads {
    padding: 24px;
    box-sizing: border-box;
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

  .title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .title {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-h);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    flex: none;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid var(--border);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text);
    opacity: 0.8;
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
