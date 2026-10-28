<script lang="ts">
  // Settings › Library › Change… (Q2, 9b). One modal, four steps:
  //   choose          — the new folder (typed or Browse…), checked by the
  //                     backend as it changes; then Move existing files,
  //                     Start fresh, or Cancel.
  //   move-confirm    — what a move does (per folder: same drive or copied;
  //                     the space), then Start move: a background job with
  //                     its own Downloads row.
  //   fresh-ask       — "Delete the old game files, or leave them on disk?"
  //   delete-confirm  — the folders Delete removes and their total size.
  // With no current library folder there is nothing to keep or delete, so
  // Start fresh switches straight away.
  //
  // Same modal shape as RemoveConfirm / LinkPicker: backdrop + role="dialog",
  // Escape / backdrop / Cancel close it (Escape steps back first), Tab is
  // trapped, and focus returns to the opener.
  //
  // Focus on each step lands on the first control that is safe to press: the
  // folder field, then "Leave on disk", then "Back". The dialog's label and
  // description follow the step, and a polite live region announces the step.
  import {
    api,
    type LibraryMoveStartOutcome,
    type LibraryPathCheck,
    type MovePreview,
    type StartFreshOutcome,
    type StartFreshPreview,
  } from '../api';
  import { pickFolder } from '../pickers';
  import {
    canMove,
    canStartFresh,
    formatBytes,
    moveItemLine,
    movePreviewSummary,
    previewSummary,
    type ChangeStep,
  } from './library';

  let {
    currentPath,
    onDone,
    onMoveStarted,
    onClose,
  }: {
    /** The stored library path, verbatim: sent back as `expectedOldRoot`. */
    currentPath: string;
    /** A Start fresh call returned; `deleted` says which answer was given. */
    onDone: (outcome: StartFreshOutcome, deleted: boolean) => void;
    /** The move started in the background. */
    onMoveStarted: (outcome: LibraryMoveStartOutcome) => void;
    onClose: () => void;
  } = $props();

  let step = $state<ChangeStep>('choose');
  let typed = $state('');
  let check = $state<LibraryPathCheck | null>(null);
  let checking = $state(false);
  let preview = $state<StartFreshPreview | null>(null);
  let pending = $state(false);
  let error = $state<string | null>(null);
  let panelEl = $state<HTMLElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let keepEl = $state<HTMLButtonElement | null>(null);
  let deleteBackEl = $state<HTMLButtonElement | null>(null);
  let moveBackEl = $state<HTMLButtonElement | null>(null);
  let movePreview = $state<MovePreview | null>(null);

  let ready = $derived(canStartFresh(check, typed) && !checking);
  let moveReady = $derived(canMove(check, typed, currentPath) && !checking);
  let hasOldRoot = $derived(currentPath.trim() !== '');

  // How the move-confirm summary reads, from the preview alone (presentation
  // only; the backend still refuses a move that does not fit). A same-drive
  // move copies nothing, so space is no concern. A copy needs `required_bytes`
  // (the copied bytes plus a safety margin): below that the drive is short,
  // within a quarter above it the space is tight, and an unreadable figure is
  // flagged rather than assumed to be fine.
  type MoveTone = 'ok' | 'warn' | 'bad';
  let moveTone = $derived.by((): MoveTone => {
    const p = movePreview;
    if (p === null || p.copy_bytes === 0) return 'ok';
    if (p.free_bytes === null) return 'warn';
    if (p.free_bytes < p.required_bytes) return 'bad';
    if (p.free_bytes < p.required_bytes * 1.25) return 'warn';
    return 'ok';
  });
  let moveHint = $derived.by((): string => {
    const p = movePreview;
    if (p === null || p.items.length === 0) return '';
    if (p.copy_bytes === 0) return 'Each folder is renamed. Nothing is copied.';
    if (moveTone === 'bad') {
      return 'The new drive does not have enough free space for this move. Free up space or choose another folder.';
    }
    if (p.free_bytes === null) return 'GRID could not read the free space on the new drive.';
    if (moveTone === 'warn') return 'Space is tight: the new drive has little room to spare.';
    return 'Files are copied and checked first. The old copies are removed afterwards.';
  });

  // What the header says per step: the eyebrow, the dialog's accessible name
  // (the heading) and its accessible description.
  const STEP_META: Record<ChangeStep, { n: number; eyebrow: string; title: string; desc: string }> = {
    choose: {
      n: 1,
      eyebrow: 'Step 1 · Choose folder',
      title: 'Change library folder',
      desc: 'Choose the folder for your library. Type its full path or pick it with Browse….',
    },
    'fresh-ask': {
      n: 2,
      eyebrow: 'Step 2 · Old games',
      title: 'Delete the old game files, or leave them on disk?',
      desc: 'Either way, the games in the old folder leave your library. Emulators and saves stay where they are.',
    },
    'delete-confirm': {
      n: 3,
      eyebrow: 'Step 3 · Final check',
      title: 'Delete these game files?',
      desc: 'This cannot be undone. Only the folders listed below are removed.',
    },
    'move-confirm': {
      n: 2,
      eyebrow: 'Step 2 · Move',
      title: 'Move your library to the new folder?',
      desc: 'Games, emulators and saves move in the background. You can follow the move in Downloads.',
    },
  };
  let meta = $derived(STEP_META[step]);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  // Ask the backend about the typed folder, 250 ms after the last change. A
  // stale answer (the field changed meanwhile) is dropped.
  $effect(() => {
    const value = typed.trim();
    check = null;
    if (value === '') {
      checking = false;
      return;
    }
    checking = true;
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .checkLibraryPath(value)
        .then((result) => {
          if (!cancelled) check = result;
        })
        .catch((err) => {
          if (!cancelled) error = errorMessage(err);
        })
        .finally(() => {
          if (!cancelled) checking = false;
        });
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  // Declared before the focus effects below: it must read the opener before
  // this dialog moves focus into itself.
  $effect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  });

  // Each step's first safe control takes focus when it mounts (the bound
  // element changes on every step change).
  $effect(() => {
    inputEl?.focus();
  });

  $effect(() => {
    keepEl?.focus();
  });

  $effect(() => {
    deleteBackEl?.focus();
  });

  $effect(() => {
    moveBackEl?.focus();
  });

  async function browse() {
    const picked = await pickFolder('Select Library Folder');
    if (picked !== null) typed = picked;
  }

  async function run(deleteFiles: boolean) {
    if (pending || check === null || check.status !== 'ok') return;
    pending = true;
    error = null;
    try {
      const outcome = deleteFiles
        ? await api.libraryStartFreshDelete(check.path, currentPath)
        : await api.libraryStartFreshKeep(check.path, currentPath);
      onDone(outcome, deleteFiles);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pending = false;
    }
  }

  function startFresh() {
    if (!ready) return;
    error = null;
    if (hasOldRoot) step = 'fresh-ask';
    else void run(false);
  }

  async function askDelete() {
    if (pending) return;
    step = 'delete-confirm';
    preview = null;
    error = null;
    try {
      preview = await api.libraryStartFreshPreview();
    } catch (err) {
      error = errorMessage(err);
    }
  }

  // Move existing files (9b): the preview first, then the start.
  async function askMove() {
    if (pending || !moveReady || check === null || check.status !== 'ok') return;
    step = 'move-confirm';
    movePreview = null;
    error = null;
    try {
      const outcome = await api.libraryMovePreview(check.path);
      if (outcome.status === 'ok') movePreview = outcome.preview;
      else error = outcome.message;
    } catch (err) {
      error = errorMessage(err);
    }
  }

  async function startMove() {
    if (pending || movePreview === null || check === null || check.status !== 'ok') return;
    pending = true;
    error = null;
    try {
      const outcome = await api.libraryMoveStart(check.path);
      if (outcome.status === 'started') onMoveStarted(outcome);
      else error = outcome.message;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      pending = false;
    }
  }

  function back() {
    if (pending) return;
    error = null;
    if (step === 'delete-confirm') step = 'fresh-ask';
    else if (step === 'fresh-ask' || step === 'move-confirm') step = 'choose';
    else onClose();
  }

  function cancel() {
    if (!pending) onClose();
  }

  function focusables(): HTMLElement[] {
    return panelEl
      ? Array.from(panelEl.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled])'))
      : [];
  }

  function trapTab(e: KeyboardEvent) {
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

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      back();
    } else if (e.key === 'Tab') {
      trapTab(e);
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) cancel();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="library-change-dialog"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby="library-change-title"
    aria-describedby="library-change-desc"
    tabindex="-1"
    onkeydown={onKey}
  >
    <header class="head">
      <p class="eyebrow" class:final={step === 'delete-confirm'} aria-hidden="true">{meta.eyebrow}</p>
      <h3 id="library-change-title">{meta.title}</h3>
      <p id="library-change-desc" class="desc">{meta.desc}</p>
    </header>
    <p class="sr-only" aria-live="polite">Step {meta.n}: {meta.title}</p>

    <div class="body">
      {#if step === 'choose'}
        <div class="field">
          <span class="field-label">Current folder</span>
          <code class="path" class:unset={!hasOldRoot}>{hasOldRoot ? currentPath : 'Not set'}</code>
        </div>

        <div class="field">
          <label class="field-label" for="library-new-path">New folder</label>
          <span class="row">
            <input
              id="library-new-path"
              data-testid="library-new-path-input"
              bind:this={inputEl}
              bind:value={typed}
              placeholder="/path/to/library"
              autocomplete="off"
              spellcheck="false"
              aria-describedby="library-new-path-status"
              aria-invalid={check?.status === 'refused'}
              disabled={pending}
            />
            <button data-testid="library-new-path-browse" class="btn secondary" onclick={browse} disabled={pending}>
              Browse…
            </button>
          </span>
          <!-- A fixed-height slot, so the choices below do not jump as the
               check runs and answers. -->
          <div id="library-new-path-status" class="status">
            {#if check?.status === 'refused'}
              <p data-testid="library-change-refusal" class="status-line bad" role="alert">
                <svg class="status-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
                  <path d="M4 4l8 8M12 4l-8 8" />
                </svg>{check.message}</p>
            {:else if check?.status === 'ok' && !checking}
              <p data-testid="library-change-ok" class="status-line good" role="status">
                <svg class="status-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
                  <path d="M3 8.5l3.2 3.2L13 5" />
                </svg>OK. This folder can be used for your library.</p>
            {:else if checking}
              <p class="status-line" role="status">Checking the folder…</p>
            {:else if typed.trim() === ''}
              <p class="status-line idle">The folder is checked as you type.</p>
            {/if}
          </div>
        </div>

        <div class="choices" role="group" aria-label="What to do with your existing files">
          <button
            data-testid="library-change-move"
            class="choice"
            disabled={!moveReady}
            aria-disabled={pending}
            aria-labelledby="library-change-move-title"
            aria-describedby="library-change-move-desc"
            onclick={askMove}
          >
            <span class="choice-text">
              <span id="library-change-move-title" class="choice-title">Move existing files</span>
              <span id="library-change-move-desc" class="choice-desc"
                >Moves your games, emulators and saves to the new folder.</span
              >
            </span>
            <svg class="chevron" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
              <path d="M6 3l5 5-5 5" />
            </svg>
          </button>
          <button
            data-testid="library-change-fresh"
            class="choice"
            disabled={!ready}
            aria-disabled={pending}
            onclick={startFresh}
          >
            <span class="choice-text">
              <span class="choice-title">{pending ? 'Changing…' : 'Start fresh'}</span>
              <span class="choice-desc"
                >New installs go to the new folder. You choose what happens to the old games next.</span
              >
            </span>
            <svg class="chevron" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
              <path d="M6 3l5 5-5 5" />
            </svg>
          </button>
        </div>
      {:else if step === 'fresh-ask'}
        <div class="field">
          <span class="field-label">Old folder</span>
          <code class="path">{currentPath}</code>
        </div>

        <div class="choices" role="group" aria-label="Old game files">
          <button
            data-testid="library-fresh-keep"
            class="choice safe"
            bind:this={keepEl}
            aria-disabled={pending}
            onclick={() => run(false)}
          >
            <span class="choice-text">
              <span class="choice-title">{pending ? 'Changing…' : 'Leave on disk'}</span>
              <span class="choice-desc"
                >The games leave your library. Their files stay in the old folder for you to keep or delete yourself.</span
              >
            </span>
            <span class="tag safe">Safe</span>
          </button>
          <button
            data-testid="library-fresh-delete"
            class="choice destructive"
            aria-disabled={pending}
            onclick={askDelete}
          >
            <span class="choice-text">
              <span class="choice-title">Delete…</span>
              <span class="choice-desc">Removes the old game files from disk. You see the full list before anything is deleted.</span>
            </span>
          </button>
        </div>
      {:else if step === 'move-confirm'}
        <div class="route">
          <span class="field-label">From</span>
          <code class="path">{currentPath}</code>
          {#if movePreview}
            <span class="field-label">To</span>
            <code class="path">{movePreview.new_root}</code>
          {/if}
        </div>
        {#if movePreview === null && error === null}
          <p class="status-line" role="status">Measuring the library…</p>
        {:else if movePreview}
          <!-- The tone is the answer to "will it fit": calm edge when the move
               is instant or the space is ample, gold when it is tight or
               unknown, danger when the new drive is short. The hint line says
               the same in words, so colour is never the only cue. -->
          <div class="summary-box tone-{moveTone}">
            <span class="field-label">Size and space</span>
            <p data-testid="library-move-summary" class="summary">{movePreviewSummary(movePreview)}</p>
            {#if movePreview.items.length > 0}
              <p class="summary-sub">
                {formatBytes(movePreview.total_bytes)} in total · {movePreview.items.length}
                {movePreview.items.length === 1 ? 'folder' : 'folders'}
              </p>
              <p class="summary-hint">
                {#if moveTone !== 'ok'}
                  <svg class="hint-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
                    <path d="M8 2.5l6 10.5H2z" />
                    <path d="M8 6.8v2.9M8 11.4v.1" />
                  </svg>
                {/if}
                <span>{moveHint}</span>
              </p>
            {/if}
          </div>

          {#if movePreview.items.length > 0}
            <div class="field">
              <span id="library-move-folders-label" class="field-label">Folders that move</span>
              <ul data-testid="library-move-list" class="folders" aria-labelledby="library-move-folders-label">
                {#each movePreview.items as item (item.name)}
                  <li class="folder">
                    <span class="sr-only">{moveItemLine(item)}</span>
                    <span class="folder-name" aria-hidden="true">{item.name}/</span>
                    <span class="folder-size" aria-hidden="true">{formatBytes(item.bytes)}</span>
                    <span class="tag" class:safe={item.strategy === 'rename'} aria-hidden="true"
                      >{item.strategy === 'rename' ? 'Same drive' : 'Copied and checked'}</span
                    >
                  </li>
                {/each}
              </ul>
            </div>
          {/if}

          <section data-testid="library-move-blocked" class="while" aria-labelledby="library-move-while-title">
            <h4 id="library-move-while-title">While moving</h4>
            <p class="while-lead">These wait until the move ends:</p>
            <ul class="waits">
              <li>Installs and uninstalls</li>
              <li>Game and emulator launches</li>
              <li>Cloud transfers</li>
            </ul>
            <p class="while-warn">
              <svg class="hint-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
                <path d="M8 2.5l6 10.5H2z" />
                <path d="M8 6.8v2.9M8 11.4v.1" />
              </svg>
              <span>Close any emulator you opened yourself before you start.</span>
            </p>
            <p class="kept-note">If GRID closes, the move continues at the next start.</p>
          </section>
        {/if}
      {:else}
        {#if preview === null && error === null}
          <p class="status-line" role="status">Measuring the old games…</p>
        {:else if preview}
          <div class="summary-box">
            <span class="field-label">Will be deleted</span>
            <p data-testid="library-delete-summary" class="summary">{previewSummary(preview)}</p>
          </div>

          {#if preview.games.length > 0}
            <!-- Focusable on purpose: a scrollable list must be reachable by keyboard to scroll. -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <ul data-testid="library-delete-list" class="list" tabindex="0" aria-label="Games and folders that will be deleted">
              {#each preview.games as game (game.title + game.platform)}
                <li class="game">
                  <div class="game-head">
                    <span class="game-title">{game.title}</span>
                    <span class="game-size">{formatBytes(game.bytes)}</span>
                  </div>
                  <span class="game-platform">{game.platform}</span>
                  {#each game.paths as path (path)}
                    <code class="path doomed">{path}</code>
                  {/each}
                </li>
              {/each}
            </ul>
          {/if}

          {#if preview.left_outside.length > 0}
            <section class="kept" aria-labelledby="library-kept-title">
              <h4 id="library-kept-title">Stays on disk <span class="tag safe">Kept</span></h4>
              <p class="kept-note">These folders are outside the old library folder, so they are not deleted.</p>
              {#each preview.left_outside as path (path)}
                <code class="path kept-path">{path}</code>
              {/each}
            </section>
          {/if}
        {/if}
      {/if}
    </div>

    {#if error}
      <p data-testid="library-change-error" class="notice error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      {#if step === 'choose'}
        <button data-testid="library-change-cancel" class="btn secondary" onclick={cancel}>Cancel</button>
      {:else if step === 'fresh-ask'}
        <button
          data-testid="library-fresh-back"
          class="btn secondary"
          aria-disabled={pending}
          onclick={back}
        >
          Back
        </button>
      {:else if step === 'move-confirm'}
        <button
          data-testid="library-move-back"
          class="btn secondary"
          bind:this={moveBackEl}
          aria-disabled={pending}
          onclick={back}
        >
          Back
        </button>
        <button
          data-testid="library-move-confirm"
          class="btn primary"
          disabled={movePreview === null}
          aria-disabled={pending}
          onclick={startMove}
        >
          {pending ? 'Starting…' : 'Start move'}
        </button>
      {:else}
        <button
          data-testid="library-delete-back"
          class="btn secondary"
          bind:this={deleteBackEl}
          aria-disabled={pending}
          onclick={back}
        >
          Back
        </button>
        <button
          data-testid="library-delete-confirm"
          class="btn danger"
          disabled={preview === null}
          aria-disabled={pending}
          onclick={() => run(true)}
        >
          {pending ? 'Deleting…' : 'Delete and change folder'}
        </button>
      {/if}
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

  /* Header and actions stay put; only the body scrolls, so the buttons are
     always on screen. `text-shadow: none` drops the Settings view's halo,
     which is for text over art, not for an opaque panel. */
  .panel {
    width: min(560px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    box-sizing: border-box;
    padding: 24px;
    border-radius: var(--r-dialog);
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 16px;
    text-shadow: none;
  }

  .panel:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .eyebrow {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-muted);
  }

  .eyebrow.final {
    color: var(--danger);
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 16px;
    line-height: 130%;
  }

  h4 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-h);
  }

  .desc {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* The scroll region. The small padding and equal negative margin leave room
     for focus outlines that sit outside their control. */
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
    margin: -4px;
  }

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

  label.field-label {
    cursor: default;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  input {
    flex: 1;
    min-width: 0;
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
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

  /* Folders as values: monospace, wrap at any character so a long path never
     widens the dialog, selectable for copying. */
  .path {
    display: block;
    padding: 8px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    font-family: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 12px;
    line-height: 150%;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .path.unset {
    font-family: inherit;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* Folders Delete removes: a solid danger edge. */
  .path.doomed {
    border-left: 3px solid var(--danger);
  }

  /* Folders that stay: dashed and muted, set apart from the delete list. */
  .path.kept-path {
    border-style: dashed;
    background: transparent;
    color: var(--text-muted);
  }

  /* Live validation. The slot holds one line (two when a message wraps). */
  .status {
    min-height: 40px;
    display: flex;
    align-items: flex-start;
  }

  .status-line {
    margin: 0;
    flex: 1;
    padding: 8px 10px;
    border-left: 3px solid var(--border);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
    font-size: 13px;
    line-height: 150%;
  }

  .status-line.idle {
    background: transparent;
    color: var(--text-muted);
  }

  .status-line.good {
    border-left-color: var(--primary);
  }

  .status-line.bad {
    border-left-color: var(--danger);
  }

  .status-icon {
    display: inline-block;
    vertical-align: -3px;
    margin-right: 6px;
    fill: none;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .good .status-icon {
    stroke: var(--primary);
  }

  .bad .status-icon {
    stroke: var(--danger);
  }

  /* Choice cards: a title, a line of explanation, and optionally a tag. */
  .choices {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .choice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    box-sizing: border-box;
    padding: 12px 14px;
    font: inherit;
    text-align: left;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-h);
    cursor: pointer;
    transition:
      border-color var(--m-fast) ease,
      background var(--m-fast) ease;
  }

  .choice-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .choice-title {
    font-weight: 600;
  }

  .choice-desc {
    font-size: 12px;
    line-height: 150%;
    color: var(--text-muted);
  }

  .choice:hover:not(:disabled):not([aria-disabled='true']) {
    border-color: var(--primary);
  }

  .choice:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .choice:disabled {
    cursor: default;
    background: transparent;
    border-style: dashed;
  }

  /* Only the text dims, so a tag on a disabled card stays readable. */
  .choice:disabled .choice-text {
    opacity: 0.65;
  }

  .choice[aria-disabled='true'] {
    cursor: default;
    opacity: 0.6;
  }

  .chevron {
    flex: none;
    fill: none;
    stroke: var(--primary);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .choice:disabled .chevron {
    opacity: 0.4;
  }

  /* Delete…: a danger outline, not a fill. The fill is kept for the final
     button on the last step. */
  .choice.destructive {
    border-color: var(--danger);
    background: transparent;
  }

  .choice.destructive .choice-title {
    color: var(--danger);
  }

  .choice.destructive:hover:not([aria-disabled='true']) {
    border-color: var(--danger);
    background: var(--surface);
  }

  .choice.destructive:focus-visible {
    outline-color: var(--danger);
  }

  .tag {
    flex: none;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    line-height: 150%;
    white-space: nowrap;
  }

  .tag.safe {
    border-color: var(--primary);
    color: var(--text-h);
  }

  /* Delete confirmation. */
  .summary-box {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-left: 3px solid var(--danger);
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .summary {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-h);
  }

  /* Move confirmation. The summary card's edge carries the tone: --primary
     when the move is instant or the space is ample, --gold (theme-aware:
     about 10:1 on the dark panel, about 5.5:1 on the light one) when it is
     tight or unknown, --danger when the new drive is short. */
  .summary-box.tone-ok {
    border-left-color: var(--primary);
  }

  .summary-box.tone-warn {
    border-left-color: var(--gold);
  }

  .summary-box.tone-bad {
    border-left-color: var(--danger);
  }

  .summary-sub {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .summary-hint,
  .while-warn {
    margin: 4px 0 0;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 12px;
    line-height: 150%;
    color: var(--text-muted);
  }

  .tone-warn .summary-hint,
  .tone-bad .summary-hint {
    color: var(--text-h);
  }

  .hint-icon {
    flex: none;
    margin-top: 1px;
    fill: none;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tone-warn .hint-icon,
  .while-warn .hint-icon {
    stroke: var(--gold);
  }

  .tone-bad .hint-icon {
    stroke: var(--danger);
  }

  /* From / To as one compact card of labelled paths. */
  .route {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 8px 12px;
  }

  /* The folders that move: one row each, name and size on the left, how it
     moves as a tag on the right. The full line is read out by a screen reader
     from the hidden span, so the visual columns are decoration. */
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .folder {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .folder-name {
    font-family: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .folder-size {
    font-variant-numeric: tabular-nums;
    color: var(--text-h);
  }

  /* "While moving": what waits, and the one thing to do first. */
  .while {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    border-radius: var(--r-row);
    border: 1px dashed var(--border);
  }

  .while-lead {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .waits {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
    color: var(--text-h);
  }

  .while-warn {
    margin-top: 4px;
    padding: 8px 10px;
    border-left: 3px solid var(--gold);
    border-radius: var(--r-row);
    background: var(--surface);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: min(240px, 35vh);
    overflow-y: auto;
    border-radius: var(--r-row);
  }

  .list:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .game {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .game-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .game-title {
    min-width: 0;
    overflow-wrap: anywhere;
    font-weight: 600;
    color: var(--text-h);
  }

  .game-size {
    flex: none;
    font-variant-numeric: tabular-nums;
    color: var(--text-h);
  }

  .game-platform {
    font-size: 12px;
    color: var(--text-muted);
  }

  .kept {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px dashed var(--border);
  }

  .kept-note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  /* An error line in the same calm shape as the Settings notices. */
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

  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .btn:hover:not(:disabled):not([aria-disabled='true']) {
    background: var(--surface);
  }

  .btn:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  .btn:disabled,
  .btn[aria-disabled='true'] {
    opacity: 0.6;
    cursor: default;
  }

  /* The primary action ("Start move"). */
  .btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: var(--on-primary);
    font-weight: 600;
  }

  .btn.primary:hover:not(:disabled):not([aria-disabled='true']) {
    background: var(--primary-hover);
    border-color: var(--primary-hover);
  }

  /* Same danger fill as RemoveConfirm. */
  .btn.danger {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
    font-weight: 600;
  }

  .btn.danger:hover:not(:disabled):not([aria-disabled='true']) {
    background: var(--danger);
    filter: brightness(1.1);
  }

  .btn.danger:focus-visible {
    outline-color: var(--danger);
  }
</style>
