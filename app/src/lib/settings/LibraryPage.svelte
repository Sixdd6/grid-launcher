<script lang="ts">
  // Settings › Library (Q2): the current library folder and Change…, which
  // opens LibraryChangeDialog. After a change the page shows what happened;
  // the installed store re-reads the registry on the backend's
  // `installed-changed` event.
  import { api, type StartFreshOutcome } from '../api';
  import LibraryChangeDialog from './LibraryChangeDialog.svelte';
  import { outcomeNotice, type Notice } from './library';

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
    if (active && !dialogOpen) load();
  });

  function openDialog() {
    notice = null;
    dialogOpen = true;
  }

  function onDone(outcome: StartFreshOutcome, deleted: boolean) {
    notice = outcomeNotice(outcome, deleted);
    if (outcome.status === 'switched') path = outcome.library_path;
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
        aria-describedby="library-folder-hint"
        onclick={openDialog}
        disabled={path === null}
      >
        Change…
      </button>
    </div>

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
  <LibraryChangeDialog currentPath={path} {onDone} onClose={() => (dialogOpen = false)} />
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
    gap: 8px;
  }

  /* The label is the theme background rather than white: white on the dark
     theme's #8b74e8 is about 3.6:1, `--bg` on it is about 5.4:1 (and about
     7.6:1 on the light theme's #553e98). */
  .primary {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: var(--bg);
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

  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
