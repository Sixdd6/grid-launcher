<script lang="ts">
  // The store dialogs' frame (GameStores prototype `.g-dialog.stdlg`): the
  // Game backups dialog's panel, 540px, over the scrim, with the close
  // button, Escape, a focus trap and focus handed back on close. The
  // dialog's own markup goes inside; the classes below (`.sub`, `.note`,
  // `.st-field`, `.foot`, `.opt`…) are the prototype's, shared by every
  // store dialog.
  import { untrack, type Snippet } from 'svelte';
  import Icon from '../../Icon.svelte';

  let {
    testId,
    label,
    onClose,
    width = 540,
    children,
  }: {
    testId: string;
    label: string;
    onClose: () => void;
    width?: number;
    children: Snippet;
  } = $props();

  let panelEl = $state<HTMLElement | null>(null);

  $effect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    untrack(() => panelEl?.focus());
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  });

  function trapTab(e: KeyboardEvent) {
    if (!panelEl) return;
    const focusable = Array.from(
      panelEl.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled])'),
    ).filter((el) => !(el instanceof HTMLInputElement && el.type === 'radio' && !el.checked));
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
      onClose();
    } else if (e.key === 'Tab') {
      trapTab(e);
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid={testId}
    class="st-dlg"
    style:width={`min(${width}px, calc(100vw - 48px))`}
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={label}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid={`${testId}-close`} class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    display: grid;
    place-items: center;
    z-index: 26;
  }

  .st-dlg {
    position: relative;
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 22px 24px;
    border-radius: var(--r-dialog);
    background: var(--overlay);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-overlay);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .st-dlg:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .close {
    position: absolute;
    top: 14px;
    right: 14px;
    color: var(--text-muted);
  }

  .st-dlg :global(h3) {
    margin: 0;
    padding-right: 36px;
    font-size: 17px;
    font-weight: 600;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .st-dlg :global(h4) {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .st-dlg :global(.note) {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .st-dlg :global(.help) {
    margin: 0;
    font-size: 11px;
    color: var(--text-muted);
  }

  .st-dlg :global(.sub) {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .st-dlg :global(.warn-line) {
    margin: 0;
    font-size: 12px;
    color: var(--warning-text);
  }

  .st-dlg :global(.st-dlg-head) {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-right: 36px;
  }

  .st-dlg :global(.st-dlg-head h3) {
    padding-right: 0;
  }

  .st-dlg :global(.st-dlg-head .note) {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .st-dlg :global(.st-cover-sm) {
    width: 54px;
    aspect-ratio: 3 / 4;
    border-radius: 6px;
    flex: none;
    box-shadow: var(--shadow-card);
    object-fit: cover;
    background: var(--field);
  }

  .st-dlg :global(.st-field) {
    display: grid;
    grid-template-columns: 130px 1fr;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }

  .st-dlg :global(.st-field > span:first-child) {
    color: var(--text-muted);
  }

  .st-dlg :global(.st-field select),
  .st-dlg :global(.st-field input.g-field) {
    width: 100%;
  }

  .st-dlg :global(.st-field .inline) {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .st-dlg :global(.st-field code),
  .st-dlg :global(code.path) {
    font: 12px ui-monospace, monospace;
    color: var(--text-h);
    overflow-wrap: anywhere;
  }

  .st-dlg :global(.opts) {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .st-dlg :global(.opt) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
    cursor: pointer;
    font-size: 13px;
  }

  .st-dlg :global(.opt.on) {
    border-color: var(--primary);
    background: var(--selected);
  }

  .st-dlg :global(.opt input) {
    margin: 0;
    accent-color: var(--primary);
  }

  .st-dlg :global(.opt-main) {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1 1 auto;
    min-width: 0;
  }

  .st-dlg :global(.opt-main b) {
    color: var(--text-h);
    font-weight: 600;
  }

  .st-dlg :global(.opt-free) {
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .st-dlg :global(.check) {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
  }

  .st-dlg :global(.check input) {
    margin: 2px 0 0;
    accent-color: var(--primary);
  }

  .st-dlg :global(.check > span) {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .st-dlg :global(.check b) {
    color: var(--text-h);
    font-weight: 600;
  }

  .st-dlg :global(.small-btn) {
    font: inherit;
    font-size: 12px;
    padding: 5px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .st-dlg :global(.small-btn:hover:not(:disabled)) {
    background: var(--surface);
  }

  .st-dlg :global(.foot) {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }

  .st-dlg :global(.foot > .note:first-child),
  .st-dlg :global(.foot > span:first-child) {
    margin-right: auto;
  }

  .st-dlg :global(.space-warn) {
    color: var(--danger);
    font-weight: 600;
  }

  .st-dlg :global(.error) {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
  }
</style>
