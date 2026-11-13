<script lang="ts">
  // The one notification surface (design: "One notification style"), mounted
  // once by App.svelte above the phase branch, so the startup import notice
  // shows on Connect too. Every message is a card in a stack at the bottom
  // right, above the download strip and over dialogs; each closes itself
  // after its countdown (stores/toasts.svelte.ts) and holds while pointed at
  // or focused. `lead` is the setup card (SetupProgress), which App.svelte
  // passes in so it sits at the top of the same stack.
  import type { Snippet } from 'svelte';
  import { holdWhileEngaged } from './countdown';
  import { dismissToast, pauseToast, resumeToast, toasts } from './stores/toasts.svelte';

  let { lead }: { lead?: Snippet } = $props();
</script>

{#if toasts.list.length > 0 || lead}
  <section data-testid="toast-region" class="stack" aria-label="Notifications">
    {@render lead?.()}
    {#each toasts.list as toast (toast.id)}
      <aside
        class="note-card"
        class:error={toast.level === 'error'}
        role={toast.level === 'error' ? 'alert' : 'status'}
        use:holdWhileEngaged={(held) => (held ? pauseToast(toast.id) : resumeToast(toast.id))}
      >
        <!-- The message alone, so an exact-text read of `toast` never picks
             up the button's label. -->
        <p data-testid="toast" class="note-title">
          {#if toast.level === 'error'}<span class="sr-only">Error: </span>{/if}{toast.text}
        </p>
        <div class="note-actions">
          {#if toast.desktopAction}
            {@const act = toast.desktopAction}
            <button
              class="note-btn primary"
              data-testid="toast-action"
              onclick={() => {
                dismissToast(toast.id);
                act.run();
              }}>{act.label}</button
            >
          {/if}
          <button class="note-btn note-dismiss" data-testid="toast-dismiss" onclick={() => dismissToast(toast.id)}>
            Dismiss<span
              class="note-cd"
              class:paused={toast.paused}
              style:animation-duration={`${toast.durationMs}ms`}
              aria-hidden="true"
            ></span>
          </button>
        </div>
      </aside>
    {/each}
  </section>
{/if}

<style>
  /* A store card's one action (GameStores prototype `stToast`'s small-btn primary). */
  .note-btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .note-btn.primary:hover {
    background: var(--primary-hover);
  }

  .stack {
    position: fixed;
    right: 24px;
    /* Above the fixed download strip, the same clearance `.view` uses. */
    bottom: calc(var(--footer-h) + 24px);
    /* Over the details dialog (z 20) and its sub-dialogs (z 25), so a card
       raised from inside one is still visible. */
    z-index: 30;
    width: min(340px, calc(100vw - 48px));
    display: flex;
    flex-direction: column;
    gap: 8px;
    /* Only the cards take the pointer: the gaps between them do not block
       what is underneath. */
    pointer-events: none;
  }
</style>
