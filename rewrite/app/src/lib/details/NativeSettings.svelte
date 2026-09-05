<script lang="ts">
  // Native Game Settings dialog (task-16-brief.md): executable candidate
  // picker, launch parameters, a compat-tool picker on non-Windows hosts,
  // and a read-only wine-prefix line. Same modal shape as Details.svelte
  // itself (backdrop + `role="dialog"`, Escape/backdrop/close-button
  // dismissal) so it reads as the same design language stacked on top.
  import { api, type CompatTool, type NativeGameSettings } from '../api';
  import { candidateLabel, installDirOf, isWindowsHost } from './actions';

  let {
    romId,
    title,
    onClose,
    onSaved,
  }: {
    romId: number;
    title: string;
    onClose: () => void;
    onSaved: () => void;
  } = $props();

  const NONE_COMPAT = '';

  // `navigator.platform` is read once per mount through the pure
  // `isWindowsHost` seam rather than branched on inline.
  const windowsHost = isWindowsHost(navigator.platform);

  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let settings = $state<NativeGameSettings | null>(null);

  let selectedExecutable = $state('');
  let parameters = $state('');
  let selectedCompat = $state(NONE_COMPAT);

  let compatTools = $state<CompatTool[]>([]);
  let compatToolsError = $state<string | null>(null);

  let saving = $state(false);
  let saveError = $state<string | null>(null);

  let panelEl = $state<HTMLElement | null>(null);

  // The backend's own answer when it has one; the shallowest candidate's
  // directory otherwise, so a row built before `install_dir` existed still
  // labels its options (`installDirOf`, details/actions.ts).
  let installDir = $derived(settings?.install_dir || installDirOf(settings?.candidates ?? []));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function load() {
    loading = true;
    loadError = null;
    try {
      const result = await api.nativeGameSettings(romId);
      settings = result;
      selectedExecutable = result.executable || result.candidates[0] || '';
      parameters = result.parameters;

      if (!windowsHost) {
        try {
          const dto = await api.listCompatTools();
          compatTools = dto.tools;
          // The game's own compat tool, falling back to the configured default.
          selectedCompat = result.compat_tool || dto.default_tool;
        } catch (err) {
          compatToolsError = errorMessage(err);
        }
      }
    } catch (err) {
      loadError = errorMessage(err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  $effect(() => {
    panelEl?.focus();
  });

  async function handleSave() {
    if (!settings || settings.candidates.length === 0) return;
    saving = true;
    saveError = null;
    try {
      await api.setNativeGameSettings(romId, selectedExecutable, parameters, selectedCompat);
      onSaved();
      onClose();
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      saving = false;
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
    data-testid="native-settings"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-label={`Game Settings — ${title}`}
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="native-settings-close" class="close" onclick={onClose} aria-label="Close">×</button>
    <h3>Game Settings</h3>

    {#if loading}
      <p class="hint">Loading…</p>
    {:else if settings}
      {#if settings.candidates.length === 0}
        <p data-testid="native-settings-empty" class="hint">
          No launchable executables were found in this game's install directory.
        </p>
      {:else}
        <div class="row">
          <span class="row-label">Install Directory</span>
          <p data-testid="native-settings-install-dir" class="row-value">{installDir || '(not found)'}</p>
        </div>
        <label>
          Executable
          <select data-testid="native-settings-exe" bind:value={selectedExecutable}>
            {#each settings.candidates as candidate (candidate)}
              <option value={candidate}>{candidateLabel(candidate, installDir)}</option>
            {/each}
          </select>
        </label>
      {/if}

      <div class="section">
        <h4 class="section-title">Custom Launch Parameters</h4>
        <label>
          Parameters
          <input data-testid="native-settings-params" bind:value={parameters} />
        </label>
        <p data-testid="native-settings-params-hint" class="hint">
          Arguments are optional and appended when launching this game.
        </p>
      </div>

      {#if !windowsHost}
        <label>
          Compatibility tool
          <select data-testid="native-settings-compat" bind:value={selectedCompat}>
            <option value={NONE_COMPAT}>None</option>
            {#each compatTools as tool (tool.path)}
              <option value={tool.path}>{tool.name}</option>
            {/each}
          </select>
        </label>
        {#if compatToolsError}<p class="hint error-hint">{compatToolsError}</p>{/if}
      {/if}

      <!-- dialogs.py:249-251: the prefix row is built ONLY on a non-Windows
           host — a Windows host runs the .exe directly and has no prefix —
           and reads "(will be created at install)" before one exists. -->
      {#if !windowsHost}
        <div class="row">
          <span class="row-label">Wine Prefix (read-only)</span>
          <p data-testid="native-settings-prefix" class="row-value">
            {settings.wineprefix || '(will be created at install)'}
          </p>
        </div>
      {/if}

      {#if saveError}<p data-testid="native-settings-error" class="error" role="alert">{saveError}</p>{/if}

      <button
        data-testid="native-settings-save"
        class="save"
        disabled={saving || settings.candidates.length === 0}
        onclick={handleSave}
      >
        {saving ? 'Saving…' : 'Save'}
      </button>
    {:else if loadError}
      <p data-testid="native-settings-error" class="error" role="alert">{loadError}</p>
    {/if}
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

  .panel {
    position: relative;
    width: min(420px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 12px;
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

  h3 {
    margin: 0;
    padding-right: 28px;
    color: var(--text-h);
    font-size: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text);
  }

  input,
  select {
    font: inherit;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  input:focus-visible,
  select:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text);
    opacity: 0.8;
  }

  .error-hint {
    color: var(--danger);
    opacity: 1;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row-label {
    font-size: 13px;
    color: var(--text);
  }

  .row-value {
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
    word-break: break-all;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }

  .section-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-h);
  }

  .save {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .save:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
