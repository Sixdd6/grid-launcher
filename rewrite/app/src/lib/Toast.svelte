<script lang="ts">
  // The single toast surface, mounted once by App.svelte (above the phase
  // branch, so the startup import notice shows on Connect too). Bottom-centred
  // above the download strip, exactly where `ToastWidget._reposition`
  // (toast.py:90-95) puts Python's: horizontally centred, 24px above the
  // bottom edge. Pointer-events are off, matching
  // `WA_TransparentForMouseEvents` (toast.py:27) — a toast never blocks a
  // click on what is under it, and there is nothing to dismiss by hand.
  import { toasts } from './stores/toasts.svelte';

  const errors = $derived(toasts.list.filter((toast) => toast.level === 'error'));
  const others = $derived(toasts.list.filter((toast) => toast.level !== 'error'));
</script>

{#if toasts.list.length > 0}
  <div data-testid="toast-region" class="toasts">
    {#if others.length > 0}
      <div class="toast-group" role="status" aria-live="polite">
        {#each others as toast (toast.id)}
          <p data-testid="toast" class="toast">{toast.text}</p>
        {/each}
      </div>
    {/if}
    {#if errors.length > 0}
      <div class="toast-group" role="alert">
        {#each errors as toast (toast.id)}
          <p data-testid="toast" class="toast error">
            <span class="sr-only">Error: </span>{toast.text}
          </p>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .toasts {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    /* Above the fixed download strip, the same clearance `.view` uses. */
    bottom: calc(var(--footer-h) + 24px);
    /* Over the details dialog (z 20), so a toast raised from inside it is
       still visible. */
    z-index: 30;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: none;
  }

  .toast-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .toast {
    margin: 0;
    max-width: 480px;
    padding: 10px 14px;
    box-sizing: border-box;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    font-size: 13px;
    font-weight: 600;
    text-align: center;
    overflow-wrap: anywhere;
    /* The one literal rgba in this plan: copied from Shell.svelte's
       `.server-menu`, the other floating panel in the shell. */
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    animation: toast-in var(--m-base) ease;
  }

  .toast.error {
    color: var(--danger);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
</style>
