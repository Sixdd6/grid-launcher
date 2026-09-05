<script lang="ts">
  // The manual add / edit form (design §9). One component, two hosts: the
  // Installed pane renders it as the edit sheet, the catalog pane as its
  // Manual tab. The fields seed from `entry` on mount only — a parent that
  // changes which entry is edited wraps this in `{#key entry.name}`.
  import { api, type EmulatorEntry, type ProfileSummary } from '../api';
  import { matchProfileByName, shouldAutoFillFromName } from './catalog';
  import { pickFile } from '../pickers';
  import {
    addedEmulatorToast,
    ARGS_LABEL,
    emulatorFormValues,
    entryPatch,
    normalizeSaveStrategy,
    SAVE_STRATEGIES,
  } from './form';
  import { pushToast } from '../stores/toasts.svelte';

  let {
    mode,
    entry = null,
    profiles,
    onSaved,
    onCancel,
  }: {
    mode: 'add' | 'edit';
    entry?: EmulatorEntry | null;
    profiles: ProfileSummary[];
    onSaved: () => void;
    onCancel: () => void;
  } = $props();

  // Seeded from `entry` once, on purpose: the fields are then the user's to
  // edit. A parent that switches which entry is edited remounts this with
  // `{#key entry.name}` rather than expecting the fields to track the prop.
  // svelte-ignore state_referenced_locally
  const seed = emulatorFormValues(entry);
  let formName = $state(seed.name);
  let formPath = $state(seed.path);
  let formArgs = $state(seed.args);
  let formSaveStrategy = $state<string>(seed.saveStrategy);
  let formIgnoreFiles = $state(seed.ignoreFiles);
  let formIgnoreExtensions = $state(seed.ignoreExtensions);
  let formSavePaths = $state(seed.savePaths);
  let formStatePaths = $state(seed.statePaths);
  let formError = $state<string | null>(null);
  let formPending = $state(false);
  let autofillMatch = $state<ProfileSummary | null>(null);

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
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

  // Manual-add auto-fill from the typed NAME (task-7-brief.md): add mode
  // only, and only when path and args are both still empty, so it never
  // clobbers a manually typed or path-derived value and never touches an
  // entry being edited. Fires on blur/input of the name field.
  function autoFillFromName() {
    if (!shouldAutoFillFromName(mode, formPath, formArgs)) {
      autofillMatch = null;
      return;
    }
    const match = matchProfileByName(formName, profiles);
    autofillMatch = match;
    if (match) {
      formArgs = match.args;
    }
  }

  // No filter: this one field accepts a bare executable, an AppImage or a
  // downloadable archive — Python labels it "Executable / Archive Path" for
  // a new entry (dialogs.py:355).
  async function browseEmulatorPath() {
    const picked = await pickFile('Select Emulator Executable or Archive');
    if (picked === null) return;
    formPath = picked;
    await autoFillFromPath();
  }

  function onPathKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      autoFillFromPath();
    }
  }

  async function save() {
    // `originalName` is what `save_emulator` uses to find-and-replace a
    // renamed entry; blank means "insert". The fields the form does not
    // show (install provenance, autoconfig paths) are spread back from
    // `entry` untouched instead of being dropped on save.
    const originalName = mode === 'add' ? '' : (entry?.name ?? '');
    const patch = entryPatch({
      name: formName,
      path: formPath,
      args: formArgs,
      saveStrategy: normalizeSaveStrategy(formSaveStrategy),
      ignoreFiles: formIgnoreFiles,
      ignoreExtensions: formIgnoreExtensions,
      savePaths: formSavePaths,
      statePaths: formStatePaths,
    });
    const next: EmulatorEntry = {
      ...(mode === 'edit' && entry ? entry : {}),
      ...patch,
    };
    formError = null;
    formPending = true;
    try {
      await api.saveEmulator(originalName, next);
      // `_save_emulator_entry` toasts only for a NEW manual entry
      // (emulator_ui_mixin.py:1590-1591) — an edit stays silent.
      if (mode === 'add') pushToast(addedEmulatorToast(patch.name));
      onSaved();
    } catch (err) {
      formError = errorMessage(err);
    } finally {
      formPending = false;
    }
  }
</script>

<form
  data-testid="emu-form"
  onsubmit={(e) => {
    e.preventDefault();
    save();
  }}
>
  <label>
    Name
    <input
      data-testid="emu-form-name"
      bind:value={formName}
      onblur={autoFillFromName}
      oninput={autoFillFromName}
      required
    />
  </label>
  {#if autofillMatch}
    <p data-testid="emu-autofill-hint" class="hint">Matched profile: {autofillMatch.name}</p>
  {/if}
  <label>
    Executable path
    <span class="path-row">
      <input data-testid="emu-form-path" bind:value={formPath} onblur={autoFillFromPath} onkeydown={onPathKeydown} />
      <button data-testid="emu-form-path-browse" type="button" disabled={formPending} onclick={browseEmulatorPath}>
        Browse…
      </button>
    </span>
  </label>
  <label>
    <span data-testid="emu-form-args-label">{ARGS_LABEL}</span>
    <input data-testid="emu-form-args" bind:value={formArgs} />
  </label>
  <label>
    Save Strategy
    <select data-testid="emu-form-save-strategy" bind:value={formSaveStrategy}>
      {#each SAVE_STRATEGIES as strategy (strategy)}
        <option value={strategy}>{strategy}</option>
      {/each}
    </select>
  </label>
  <label>
    Ignore Files (; separated)
    <input data-testid="emu-form-ignore-files" bind:value={formIgnoreFiles} />
  </label>
  <label>
    Ignore Extensions (; separated)
    <input data-testid="emu-form-ignore-extensions" bind:value={formIgnoreExtensions} />
  </label>
  <label>
    Save Dirs (; separated)
    <input data-testid="emu-form-save-paths" bind:value={formSavePaths} />
  </label>
  <label>
    State Dirs (; separated)
    <input data-testid="emu-form-state-paths" bind:value={formStatePaths} />
  </label>
  {#if formError}<p data-testid="emu-form-error" class="error" role="alert">{formError}</p>{/if}
  <div class="form-actions">
    <button data-testid="emu-form-save" type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
    <button data-testid="emu-form-cancel" type="button" onclick={onCancel} disabled={formPending}>Cancel</button>
  </div>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  input {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  select {
    font: inherit;
    padding: 8px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  select:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .path-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .path-row input {
    flex: 1;
    min-width: 0;
  }

  .path-row button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .path-row button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .form-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .form-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
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
</style>
