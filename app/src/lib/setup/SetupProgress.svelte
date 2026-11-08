<script lang="ts">
  import { api } from '../api';
  import { downloads } from '../stores/downloads.svelte';
  import { dismissSetupRun, setupRun } from '../stores/setupRun.svelte';
  import { CountdownTimer, holdWhileEngaged } from '../countdown';
  import { TOAST_DURATION_MS } from '../stores/toasts.svelte';
  import { cardCountsDown, headline, outcomeNotes, readyCount, readyText, segments, summarize } from './progress';

  // The first-run setup's progress in the shell (plan step 3, "Into the
  // app"): the first card in the notification stack (Toast.svelte, which
  // App.svelte hands this to). The installs themselves are ordinary
  // Downloads rows; this only follows them. Like every card it closes itself,
  // but its countdown only starts once the installs are done. Its bar has one
  // segment per install (Proton included): filled when done, a moving fill
  // while installing, empty while waiting; pointing at one names it.
  let { onOpenDownloads }: { onOpenDownloads: () => void } = $props();

  let progress = $derived(summarize(setupRun.queued, downloads.entries));
  let notes = $derived(outcomeNotes(setupRun.outcomes));
  let finished = $derived(cardCountsDown(progress));
  let segs = $derived(segments(setupRun.queued, downloads.entries, setupRun.names));
  let ready = $derived(readyCount(setupRun.outcomes, setupRun.compat, segs));
  let retryError = $state<string | null>(null);
  let countsDown = $derived(setupRun.active && finished);

  // Plain, not `$state`: the countdown effect below must not restart when
  // the pointer comes and goes, only when the card starts or stops counting.
  let held = false;
  let paused = $state(false);
  let timer: CountdownTimer | null = null;

  $effect(() => {
    if (!countsDown) return;
    const t = new CountdownTimer(TOAST_DURATION_MS, dismissSetupRun);
    if (held) t.pause();
    timer = t;
    return () => {
      t.stop();
      if (timer === t) timer = null;
    };
  });

  function onHold(next: boolean) {
    held = next;
    paused = next;
    if (next) timer?.pause();
    else timer?.resume();
  }

  async function retry(entryId: number) {
    retryError = null;
    try {
      await api.retryInstall(entryId);
    } catch (e) {
      retryError = String(e);
    }
  }
</script>

{#if setupRun.active}
  <aside class="note-card" data-testid="setup-progress" aria-live="polite" use:holdWhileEngaged={onHold}>
    <p class="note-title" data-testid="setup-progress-title">{headline(progress)}</p>
    {#if progress.total > 0}
      <div
        class="segbar"
        role="progressbar"
        aria-valuemin="0"
        aria-valuemax={progress.total}
        aria-valuenow={progress.done}
        aria-label={`${progress.done} of ${progress.total} installs done`}
      >
        {#each segs as seg (seg.sourceId)}
          <span
            class={`seg ${seg.state}`}
            title={seg.title}
            data-testid={`setup-seg-${seg.sourceId}`}
            data-state={seg.state}
          ><b></b></span>
        {/each}
      </div>
    {/if}
    {#if finished && ready > 0}
      <p class="line" data-testid="setup-progress-ready">{readyText(ready)}</p>
    {/if}
    {#each progress.failed as entry (entry.id)}
      <p class="line failed" data-testid={`setup-failed-${entry.source_id}`}>
        {entry.title} didn't install.
        <button class="link" onclick={() => retry(entry.id)}>Retry</button>
      </p>
    {/each}
    {#each setupRun.errors as error (error)}
      <p class="line failed">{error}</p>
    {/each}
    {#each notes as note (note)}
      <p class="line" data-testid="setup-note">{note}</p>
    {/each}
    {#if retryError}<p class="line failed" role="alert">{retryError}</p>{/if}
    <div class="note-actions">
      <button class="note-btn" onclick={onOpenDownloads}>Open Downloads</button>
      {#if countsDown}
        <button class="note-btn note-dismiss" data-testid="setup-progress-dismiss" onclick={dismissSetupRun}>
          Dismiss<span
            class="note-cd"
            class:paused
            style:animation-duration={`${TOAST_DURATION_MS}ms`}
            aria-hidden="true"
          ></span>
        </button>
      {/if}
    </div>
  </aside>
{/if}

<style>
  /* The card itself (.note-card, .note-btn, the Dismiss countdown) is the
     shared notification style in app.css; Toast.svelte positions the stack. */
  /* One segment per install: done is a solid fill, the one installing a
     moving fill, waiting ones empty, a failed one red. */
  .segbar {
    display: flex;
    gap: 3px;
  }

  .seg {
    position: relative;
    flex: 1 1 0;
    height: 4px;
    border-radius: var(--r-pill);
    background: var(--surface);
    overflow: hidden;
  }

  .seg b {
    position: absolute;
    inset: 0;
    transform: scaleX(0);
    transform-origin: left center;
    background: var(--primary);
  }

  .seg.done b {
    transform: scaleX(1);
    transition: transform var(--m-base) ease;
  }

  .seg.running b {
    transform: scaleX(1);
    opacity: 0.45;
    background: linear-gradient(90deg, transparent 0, var(--primary) 50%, transparent 100%);
    background-size: 200% 100%;
    animation: seg-run 1.2s linear infinite;
  }

  .seg.failed {
    background: var(--danger);
  }

  @keyframes seg-run {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .seg.running b {
      animation: none;
      background: var(--primary);
    }

    .seg.done b {
      transition: none;
    }
  }

  .line {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .line.failed {
    color: var(--danger);
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: 12px;
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
  }
</style>
