<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { libraryMove } from './stores/libraryMove.svelte';
  import type { DownloadEntry } from './api';
  import { cancelEntry, dismissEntry, retryEntry, runRowAction } from './downloads/actions';
  import { entryDetail, graphCaption, kindLabel, percent, rowAction } from './downloads/format';
  import {
    groupBySegment,
    segmentEmptyText,
    segmentLabel,
    SEGMENTS,
  } from './downloads/segments';
  import Sparkline from './downloads/Sparkline.svelte';
  import { compatTools } from './stores/compatTools.svelte';
  import { gameStores } from './stores/gameStores.svelte';
  import { entrySource, hasStoreBadge } from './gamestores/downloads';
  import { storeName } from './gamestores/marks';
  import StoreMark from './gamestores/desktop/StoreMark.svelte';
  import type { LineContext } from './downloads/format';
  import { isWaitingForStore } from './gamestores/jobs';
  import { STOP_WAITING, STOP_WAITING_TITLE } from './gamestores/steam';
  import { steamStopWaiting } from './gamestores/desktop/ui.svelte';

  /** What a store or installer row's line needs beyond the row (GameStores README). */
  function lineCtx(e: DownloadEntry): LineContext {
    return { compat: compatTools.defaultTool, version: e.job === 'store' ? (gameStores.game(e.source_id)?.version ?? '') : '' };
  }

  let errors = $state<Record<number, string>>({});
  let pending = $state<Record<number, boolean>>({});

  // Design §8: three stacked segments in this order. A row moves between
  // them as its status changes; its `download-row-<id>` element exists
  // exactly once at all times, which every install spec relies on.
  let groups = $derived(groupBySegment(downloads.entries));

  // Errors and pending flags are this view's; the actions themselves are
  // shared (downloads/actions.ts), including a library move row's Cancel
  // and Retry, which are the move's own commands (9b).
  const track = {
    clearError: (id: number) => {
      const { [id]: _dropped, ...rest } = errors;
      errors = rest;
    },
    setError: (id: number, message: string) => {
      errors = { ...errors, [id]: message };
    },
    setPending: (id: number, on: boolean) => {
      if (on) {
        pending = { ...pending, [id]: true };
      } else {
        const { [id]: _cleared, ...remaining } = pending;
        pending = remaining;
      }
    },
  };

  const cancel = (e: DownloadEntry) => runRowAction(e.id, () => cancelEntry(e), track);
  const retry = (e: DownloadEntry) => runRowAction(e.id, () => retryEntry(e), track);
  const dismiss = (id: number) => runRowAction(id, () => dismissEntry(id), track);
  // The card says what happened (or why it was refused), so no row error.
  async function stopWaiting(e: DownloadEntry) {
    track.setPending(e.id, true);
    try {
      await steamStopWaiting(e);
    } finally {
      track.setPending(e.id, false);
    }
  }

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
<section class="downloads view-content panel scrolls" aria-label="Downloads">
  <header class="head">
    <div class="head-text">
      <h1>Downloads</h1>
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
          {@const action = rowAction(e, libraryMove.phase)}
          {@const progress = rowProgress(e)}
          {@const waiting = isWaitingForStore(e)}
          <div data-testid={`download-row-${e.id}`} class="row" class:move={e.kind === 'library_move'}>
            <div class="row-text">
              <span class="title-row">
                <span class="title">{e.title}</span>
                {#if kindLabel(e.kind)}
                  <span data-testid={`download-kind-${e.id}`} class="kind">{kindLabel(e.kind)}</span>
                {/if}
                {#if hasStoreBadge(e) && entrySource(e) !== null}
                  {@const src = entrySource(e)!}
                  <!-- GameStores README: a store row's badge, its mark and name. -->
                  <span data-testid={`download-store-${e.id}`} class="kind store"><StoreMark source={src} />{storeName(src)}</span>
                {:else}
                  <span class="platform">{e.platform}</span>
                {/if}
              </span>
              <span
                data-testid={`download-detail-${e.id}`}
                class="detail"
                class:err={e.status === 'failed' && (e.job === 'store' || e.failure !== null)}
                title={entryDetail(e, lineCtx(e))}>{entryDetail(e, lineCtx(e))}</span
              >
              {#if !waiting}
                <span class="bar-track" class:indeterminate={progress.indeterminate}>
                  <span class="bar-fill" style={progress.indeterminate ? '' : `width: ${progress.pct}%`}></span>
                </span>
              {/if}
              {#if errors[e.id]}
                <p class="row-error">{errors[e.id]}</p>
              {/if}
            </div>

            <!-- Design §8: the 120×38 sparkline panel beside the buttons —
                 network in primary, disk in teal, 60 one-second samples. -->
            {#if waiting}
              <!-- GameStores README: a Steam row waiting for Steam to start has no bar and no sparkline. -->
              <span class="graph" aria-hidden="true"></span>
            {:else}
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
            {/if}

            <div class="row-actions">
              {#if action === 'cancel'}
                <!-- A library move's Cancel is the quiet choice: secondary, and
                     named for what it stops. -->
                <button
                  data-testid={`download-action-cancel-${e.id}`}
                  class:secondary={e.kind === 'library_move'}
                  disabled={pending[e.id]}
                  onclick={() => cancel(e)}
                >{e.kind === 'library_move' ? 'Cancel move' : 'Cancel'}</button>
              {:else if action === 'managed'}
                <!-- GameStores README: Steam owns pause and cancel of its installs. -->
                <button data-testid={`download-action-cancel-${e.id}`} disabled title="Managed by Steam">Cancel</button>
              {:else if action === 'stop-waiting'}
                <!-- Steam writes nothing when its install window is cancelled: the user ends the wait. -->
                <button
                  data-testid={`download-action-stop-waiting-${e.id}`}
                  class="secondary small"
                  title={STOP_WAITING_TITLE}
                  disabled={pending[e.id]}
                  onclick={() => stopWaiting(e)}
                >{STOP_WAITING}</button>
              {:else if action === 'retry-dismiss'}
                <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e)}>Retry</button>
                <button data-testid={`download-action-dismiss-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {:else if action === 'retry-cancel'}
                <!-- A failed library move still in its copy: run it again, or undo it. -->
                <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e)}>Retry</button>
                <button data-testid={`download-action-cancel-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => cancel(e)}>Cancel move</button>
              {:else if action === 'retry'}
                <!-- A failed library move past its copy cannot be undone: Retry only. -->
                <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e)}>Retry</button>
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
    padding: 20px 24px;
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

  /* A failed store or installer row's line is red (GameStores prototype `.s.err`). */
  .detail.err {
    color: var(--danger);
  }

  /* A store row's badge: its mark and name (GameStores prototype `.kind.st-schip`). */
  .kind.store {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    text-transform: none;
    letter-spacing: 0;
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
    color: var(--on-primary);
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

  /* The app's small button (GameStores README): 6px corners, 10px sides. */
  .row-actions button.small {
    padding: 5px 10px;
  }

  .row-actions button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  /* The library move row. It reads like the others (same grid, bar, graph
     and buttons); the differences are for legibility only. Its detail line
     carries a phase ("Copying • games/ · 1.2 of 5.6 GB", "Updating paths…",
     "Removing the old copies…") or a failure reason, so it wraps instead of
     being cut off; the full text is also the tooltip. A thin --primary edge
     marks it as the one job that is not a download. */
  .row.move {
    box-shadow: inset 3px 0 0 var(--primary);
  }

  .row.move .detail {
    white-space: normal;
    overflow-wrap: anywhere;
  }

  .row.move .row-actions button:not(.secondary) {
    font-weight: 600;
  }

  .row.move .row-actions button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
</style>
