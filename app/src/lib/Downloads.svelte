<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { api, type DownloadEntry } from './api';
  import { actionFor, entryDetail, graphCaption, kindLabel, percent } from './downloads/format';
  import {
    groupBySegment,
    LEGEND_TEXT,
    segmentEmptyText,
    segmentLabel,
    SEGMENTS,
  } from './downloads/segments';
  import Sparkline from './downloads/Sparkline.svelte';

  let errors = $state<Record<number, string>>({});
  let pending = $state<Record<number, boolean>>({});

  // Design §8: three stacked segments in this order. A row moves between
  // them as its status changes; its `download-row-<id>` element exists
  // exactly once at all times, which every install spec relies on.
  let groups = $derived(groupBySegment(downloads.entries));

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

<!-- D-UI-7: `.view-content` caps the column at 1100px and centres it. -->
<section class="downloads view-content over-art" aria-label="Downloads">
  <header class="head">
    <div class="head-text">
      <h1>Downloads</h1>
      <p data-testid="downloads-legend" class="legend">{LEGEND_TEXT}</p>
    </div>
    <div data-testid="downloads-graph-key" class="graph-key" aria-label="Graph colours">
      <span class="key-item"><span class="swatch net" aria-hidden="true"></span>Network</span>
      <span class="key-item"><span class="swatch disk" aria-hidden="true"></span>Disk</span>
    </div>
  </header>

  {#each SEGMENTS as seg (seg)}
    {@const rows = groups[seg]}
    <section data-testid={`downloads-seg-${seg}`} class="segment" aria-label={segmentLabel(seg)}>
      <h2 class="seg-head">
        <span>{segmentLabel(seg)}</span>
        <span data-testid={`downloads-seg-count-${seg}`} class="seg-count">{rows.length}</span>
      </h2>
      {#if rows.length === 0}
        <p class="seg-empty">{segmentEmptyText(seg)}</p>
      {:else}
        {#each rows as e (e.id)}
          {@const action = actionFor(e.status, e.kind)}
          {@const progress = rowProgress(e)}
          <div data-testid={`download-row-${e.id}`} class="row">
            <div class="row-text">
              <span class="title-row">
                <span class="title">{e.title}</span>
                {#if kindLabel(e.kind)}
                  <span data-testid={`download-kind-${e.id}`} class="kind">{kindLabel(e.kind)}</span>
                {/if}
                <span class="platform">{e.platform}</span>
              </span>
              <span data-testid={`download-detail-${e.id}`} class="detail">{entryDetail(e)}</span>
              <span class="bar-track" class:indeterminate={progress.indeterminate}>
                <span class="bar-fill" style={progress.indeterminate ? '' : `width: ${progress.pct}%`}></span>
              </span>
              {#if errors[e.id]}
                <p class="row-error">{errors[e.id]}</p>
              {/if}
            </div>

            <!-- Design §8: the 120×38 sparkline panel beside the buttons —
                 network in primary, disk in teal, 60 one-second samples. -->
            <div class="graph">
              <Sparkline
                samples={downloads.samplesFor(e.id)}
                width={120}
                height={38}
                label={`Transfer rate for ${e.title}`}
                testId={`download-graph-${e.id}`}
              />
              <span data-testid={`download-graph-caption-${e.id}`} class="graph-caption">
                {graphCaption(e)}
              </span>
            </div>

            <div class="row-actions">
              {#if action === 'cancel'}
                <button data-testid={`download-action-cancel-${e.id}`} disabled={pending[e.id]} onclick={() => cancel(e.id)}>Cancel</button>
              {:else if action === 'retry-dismiss'}
                <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e.id)}>Retry</button>
                <button data-testid={`download-action-dismiss-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {:else if action === 'dismiss'}
                <button data-testid={`download-action-dismiss-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </section>
  {/each}
</section>

<style>
  .downloads {
    padding: 24px;
  }

  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  .head-text {
    min-width: 0;
  }

  .head h1 {
    margin: 0 0 4px;
  }

  .legend {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .graph-key {
    display: flex;
    flex: none;
    gap: 12px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .key-item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .swatch {
    width: 14px;
    height: 2px;
    border-radius: 1px;
  }

  .swatch.net {
    background: var(--primary);
  }

  .swatch.disk {
    background: var(--graph-disk);
  }

  .segment {
    margin-bottom: 20px;
  }

  .seg-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .seg-count {
    padding: 0 7px;
    border-radius: var(--r-pill);
    background: var(--surface);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .seg-empty {
    margin: 0;
    padding: 10px 16px;
    border-radius: var(--r-row);
    border: 1px dashed var(--border);
    font-size: 12px;
    color: var(--text-muted);
  }

  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    margin-bottom: 8px;
    border-radius: var(--r-row);
    background: var(--surface);
    transition: background var(--m-fast) ease;
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .title {
    flex: 0 1 auto;
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
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text);
    opacity: 0.8;
    white-space: nowrap;
  }

  .platform {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .detail {
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar-track {
    position: relative;
    display: block;
    width: 100%;
    height: 4px;
    margin-top: 2px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .bar-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    border-radius: 2px;
    background: var(--primary);
    transition: width var(--m-base) ease;
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

  .row-error {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--danger);
  }

  .graph {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    width: 120px;
  }

  .graph-caption {
    min-height: 13px;
    font-size: 10px;
    line-height: 13px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .row-actions button {
    font: inherit;
    font-size: 12px;
    padding: 5px 12px;
    border-radius: var(--r-chip);
    border: 1px solid transparent;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .row-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .row-actions button.secondary {
    background: transparent;
    border-color: var(--border);
    color: var(--text-h);
  }

  .row-actions button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
