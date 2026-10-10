<script lang="ts">
  // Details › Achievements: RomM's RetroAchievements list for the ROM, joined
  // with the RomM account's progress (`get_achievements`).
  import { api, type AchievementsView } from '../api';
  import Icon from '../Icon.svelte';
  import AchievementBadge from './AchievementBadge.svelte';
  import { PROGRESS_HINT, showsProgressHint, summaryLine, unlockDate } from './achievements';

  let { romId }: { romId: number } = $props();

  let view = $state<AchievementsView | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = romId;
    let cancelled = false;
    view = null;
    error = null;
    api
      .getAchievements(id)
      .then((v) => {
        if (!cancelled) view = v;
      })
      .catch((err) => {
        if (!cancelled) error = err instanceof Error ? err.message : String(err);
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class="achievements">
  {#if error}
    <p class="notice error" role="alert" data-testid="achievements-error">{error}</p>
  {:else if view === null}
    <p class="muted">Loading achievements…</p>
  {:else}
    {@const known = view.progress_known}
    {@const total = view.summary.total}
    {@const earned = view.summary.earned}
    <header class="head">
      {#if known && total > 0}
        <div class="bar-row">
          <div
            class="bar"
            role="progressbar"
            aria-label="Achievements unlocked"
            aria-valuemin={0}
            aria-valuemax={total}
            aria-valuenow={earned}
          >
            <span class="fill" style:width="{(earned / total) * 100}%"></span>
          </div>
          <span class="percent" aria-hidden="true">{Math.round((earned / total) * 100)}%</span>
        </div>
      {/if}
      <p class="summary" data-testid="achievements-summary">{summaryLine(view)}</p>
    </header>
    {#if showsProgressHint(view)}
      <p class="notice" data-testid="achievements-progress-hint">{PROGRESS_HINT}</p>
    {/if}
    <ul class="rows">
      {#each view.rows as row, i (row.ra_id ?? `row-${i}`)}
        {@const locked = known && !row.earned}
        <li class="row" class:locked data-testid="achievement-row">
          <AchievementBadge
            url={row.badge_url}
            alt={row.title}
            dimmed={locked}
            ring={row.earned && row.hardcore ? 'hardcore' : null}
          />
          <div class="text">
            <span class="title">{row.title}</span>
            {#if row.description}<span class="description">{row.description}</span>{/if}
            {#if row.earned}
              <span class="status">
                <span class="unlocked">Unlocked {unlockDate(row.unlocked_at)}</span>
                {#if row.hardcore}
                  <span class="hardcore" data-testid="achievement-hardcore"
                    ><Icon name="star" size={11} />Hardcore</span
                  >
                {/if}
              </span>
            {:else if known}
              <span class="status">
                <span class="lock" data-testid="achievement-locked"
                  ><svg
                    viewBox="0 0 24 24"
                    width="11"
                    height="11"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                    focusable="false"
                  >
                    <rect x="5" y="11" width="14" height="9" rx="2" />
                    <path d="M8 11V8a4 4 0 0 1 8 0v3" />
                  </svg>Locked</span
                >
              </span>
            {/if}
          </div>
          <span class="points">{row.points} pts</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .achievements {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* The summary stays at the top of the tab while a long list scrolls under
     it, so the totals are always in view. Opaque (--surface-2), because rows
     pass beneath it. */
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--r-row);
    background: var(--surface-2);
  }

  .bar-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bar {
    flex: 1;
    height: 8px;
    border-radius: var(--r-pill);
    background: var(--border);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    border-radius: var(--r-pill);
    background: var(--primary);
    transition: width var(--m-base) ease;
  }

  .percent {
    min-width: 3ch;
    text-align: right;
    color: var(--text-h);
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .summary {
    margin: 0;
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  /* A calm notice, as on the Connect form's reason line: the neutral
     `--surface` fill with a `--primary` edge and plain text colour. The error
     variant swaps the edge for `--danger`; the text stays plain so contrast
     holds in both themes. */
  .notice {
    margin: 0;
    padding: 10px 12px;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
    font-size: 13px;
  }

  .notice.error {
    border-left-color: var(--danger);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* Every row carries a 1px border (transparent when unlocked) so a locked
     row, which swaps its fill for an outline, does not change size. */
  .row {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  /* Locked: no fill, an outline, a faded badge (in AchievementBadge) and
     muted text. The text keeps its own token colours, never opacity, so it
     stays readable in both themes. */
  .row.locked {
    background: transparent;
    border-color: var(--border);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .title {
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .row.locked .title {
    color: var(--text-muted);
  }

  .description {
    color: var(--text-muted);
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin-top: 2px;
    color: var(--text-muted);
    font-size: 12px;
  }

  .lock {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .hardcore {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    border: 1px solid var(--gold);
    border-radius: var(--r-pill);
    color: var(--gold);
    font-size: 11px;
    font-weight: 600;
    line-height: 16px;
  }

  .points {
    flex: none;
    padding: 2px 10px;
    border: 1px solid var(--border);
    border-radius: var(--r-pill);
    background: var(--surface);
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .row.locked .points {
    background: transparent;
    color: var(--text-muted);
  }
</style>
