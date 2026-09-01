<script lang="ts">
  import { api, type EmulatorEntry, type LaunchDefaults, type Platform } from './api';

  let { onClose }: { onClose: () => void } = $props();

  let emulators = $state<EmulatorEntry[]>([]);
  let listLoading = $state(true);
  let listError = $state<string | null>(null);

  let platforms = $state<Platform[]>([]);
  let defaults = $state<LaunchDefaults | null>(null);
  let defaultsError = $state<string | null>(null);

  // 'new' for the add form, or the entry's current name while editing it —
  // saveEmulator's originalName arg so a rename can find & replace itself.
  let editing = $state<'new' | string | null>(null);
  let formName = $state('');
  let formPath = $state('');
  let formArgs = $state('');
  let formError = $state<string | null>(null);
  let formPending = $state(false);

  let confirmingDelete = $state<string | null>(null);
  let deletePending = $state<string | null>(null);

  let panelEl = $state<HTMLElement | null>(null);

  $effect(() => {
    panelEl?.focus();
  });

  $effect(() => {
    refreshEmulators();
    refreshPlatformsAndDefaults();
  });

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function refreshEmulators() {
    listLoading = true;
    try {
      emulators = await api.listEmulators();
      listError = null;
    } catch (err) {
      listError = errorMessage(err);
    } finally {
      listLoading = false;
    }
  }

  async function refreshPlatformsAndDefaults() {
    try {
      const [p, d] = await Promise.all([api.listPlatforms(), api.getLaunchDefaults()]);
      platforms = p;
      defaults = d;
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  async function refreshDefaults() {
    try {
      defaults = await api.getLaunchDefaults();
      defaultsError = null;
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  function openAdd() {
    editing = 'new';
    formName = '';
    formPath = '';
    formArgs = '';
    formError = null;
  }

  function openEdit(entry: EmulatorEntry) {
    editing = entry.name;
    formName = entry.name;
    formPath = entry.path;
    formArgs = entry.args;
    formError = null;
  }

  function closeForm() {
    editing = null;
    formError = null;
  }

  async function autoFillFromPath() {
    if (formName.trim() !== '' || formArgs.trim() !== '') return;
    const path = formPath.trim();
    if (!path) return;
    try {
      const profile = await api.matchProfile(path);
      if (profile) {
        formName = profile.name;
        formArgs = profile.args;
      }
    } catch {
      // Best-effort autofill only — leave the form as typed on failure.
    }
  }

  function onPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      autoFillFromPath();
    }
  }

  async function saveForm() {
    if (editing === null) return;
    const originalName = editing === 'new' ? '' : editing;
    // Backend stores the name as-given; trim client-side so a name typed
    // with stray whitespace doesn't get persisted verbatim.
    const entry: EmulatorEntry = { name: formName.trim(), path: formPath, args: formArgs };
    formError = null;
    formPending = true;
    try {
      await api.saveEmulator(originalName, entry);
      closeForm();
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      formError = errorMessage(err);
    } finally {
      formPending = false;
    }
  }

  async function handleDeleteClick(name: string) {
    if (confirmingDelete !== name) {
      confirmingDelete = name;
      return;
    }
    deletePending = name;
    try {
      await api.deleteEmulator(name);
      await refreshEmulators();
      await refreshDefaults();
    } catch (err) {
      listError = errorMessage(err);
    } finally {
      deletePending = null;
      confirmingDelete = null;
    }
  }

  function defaultFor(platformName: string): string {
    if (!defaults) return '(none)';
    const folded = platformName.toLowerCase();
    const key = Object.keys(defaults.default_emulators).find((k) => k.toLowerCase() === folded);
    return key ? defaults.default_emulators[key] : '(none)';
  }

  async function handleDefaultChange(platformName: string, value: string) {
    try {
      await api.setDefaultEmulator(platformName, value === '(none)' ? '' : value);
      await refreshDefaults();
    } catch (err) {
      defaultsError = errorMessage(err);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label="Emulators"
    tabindex="-1"
    onkeydown={onKey}
  >
    <button class="close" onclick={onClose} aria-label="Close">×</button>
    <h2>Emulators</h2>

    <section class="list-section">
      <div class="section-header">
        <h3>Installed emulators</h3>
        <button class="add-btn" onclick={openAdd}>+ Add emulator</button>
      </div>

      {#if listLoading}
        <p class="muted">Loading…</p>
      {:else if listError}
        <p class="error" role="alert">{listError}</p>
      {:else if emulators.length === 0}
        <p class="muted">No emulators configured.</p>
      {:else}
        <ul class="emulator-list">
          {#each emulators as e (e.name)}
            <li class="emulator-row">
              <div class="row-text">
                <span class="name">{e.name}</span>
                <span class="path" title={e.path}>{e.path}</span>
                {#if e.args}<span class="args">{e.args}</span>{/if}
              </div>
              <div class="row-actions">
                <button onclick={() => openEdit(e)}>Edit</button>
                <button
                  class:confirm={confirmingDelete === e.name}
                  disabled={deletePending === e.name}
                  onclick={() => handleDeleteClick(e.name)}
                >
                  {confirmingDelete === e.name ? 'Confirm delete' : 'Delete'}
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    {#if editing !== null}
      <section class="form-section">
        <h3>{editing === 'new' ? 'Add emulator' : 'Edit emulator'}</h3>
        <form
          onsubmit={(e) => {
            e.preventDefault();
            saveForm();
          }}
        >
          <label>Name <input bind:value={formName} required /></label>
          <label>
            Executable path
            <input bind:value={formPath} onblur={autoFillFromPath} onkeydown={onPathKeydown} />
          </label>
          <label>Arguments <input bind:value={formArgs} /></label>
          {#if formError}<p class="error" role="alert">{formError}</p>{/if}
          <div class="form-actions">
            <button type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
            <button type="button" onclick={closeForm} disabled={formPending}>Cancel</button>
          </div>
        </form>
      </section>
    {/if}

    <section class="defaults-section">
      <h3>Per-platform defaults</h3>
      {#if defaultsError}
        <p class="error" role="alert">{defaultsError}</p>
      {/if}
      {#if platforms.length === 0}
        <p class="muted">No platforms available.</p>
      {:else}
        <ul class="defaults-list">
          {#each platforms as p (p.id)}
            <li class="defaults-row">
              <span class="platform-name">{p.name}</span>
              <select
                value={defaultFor(p.name)}
                onchange={(e) => handleDefaultChange(p.name, (e.currentTarget as HTMLSelectElement).value)}
              >
                <option value="(none)">(none)</option>
                {#each emulators as em (em.name)}
                  <option value={em.name}>{em.name}</option>
                {/each}
              </select>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 20;
  }

  .panel {
    position: relative;
    width: min(560px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    overflow-x: hidden;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    line-height: 1;
    font-size: 20px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .close:hover,
  .close:focus-visible {
    background: var(--border);
  }

  h2 {
    margin: 0;
    color: var(--text-h);
    font-size: 18px;
    padding-right: 28px;
  }

  h3 {
    margin: 0;
    color: var(--text-h);
    font-size: 14px;
  }

  .muted {
    margin: 0;
    color: var(--text);
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: #e5484d;
    font-size: 13px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .list-section,
  .form-section,
  .defaults-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .list-section {
    border-top: none;
    padding-top: 0;
  }

  .add-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .add-btn:hover {
    background: var(--border);
  }

  .emulator-list,
  .defaults-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .emulator-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--border);
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    color: var(--text-h);
    font-weight: 500;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path {
    color: var(--text);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 320px;
  }

  .args {
    color: var(--text);
    font-size: 11px;
    opacity: 0.8;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .row-actions button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .row-actions button.confirm {
    background: #e5484d;
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .form-section form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .form-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  .form-section input {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .form-section input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .form-actions button[type='button'] {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-h);
  }

  .form-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .defaults-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .platform-name {
    color: var(--text-h);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .defaults-row select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
    max-width: 200px;
  }
</style>
