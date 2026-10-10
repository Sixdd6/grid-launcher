<script lang="ts">
  // The unreachable-library dialog (9b). Blocking: App.svelte renders it in
  // place of the whole app while the library folder cannot be reached, and it
  // has no Cancel or Escape — GRID cannot run without a valid library. Two
  // ways out only: relocate the library to the folder where it is now
  // (stored paths are rewritten, no file moves), or close GRID without
  // changing anything.
  //
  // Functional markup only; the designer styles it. The folder field is
  // additive to the picker, as for every Browse in the app: it is the path
  // the end-to-end suite drives.
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
  let copy = $derived(unreachableCopy(libraryUnreachable.status));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

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
</script>

{#if copy}
  <div class="backdrop" role="presentation">
    <div
      data-testid="library-unreachable"
      class="panel"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="library-unreachable-title"
      aria-describedby="library-unreachable-reason library-unreachable-body"
    >
      <h2 id="library-unreachable-title">{copy.title}</h2>
      <p id="library-unreachable-reason" data-testid="library-unreachable-reason">{copy.reason}</p>
      <code data-testid="library-unreachable-root">{copy.root}</code>
      <p id="library-unreachable-body">{copy.body}</p>

      <label for="library-unreachable-path">Folder where the library is now</label>
      <span class="row">
        <input
          id="library-unreachable-path"
          data-testid="library-unreachable-path"
          bind:value={typed}
          placeholder="/path/to/library"
          autocomplete="off"
          spellcheck="false"
          disabled={pending}
        />
        <button data-testid="library-unreachable-browse" onclick={browse} disabled={pending}>Browse…</button>
      </span>

      {#if error}
        <p data-testid="library-unreachable-error" role="alert">{error}</p>
      {/if}

      <div class="actions">
        <button data-testid="library-unreachable-relocate" onclick={relocate} disabled={pending}>
          {pending ? 'Relocating…' : 'Relocate the library…'}
        </button>
        <button data-testid="library-unreachable-close" onclick={close} disabled={pending}>
          Close without changing anything
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Placeholder layout so the dialog is usable; the designer replaces it. */
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--bg);
    z-index: 100;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 560px;
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
  }

  .row,
  .actions {
    display: flex;
    gap: 8px;
  }

  input {
    flex: 1;
  }

  code {
    overflow-wrap: anywhere;
  }
</style>
