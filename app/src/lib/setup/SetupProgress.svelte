<script lang="ts">
  import { api } from '../api';
  import { downloads } from '../stores/downloads.svelte';
  import { dismissSetupRun, setupRun } from '../stores/setupRun.svelte';
  import { assignedCount, headline, outcomeNotes, summarize } from './progress';

  // The first-run setup's progress in the shell (plan step 3, "Into the
  // app"): a small card over the views until dismissed. The installs
  // themselves are ordinary Downloads rows; this only follows them.
  let { onOpenDownloads }: { onOpenDownloads: () => void } = $props();

  let progress = $derived(summarize(setupRun.queued, downloads.entries));
  let notes = $derived(outcomeNotes(setupRun.outcomes));
  let finished = $derived(progress.running === 0);
  let assigned = $derived(assignedCount(setupRun.outcomes));
  let percent = $derived(progress.total === 0 ? 100 : Math.round((progress.done / progress.total) * 100));
  let retryError = $state<string | null>(null);

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
  <aside class="card" data-testid="setup-progress" aria-live="polite">
    <p class="title" data-testid="setup-progress-title">{headline(progress)}</p>
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
    <div class="actions">
      <button class="secondary" onclick={onOpenDownloads}>Open Downloads</button>
      {#if finished}
        <button class="secondary" data-testid="setup-progress-dismiss" onclick={dismissSetupRun}>Dismiss</button>
      {/if}
    </div>
  </aside>
{/if}

<style>
  /* Floats over the views at the bottom right, above the download strip;
     under the details dialog (z 20) and toasts (z 30). Toasts are centred and
     at most 480px wide, so only a narrow window brings them under the card:
     there it rises above the toast lane instead. Design system: surface-2
     panel, border hairline, r-row, shadow-float. */
  .card {
    position: fixed;
    right: 24px;
    bottom: calc(var(--footer-h) + 24px);
    z-index: 15;
    width: min(340px, calc(100vw - 48px));
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    box-sizing: border-box;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }

  @media (max-width: 1240px) {
    .card {
      bottom: calc(var(--footer-h) + 96px);
    }
  }

  .title {
    margin: 0;
    font-weight: 600;
    color: var(--text-h);
  }

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

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .secondary {
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
  }

  .secondary:hover {
    background: var(--surface);
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--primary);
    text-decoration: underline;
  }

  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }
</style>
