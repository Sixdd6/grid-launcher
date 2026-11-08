<script lang="ts">
  // Settings › Systems' settings sheet (ProposedApp README: "The settings
  // sheet is the same for adding and editing"): "Which emulator is it?"
  // when adding, Name, Program, Launch options, and the cloud fields folded
  // under "Cloud saves (advanced)". The fields seed from `entry` on mount
  // only — a parent that changes which entry is edited wraps this in
  // `{#key entry.name}`.
  import { api, type EmulatorEntry, type ProfileSummary } from '../api';
  import { pickFile } from '../pickers';
  import {
    addedEmulatorToast,
    applyWhichEmulator,
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
  {#if formError}<p data-testid="emu-form-error" class="g-error" role="alert">{formError}</p>{/if}
  <div class="g-actions">
    <button class="g-btn" data-testid="emu-form-save" type="submit" disabled={formPending}>{formPending ? 'Saving…' : 'Save'}</button>
    <button class="g-btn secondary" data-testid="emu-form-cancel" type="button" onclick={onCancel} disabled={formPending}>Cancel</button>
  </div>
{/snippet}

<form
  class="g-form"
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
        class="g-field"
        data-testid="emu-form-which"
        value={whichValue}
        onchange={(e) => onWhichChange((e.currentTarget as HTMLSelectElement).value)}
      >
        {#each profiles as profile (profile.name)}
          <option value={profile.name}>{profile.name}</option>
        {/each}
        <option value={SOMETHING_ELSE}>Something else</option>
      </select>
      <span class="g-help">GRID fills in the rest for emulators it knows.</span>
    </label>
  {/if}
  <!-- Adding: asked only for Something else (a known emulator brings its
       name). Editing: always, so an entry can be renamed — the save finds
       and replaces it by its old name, in place. -->
  {#if mode === 'edit' || whichValue === SOMETHING_ELSE}
    <label>
      Name
      <input class="g-field" data-testid="emu-form-name" bind:value={formName} required />
    </label>
  {/if}
  <label>
    Program
    <span class="path-row">
      <input
        class="g-field"
        data-testid="emu-form-path"
        bind:value={formPath}
        placeholder="Choose the emulator's program file"
        onblur={autoFillFromPath}
        onkeydown={onPathKeydown}
      />
      <button class="g-btn secondary" data-testid="emu-form-path-browse" type="button" disabled={formPending} onclick={browseEmulatorPath}>
        Browse…
      </button>
    </span>
  </label>
  <label>
    Launch options
    <input class="g-field" data-testid="emu-form-args" bind:value={formArgs} />
    <span class="g-help">%rom% is replaced by the game file.</span>
  </label>
  <details class="advanced">
    <summary data-testid="emu-form-advanced">Cloud saves (advanced)</summary>
    <div class="advanced-fields">
      <label>
        How this emulator stores saves
        <select class="g-field" data-testid="emu-form-save-strategy" bind:value={formSaveStrategy}>
          {#each SAVE_STRATEGIES as strategy (strategy)}
            <option value={strategy}>{SAVE_STRATEGY_LABELS[strategy]}</option>
          {/each}
        </select>
      </label>
      <label>
        Save folders
        <input class="g-field" data-testid="emu-form-save-paths" bind:value={formSavePaths} placeholder="memcards; saves" />
        <span class="g-help">Separate folders with ;</span>
      </label>
      <label>
        Save state folders
        <input class="g-field" data-testid="emu-form-state-paths" bind:value={formStatePaths} placeholder="savestates" />
      </label>
      <label>
        Don't upload these files
        <input class="g-field" data-testid="emu-form-ignore-files" bind:value={formIgnoreFiles} placeholder="settings.ini" />
      </label>
      <label>
        Don't upload these file types
        <input class="g-field" data-testid="emu-form-ignore-extensions" bind:value={formIgnoreExtensions} placeholder=".log; .tmp" />
      </label>
    </div>
  </details>
  {@render actions()}
</form>

<style>
  .path-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .path-row input {
    flex: 1;
  }

  .path-row button {
    padding: 0 14px;
    height: var(--control-h);
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
</style>
