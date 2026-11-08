<script lang="ts">
  import { api } from '../api';
  import { downloads } from '../stores/downloads.svelte';
  import { dismissSetupRun, setupRun } from '../stores/setupRun.svelte';
  import { CountdownTimer, holdWhileEngaged } from '../countdown';
  import { TOAST_DURATION_MS } from '../stores/toasts.svelte';
  import { assignedCount, cardCountsDown, headline, outcomeNotes, summarize } from './progress';

  // The first-run setup's progress in the shell (plan step 3, "Into the
  // app"): the first card in the notification stack (Toast.svelte, which
  // App.svelte hands this to). The installs themselves are ordinary
  // Downloads rows; this only follows them. Like every card it closes itself,
  // but its countdown only starts once the installs are done.
  let { onOpenDownloads }: { onOpenDownloads: () => void } = $props();

  let progress = $derived(summarize(setupRun.queued, downloads.entries));
  let notes = $derived(outcomeNotes(setupRun.outcomes));
  let finished = $derived(cardCountsDown(progress));
  let assigned = $derived(assignedCount(setupRun.outcomes));
  let percent = $derived(progress.total === 0 ? 100 : Math.round((progress.done / progress.total) * 100));
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
      <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}>
        <span style={`width: ${percent}%`}></span>
      </div>
    {/if}
    {#if finished && assigned > 0}
      <p class="line">{assigned} {assigned === 1 ? 'platform is' : 'platforms are'} ready to play.</p>
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
  .bar {
    height: 4px;
    border-radius: var(--r-pill);
    background: var(--surface);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--primary);
    transition: width var(--m-base) ease;
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
