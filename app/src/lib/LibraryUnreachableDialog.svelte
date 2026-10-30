<script lang="ts">
  // The unreachable-library dialog (9b). Blocking: App.svelte renders it in
  // place of the whole app while the library folder cannot be reached, and it
  // has no Cancel or Escape — GRID cannot run without a valid library. Two
  // ways out only: relocate the library to the folder where it is now
  // (stored paths are rewritten, no file moves), or close GRID without
  // changing anything.
  //
  // Serious but calm: no red wash, a gold-edged badge, the missing folder in a
  // monospace box, then the two choices as large buttons ("Relocate the
  // library…" is the primary and takes focus). The folder field is additive to
  // the picker, as for every Browse in the app: it is the path the end-to-end
  // suite drives. Escape is swallowed on purpose.
  import { tick } from 'svelte';
  import { pickFolder } from './pickers';
  import {
    RELOCATE_PICKER_TITLE,
    relocateError,
    relocateNeedsPicker,
    unreachableCopy,
  } from './libraryUnreachable';
  import { closeWithoutChanges, libraryUnreachable, relocateLibrary } from './stores/libraryUnreachable.svelte';

  let typed = $state('');
  let pending = $state(false);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);
  let relocateEl = $state<HTMLButtonElement | null>(null);
  let copy = $derived(unreachableCopy(libraryUnreachable.status));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  // Focus starts on the primary choice once the dialog is on screen.
  $effect(() => {
    relocateEl?.focus();
  });

  async function browse() {
    const picked = await pickFolder(RELOCATE_PICKER_TITLE);
    if (picked !== null) typed = picked;
  }

  async function relocate() {
    if (pending) return;
    if (relocateNeedsPicker(typed)) {
      await browse();
      if (relocateNeedsPicker(typed)) return;
    }
    pending = true;
    error = null;
    try {
      error = relocateError(await relocateLibrary(typed.trim()));
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pending = false;
    }
    // The buttons were disabled while the call ran, which dropped focus.
    await tick();
    relocateEl?.focus();
  }

  async function close() {
    if (pending) return;
    pending = true;
    try {
      await closeWithoutChanges();
    } catch (err) {
      error = errorMessage(err);
      pending = false;
    }
  }

  function focusables(): HTMLElement[] {
    return panelEl
      ? Array.from(panelEl.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled])'))
      : [];
  }

  // Tab stays inside the dialog; Escape does nothing (by design).
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
    } else if (e.key === 'Tab') {
      const items = focusables();
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || active === panelEl)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

</script>

{#if copy}
  <div class="backdrop" role="presentation">
    <div
      data-testid="library-unreachable"
      class="panel"
      bind:this={panelEl}
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="library-unreachable-title"
      aria-describedby="library-unreachable-reason library-unreachable-body"
      tabindex="-1"
      onkeydown={onKey}
    >
      <header class="head">
        <span class="badge" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="24" height="24" focusable="false">
            <path d="M12 3.5l9.5 16.5h-19z" />
            <path d="M12 10v4.6M12 17.4v.1" />
          </svg>
        </span>
        <h2 id="library-unreachable-title">{copy.title}</h2>
      </header>

      <p id="library-unreachable-reason" data-testid="library-unreachable-reason" class="reason">{copy.reason}</p>

      <div class="missing">
        <span class="field-label">Folder GRID cannot reach</span>
        <code data-testid="library-unreachable-root" class="path">{copy.root}</code>
      </div>

      <p id="library-unreachable-body" class="explain">{copy.body}</p>

      <div class="field">
        <label class="field-label" for="library-unreachable-path">Folder where the library is now</label>
        <span class="row">
          <input
            id="library-unreachable-path"
            data-testid="library-unreachable-path"
            bind:value={typed}
            placeholder="/path/to/library"
            autocomplete="off"
            spellcheck="false"
            aria-invalid={error !== null}
            aria-describedby={error !== null ? 'library-unreachable-error' : undefined}
            disabled={pending}
          />
          <button data-testid="library-unreachable-browse" class="browse" onclick={browse} disabled={pending}>
            Browse…
          </button>
        </span>
        {#if error}
          <p id="library-unreachable-error" data-testid="library-unreachable-error" class="notice error" role="alert">
            {error}
          </p>
        {/if}
      </div>

      <div class="actions">
        <button
          data-testid="library-unreachable-relocate"
          class="big primary"
          bind:this={relocateEl}
          onclick={relocate}
          disabled={pending}
        >
          {pending ? 'Relocating…' : 'Relocate the library…'}
        </button>
        <button data-testid="library-unreachable-close" class="big" onclick={close} disabled={pending}>
          Close without changing anything
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Opaque: the dialog replaces the whole app, so there is nothing behind it
     to dim. The panel sits one step above the page (`--surface-2`). */
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--bg);
    color: var(--text);
    z-index: 100;
  }

  .panel {
    width: min(560px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    box-sizing: border-box;
    overflow-y: auto;
    padding: 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    border-radius: var(--r-panel);
    border: 1px solid var(--border);
    border-top: 3px solid var(--gold);
    background: var(--surface-2);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  /* The warning badge: --gold (theme-aware) on a neutral disc. */
  .badge {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--r-pill);
    border: 1px solid var(--gold);
    background: var(--surface);
  }

  .badge svg {
    fill: none;
    stroke: var(--gold);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    line-height: 130%;
    color: var(--text-h);
  }

  .reason {
    margin: 0;
    font-size: 15px;
    line-height: 150%;
    color: var(--text-h);
  }

  .explain {
    margin: 0;
    font-size: 13px;
    line-height: 150%;
    color: var(--text-muted);
  }

  .missing,
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
  }

  /* The missing folder as a value: monospace, wraps at any character so a
     long path never widens the dialog, selectable for copying. */
  .path {
    display: block;
    padding: 10px 12px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    border-left: 3px solid var(--gold);
    background: var(--surface);
    color: var(--text-h);
    font-family: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 13px;
    line-height: 150%;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  input {
    flex: 1;
    min-width: 0;
    box-sizing: border-box;
    min-height: 40px;
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  input::placeholder {
    color: var(--text-muted);
    opacity: 0.8;
  }

  input[aria-invalid='true'] {
    border-color: var(--danger);
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  /* An error line in the calm notice shape, with the danger edge. */
  .notice {
    margin: 0;
    padding: 10px 12px;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
    font-size: 13px;
    line-height: 150%;
    overflow-wrap: anywhere;
  }

  .notice.error {
    border-left-color: var(--danger);
  }

  button {
    font: inherit;
    cursor: pointer;
    color: var(--text-h);
    transition:
      background var(--m-fast) ease,
      border-color var(--m-fast) ease;
  }

  button:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .browse {
    flex: none;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
  }

  .browse:hover:not(:disabled) {
    background: var(--surface);
  }

  /* The two choices: full width, tall enough for a thumb or a pointer
     cursor, stacked with the safe path first. */
  .actions {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 4px;
  }

  .big {
    width: 100%;
    box-sizing: border-box;
    min-height: 48px;
    padding: 12px 20px;
    font-size: 15px;
    font-weight: 600;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: transparent;
  }

  .big:hover:not(:disabled) {
    background: var(--surface);
    border-color: var(--text-muted);
  }

  .big.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
  }

  .big.primary:hover:not(:disabled) {
    background: var(--primary-hover);
    border-color: var(--primary-hover);
  }
</style>
