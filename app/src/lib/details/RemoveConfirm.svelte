<script lang="ts">
  // Q4 "Remove from library (keeps files)": the confirm. It names the folder
  // that stays on disk; confirming deletes the registry row only.
  //
  // Cancel has focus on open (the safe default), Tab cycles Cancel / Remove
  // inside the dialog, Escape cancels, and focus returns to the opener on
  // close. The visible copy is split into a lead line and a monospace path
  // box; `removeConfirmMessage` stays the dialog's accessible description, so
  // the full sentence is what a screen reader hears.
  import { removeConfirmMessage } from './unlinked';
  import { activateFocused, moveDialogFocus, registerDialog, type NavAction } from '../focus/dialogNav';

  let {
    title,
    folder,
    onConfirm,
    onClose,
  }: {
    title: string;
    /** The folder that stays on disk; `''` when the row stores none. */
    folder: string;
    /** Removes the row. A rejection is shown in the dialog. */
    onConfirm: () => Promise<void>;
    onClose: () => void;
  } = $props();

  let pending = $state(false);
  let error = $state<string | null>(null);
  let cancelEl = $state<HTMLButtonElement | null>(null);
  let panelEl = $state<HTMLElement | null>(null);

  $effect(() => {
    cancelEl?.focus();
  });

  // Return focus to whatever opened the dialog once it unmounts, if that
  // element is still on the page (a successful remove can take it away).
  $effect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  });

  async function confirm() {
    // `aria-disabled`, not `disabled`: a disabled button drops focus to the
    // page body and takes the keyboard out of the dialog mid-request.
    if (pending) return;
    pending = true;
    error = null;
    try {
      await onConfirm();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      pending = false;
    }
  }

  function cancel() {
    if (!pending) onClose();
  }

  function trapTab(e: KeyboardEvent) {
    if (!panelEl) return;
    const focusable = Array.from(panelEl.querySelectorAll<HTMLElement>('button:not([disabled])'));
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === panelEl)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    } else if (e.key === 'Tab') {
      trapTab(e);
    }
  }

  /** A gamepad `nav` action, routed here while the dialog is open: the two
   *  buttons are a row, so left / right and up / down both move between them. */
  export function handleNav(action: NavAction): boolean {
    if (action === 'back') {
      cancel();
      return true;
    }
    if (action === 'accept') return activateFocused(panelEl);
    const buttons = panelEl
      ? Array.from(panelEl.querySelectorAll<HTMLElement>('button:not([disabled])'))
      : [];
    return moveDialogFocus(buttons, action, true);
  }

  $effect(() => registerDialog(handleNav));

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) cancel();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="remove-confirm"
    class="panel"
    bind:this={panelEl}
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="remove-confirm-title"
    aria-describedby="remove-confirm-message"
    tabindex="-1"
    onkeydown={onKey}
  >
    <h3 id="remove-confirm-title">Remove from library</h3>

    <!-- The full sentence, for assistive tech and as the testable message. -->
    <p id="remove-confirm-message" data-testid="remove-confirm-message" class="sr-only">
      {removeConfirmMessage(title, folder)}
    </p>

    <div class="body" aria-hidden="true">
      <p class="lead">Remove <strong>“{title}”</strong> from your library?</p>
      {#if folder === ''}
        <p class="reassure">No files are deleted.</p>
      {:else}
        <p class="reassure">Its files stay on disk in:</p>
        <code class="path" data-testid="remove-confirm-folder">{folder}</code>
      {/if}
    </div>

    {#if error}
      <p data-testid="remove-confirm-error" class="error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      <button data-testid="remove-confirm-no" class="btn secondary" bind:this={cancelEl} onclick={cancel}>
        Cancel
      </button>
      <button
        data-testid="remove-confirm-yes"
        class="btn danger"
        aria-disabled={pending}
        onclick={confirm}
      >
        {pending ? 'Removing…' : 'Remove'}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 25;
  }

  .panel {
    width: min(420px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 16px;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .lead {
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .lead strong {
    font-weight: 600;
  }

  .reassure {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* The kept folder: monospace, wraps at any character so a long path never
     widens the dialog, and reads as a value rather than prose. */
  .path {
    display: block;
    padding: 8px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-h);
    font-family: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 12px;
    line-height: 150%;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .btn {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .btn:hover:not([aria-disabled='true']) {
    background: var(--surface);
  }

  .btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* Same danger fill as Details' `.confirm` buttons, but the label is the
     theme background rather than white: white on the dark theme's #ff5050 is
     about 3:1, `--bg` on it is about 6.6:1, and on the light theme's #c62828
     it is about 5:1. */
  .btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--bg);
    font-weight: 600;
  }

  .btn.danger:hover:not([aria-disabled='true']) {
    background: var(--danger);
    filter: brightness(1.1);
  }

  .btn.danger:focus-visible {
    outline-color: var(--danger);
  }

  .btn[aria-disabled='true'] {
    opacity: 0.6;
    cursor: default;
  }
</style>
