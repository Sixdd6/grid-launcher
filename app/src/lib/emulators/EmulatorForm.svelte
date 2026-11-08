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
    applyWhichEmulator,
    ARGS_LABEL,
    emulatorFormValues,
    entryPatch,
    normalizeSaveStrategy,
    SAVE_STRATEGIES,
    SAVE_STRATEGY_LABELS,
    SOMETHING_ELSE,
  } from './form';
  import { pushToast } from '../stores/toasts.svelte';

  let {
    mode,
    entry = null,
    profiles,
    onSaved,
    onCancel,
    variant = 'classic',
  }: {
    mode: 'add' | 'edit';
    entry?: EmulatorEntry | null;
    profiles: ProfileSummary[];
    onSaved: () => void;
    onCancel: () => void;
    /** `sheet`: Settings › Systems' settings sheet ("Which emulator is it?",
     *  Program, Launch options, the cloud fields folded under "Cloud saves
     *  (advanced)"). `classic`: the Emulators view's form. Same fields and
     *  the same save either way. */
    variant?: 'classic' | 'sheet';
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

  // The sheet's "Which emulator is it?": a profile fills the name and its
  // launch options; Something else asks for a name.
  let whichValue = $state(SOMETHING_ELSE);
  let whichFilled = $state<{ name: string; args: string } | null>(null);

  function onWhichChange(value: string) {
    whichValue = value;
    const profile = profiles.find((p) => p.name === value) ?? null;
    const next = applyWhichEmulator(profile, { name: formName, args: formArgs }, whichFilled);
    formName = next.name;
    formArgs = next.args;
    whichFilled = profile === null ? null : next;
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

{#snippet actions()}
  {#if formError}<p data-testid="emu-form-error" class="error" role="alert">{formError}</p>{/if}
  <div class="form-actions">
    <button data-testid="emu-form-save" type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
    <button data-testid="emu-form-cancel" type="button" onclick={onCancel} disabled={formPending}>Cancel</button>
  </div>
{/snippet}

{#if variant === 'sheet'}
<form
  data-testid="emu-form"
  onsubmit={(e) => {
    e.preventDefault();
    save();
  }}
>
  {#if mode === 'add'}
    <label>
      Which emulator is it?
      <select
        data-testid="emu-form-which"
        value={whichValue}
        onchange={(e) => onWhichChange((e.currentTarget as HTMLSelectElement).value)}
      >
        {#each profiles as profile (profile.name)}
          <option value={profile.name}>{profile.name}</option>
        {/each}
        <option value={SOMETHING_ELSE}>Something else</option>
      </select>
      <span class="help">GRID fills in the rest for emulators it knows.</span>
    </label>
    {#if whichValue === SOMETHING_ELSE}
      <label>
        Name
        <input data-testid="emu-form-name" bind:value={formName} required />
      </label>
    {/if}
  {/if}
  <label>
    Program
    <span class="path-row">
      <input
        data-testid="emu-form-path"
        bind:value={formPath}
        placeholder="Choose the emulator's program file"
        onblur={autoFillFromPath}
        onkeydown={onPathKeydown}
      />
      <button data-testid="emu-form-path-browse" type="button" disabled={formPending} onclick={browseEmulatorPath}>
        Browse…
      </button>
    </span>
  </label>
  <label>
    Launch options
    <input data-testid="emu-form-args" bind:value={formArgs} />
    <span class="help">%rom% is replaced by the game file.</span>
  </label>
  <details class="advanced">
    <summary data-testid="emu-form-advanced">Cloud saves (advanced)</summary>
    <div class="advanced-fields">
      <label>
        How this emulator stores saves
        <select data-testid="emu-form-save-strategy" bind:value={formSaveStrategy}>
          {#each SAVE_STRATEGIES as strategy (strategy)}
            <option value={strategy}>{SAVE_STRATEGY_LABELS[strategy]}</option>
          {/each}
        </select>
      </label>
      <label>
        Save folders
        <input data-testid="emu-form-save-paths" bind:value={formSavePaths} placeholder="memcards; saves" />
        <span class="help">Separate folders with ;</span>
      </label>
      <label>
        Save state folders
        <input data-testid="emu-form-state-paths" bind:value={formStatePaths} placeholder="savestates" />
      </label>
      <label>
        Don't upload these files
        <input data-testid="emu-form-ignore-files" bind:value={formIgnoreFiles} placeholder="settings.ini" />
      </label>
      <label>
        Don't upload these file types
        <input data-testid="emu-form-ignore-extensions" bind:value={formIgnoreExtensions} placeholder=".log; .tmp" />
      </label>
    </div>
  </details>
  {@render actions()}
</form>
{:else}
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
  {@render actions()}
</form>
{/if}

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

  .help {
    font-size: 11px;
    color: var(--text-muted);
  }

  .advanced summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 0;
  }

  .advanced summary:hover,
  .advanced[open] summary {
    color: var(--text-h);
  }

  .advanced-fields {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 10px;
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
    color: var(--on-primary);
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
