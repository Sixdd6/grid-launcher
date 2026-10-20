<script lang="ts">
  // Settings › Library (Q2): the current library folder and Change…, which
  // opens LibraryChangeDialog. After a change the page shows what happened;
  // the installed store re-reads the registry on the backend's
  // `installed-changed` event.
  //
  // 9b: while a library move runs or waits, the pane says so ("Moving to
  // <new>…", or the stopped / waiting wording) and Change… is disabled, with
  // the reason beside it; the folder is re-read when the move (or a relocate)
  // changes it.
  import { api, type LibraryMoveStartOutcome, type StartFreshOutcome } from '../api';
  import { libraryMove } from '../stores/libraryMove.svelte';
  import LibraryChangeDialog from './LibraryChangeDialog.svelte';
  import { moveInProgress, moveStartNotice, outcomeNotice, paneMoveText, type Notice } from './library';
  import { pushToast } from '../stores/toasts.svelte';
  import { SKIPPED_TOAST_MS, skippedRemovalText } from '../uninstallSkips';

  let { active = true }: { active?: boolean } = $props();

  let path = $state<string | null>(null);
  let loadError = $state<string | null>(null);
  let dialogOpen = $state(false);
  let notice = $state<Notice | null>(null);

  function load() {
    api
      .getLibraryPath()
      .then((p) => {
        path = p;
        loadError = null;
      })
      .catch((err) => {
        loadError = err instanceof Error ? err.message : String(err);
      });
  }

  // Re-read whenever the page comes forward: First Run Setup or the Server
  // banner may have set the folder since.
  $effect(() => {
    // Read so a finished move or a relocate re-reads the folder.
    void libraryMove.pathVersion;
    if (active && !dialogOpen) load();
  });

  let moveText = $derived(paneMoveText(libraryMove.status));
  let moving = $derived(moveInProgress(libraryMove.status));
  // Presentation only: the move is running, or stopped with an error.
  let running = $derived(moving && libraryMove.status?.running === true);
  let stopped = $derived(moving && !running && libraryMove.status?.error != null);

  function onMoveStarted(outcome: LibraryMoveStartOutcome) {
    notice = moveStartNotice(outcome);
    dialogOpen = false;
  }

  function openDialog() {
    notice = null;
    dialogOpen = true;
  }

  function onDone(outcome: StartFreshOutcome, deleted: boolean) {
    notice = outcomeNotice(outcome, deleted);
    if (outcome.status === 'switched') {
      path = outcome.library_path;
      pushToast(skippedRemovalText(outcome.skipped), 'error', SKIPPED_TOAST_MS);
    }
    dialogOpen = false;
  }
</script>

<div data-testid="settings-library-pane" class="library">
  <section class="section" aria-labelledby="library-folder-title">
    <h3 id="library-folder-title">Library folder</h3>
    <p id="library-folder-hint" class="hint">
      Games, emulators and saves live in this folder. New installs go here.
    </p>

    {#if loadError}
      <p class="notice error" role="alert">{loadError}</p>
    {:else}
      <code
        data-testid="library-path-value"
        class="path"
        class:unset={path !== null && path.trim() === ''}>{path === null ? '…' : path.trim() === '' ? 'Not set' : path}</code
      >
    {/if}

    <div class="actions">
      <button
        data-testid="library-path-change"
        class="primary"
        aria-describedby={moving ? 'library-folder-hint library-change-reason' : 'library-folder-hint'}
        onclick={openDialog}
        disabled={path === null || moving}
      >
        Change…
      </button>
      {#if moving}
        <!-- A disabled button cannot take focus, so the reason is plain text
             beside it, and also its description. -->
        <p id="library-change-reason" class="reason">
          Unavailable while the library is being moved.
        </p>
      {/if}
    </div>

    {#if moveText}
      <!-- Running: a calm progress card (indeterminate bar, the real numbers
           are on the Downloads row). Stopped or waiting: the same card with a
           gold edge and the consequence in plain words. The text itself comes
           from `paneMoveText`. -->
      <div class="move" class:stopped>
        {#if running}
          <svg class="move-icon spin" viewBox="0 0 16 16" width="18" height="18" aria-hidden="true" focusable="false">
            <path d="M8 2a6 6 0 1 1-6 6" />
          </svg>
        {:else}
          <svg class="move-icon" viewBox="0 0 16 16" width="18" height="18" aria-hidden="true" focusable="false">
            {#if stopped}
              <path d="M8 2.5l6 10.5H2z" />
              <path d="M8 6.8v2.9M8 11.4v.1" />
            {:else}
              <path d="M6 3.5v9M10 3.5v9" />
            {/if}
          </svg>
        {/if}
        <div class="move-body">
          <p data-testid="library-move-state" class="move-text" role="status">{moveText}</p>
          <p class="move-hint">
            {#if running}
              Follow the progress in Downloads. Installs, launches and cloud transfers wait until the move ends.
            {:else}
              Installs, launches and cloud transfers stay paused until the move ends.
            {/if}
          </p>
          {#if running}
            <span class="bar-track" aria-hidden="true"><span class="bar-fill"></span></span>
          {/if}
        </div>
      </div>
    {/if}

    {#if notice}
      <p
        data-testid="library-change-notice"
        class="notice"
        class:error={notice.kind === 'error'}
        role={notice.kind === 'error' ? 'alert' : 'status'}
      >
        {notice.text}
      </p>
    {/if}
  </section>
</div>

{#if dialogOpen && path !== null}
  <LibraryChangeDialog currentPath={path} {onDone} {onMoveStarted} onClose={() => (dialogOpen = false)} />
{/if}

<style>
  .library {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 720px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .hint {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
    max-width: 60ch;
  }

  /* The folder as a value: monospace, wraps at any character so a long path
     never widens the pane, selectable for copying. */
  .path {
    display: block;
    padding: 10px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
    font-family: ui-monospace, 'Cascadia Mono', Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 12px;
    line-height: 150%;
    overflow-wrap: anywhere;
    user-select: text;
  }

  /* "Not set" is a status, not a path: proportional and muted. */
  .path.unset {
    font-family: inherit;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* A calm notice, as on the Connect form's reason line: the neutral
     `--surface` fill with a `--primary` edge and plain text colour. The error
     variant swaps the edge for `--danger`; the text stays plain so contrast
     holds in both themes. */
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  /* Why Change… is off, next to the button. */
  .reason {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* The move's state: the calm notice shape (surface fill, --primary edge,
     plain text) with an icon and, while it runs, a thin indeterminate bar.
     Stopped or waiting swaps the edge for --gold (theme-aware) and the icon
     for a warning; the words say the same, so colour is not the only cue. */
  .move {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 12px;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-row);
    background: var(--surface);
    color: var(--text-h);
  }

  .move.stopped {
    border-left-color: var(--gold);
  }

  .move-icon {
    flex: none;
    margin-top: 1px;
    fill: none;
    stroke: var(--primary);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .move.stopped .move-icon {
    stroke: var(--gold);
  }

  .move-icon.spin {
    animation: move-spin 1.1s linear infinite;
  }

  @keyframes move-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .move-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .move-text {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .move-hint {
    margin: 0;
    font-size: 12px;
    line-height: 150%;
    color: var(--text-muted);
    max-width: 60ch;
  }

  .bar-track {
    position: relative;
    display: block;
    height: 4px;
    margin-top: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .bar-fill {
    position: absolute;
    top: 0;
    left: 0;
    width: 35%;
    height: 100%;
    border-radius: 2px;
    background: var(--primary);
    animation: move-slide 1.1s ease-in-out infinite;
  }

  @keyframes move-slide {
    0% {
      left: -35%;
    }
    100% {
      left: 100%;
    }
  }

  /* No motion for people who ask for none: the spinner stops and the bar
     rests at one end. */
  @media (prefers-reduced-motion: reduce) {
    .move-icon.spin,
    .bar-fill {
      animation: none;
    }

    .bar-fill {
      left: 0;
    }
  }

  /* The label is the theme background rather than white: white on the dark
     theme's #8b74e8 is about 3.6:1, `--bg` on it is about 5.4:1 (and about
     7.6:1 on the light theme's #553e98). */
  .primary {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: 1px solid transparent;
    background: var(--primary);
    color: var(--on-primary);
    font-weight: 600;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .primary:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }

  /* Visibly off: a dashed outline and muted text rather than a faded fill, so
     it reads as unavailable and its label stays legible. */
  .primary:disabled {
    background: transparent;
    border: 1px dashed var(--border);
    color: var(--text-muted);
    cursor: default;
  }
</style>
