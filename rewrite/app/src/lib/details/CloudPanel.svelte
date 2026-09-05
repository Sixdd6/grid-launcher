<script lang="ts">
  import {
    api,
    type CloudMessage,
    type CloudPanelInfo,
    type CloudRecord,
    type InstalledGame,
    type NativeSavePaths,
    type SaveType,
  } from '../api';
  import Icon from '../Icon.svelte';
  import {
    cloudButtonLabel,
    cloudRecordSummary,
    cloudRecordTitle,
    createRequestGuard,
    deleteConfirmText,
    recordsStatusLine,
    restoreConfirmText,
    sharedScopeWarning,
    uploadButtonLabel,
    uploadedLine,
  } from './cloud';
  import {
    nativePathsEmptyLabel,
    nativePathsStatusLine,
    nativeUploadEnabled,
    nativeUploadTooltip,
    type NativePathsPhase,
  } from './nativePaths';
  import { pickFolder } from '../pickers';

  let {
    game,
    gameTitle,
    saveType,
    panelInfo,
    isNative,
    onBack,
  }: {
    game: InstalledGame;
    gameTitle: string;
    saveType: SaveType;
    panelInfo: CloudPanelInfo;
    isNative: boolean;
    onBack: () => void;
  } = $props();

  // Native saves bypass the emulator-resolution block reason entirely
  // (`_refresh_native_save_panel`, details_view_mixin.py:1148-1237): the
  // path-list section below drives its own enablement/messages instead of
  // `panelInfo.supported`/`block_reason`. States are never supported for
  // native games (details_cloud_mode_supported returns false), so this
  // combination is only reachable defensively.
  let nativeSave = $derived(isNative && saveType === 'save');
  let nativeStateBlocked = $derived(isNative && saveType === 'state');

  // `_refresh_details_cloud_panel`'s `compatibility_reason` early return
  // (details_view_mixin.py:1000-1006): when the emulator-resolution gate
  // is unsupported, the panel shows the block reason only — upload stays
  // visible but disabled, and no records fetch is attempted. Native saves
  // never hit this (they bypass `panelInfo` entirely); this is normally
  // unreachable while `supported` also gates the toggle button itself, but
  // guards against a panel left open across a `panelInfo` refresh that
  // flips it to false.
  let recordsBlocked = $derived(nativeStateBlocked || (!nativeSave && !panelInfo.supported));

  let panelLabel = $derived(cloudButtonLabel(saveType, panelInfo.scope));
  let kindLabel = $derived(saveType === 'save' ? 'saves' : 'states');

  let records = $state<CloudRecord[]>([]);
  let recordsLoading = $state(true);
  let recordsError = $state<string | null>(null);

  let nativePaths = $state<NativeSavePaths | null>(null);
  let nativePathsLoading = $state(false);
  let nativePathsAttempted = $state(false);
  let manualPathInput = $state('');
  let manualPathPending = $state(false);
  let manualPathError = $state<string | null>(null);

  let uploadPending = $state(false);
  let uploadMessages = $state<CloudMessage[]>([]);
  let uploadError = $state<string | null>(null);

  let actionPendingId = $state<number | null>(null);
  let actionError = $state<string | null>(null);
  let confirmState = $state<{ kind: 'restore' | 'delete'; record: CloudRecord } | null>(null);

  const recordsGuard = createRequestGuard();
  const nativeGuard = createRequestGuard();

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function loadRecords() {
    if (recordsBlocked) {
      recordsLoading = false;
      return;
    }
    const id = recordsGuard.next();
    recordsLoading = true;
    recordsError = null;
    try {
      const result = await api.cloudRecords(game, saveType);
      if (!recordsGuard.isCurrent(id)) return; // a newer fetch superseded this one
      records = result;
    } catch (err) {
      if (!recordsGuard.isCurrent(id)) return;
      recordsError = errorMessage(err);
    } finally {
      if (recordsGuard.isCurrent(id)) recordsLoading = false;
    }
  }

  async function loadNativePaths() {
    const id = nativeGuard.next();
    nativePathsLoading = true;
    try {
      const result = await api.nativeSavePaths(game);
      if (!nativeGuard.isCurrent(id)) return;
      nativePaths = result;
    } catch (err) {
      if (!nativeGuard.isCurrent(id)) return;
      manualPathError = errorMessage(err);
    } finally {
      if (nativeGuard.isCurrent(id)) {
        nativePathsLoading = false;
        nativePathsAttempted = true;
      }
    }
  }

  $effect(() => {
    void game;
    void saveType;
    loadRecords();
    if (nativeSave && !recordsBlocked) loadNativePaths();
  });

  // The lookup is "loading" until `loadNativePaths()` settles, success or
  // failure: `native_save_paths` can itself error (e.g. `blocking_load_config`
  // failing before the PCGW step ever runs), leaving `nativePaths` null
  // forever, so phase must track the attempt rather than a non-null result.
  let nativePhase = $derived<NativePathsPhase>(
    nativePathsLoading || !nativePathsAttempted ? 'loading' : 'loaded'
  );
  let nativePathCount = $derived((nativePaths?.pcgw.length ?? 0) + (nativePaths?.manual.length ?? 0));
  let nativeUploadIsEnabled = $derived(
    nativeUploadEnabled(nativePhase, nativePathCount, game.rom_id !== null, uploadPending)
  );
  let nativeUploadHint = $derived(
    nativeUploadTooltip(nativePhase, nativePathCount, game.rom_id !== null)
  );

  let uploadEnabled = $derived(
    nativeSave ? nativeUploadIsEnabled : !nativeStateBlocked && panelInfo.supported && !uploadPending
  );
  let uploadTooltip = $derived(
    nativeSave ? nativeUploadHint : panelInfo.supported ? '' : panelInfo.block_reason
  );

  async function handleUpload() {
    uploadPending = true;
    uploadMessages = [];
    uploadError = null;
    try {
      const report = await api.cloudUpload(game, saveType);
      uploadMessages = report.messages;
      if (report.uploaded > 0) await loadRecords();
    } catch (err) {
      uploadError = errorMessage(err);
    } finally {
      uploadPending = false;
    }
  }

  function requestRestore(record: CloudRecord) {
    if (!record.restorable) return;
    confirmState = { kind: 'restore', record };
  }

  function requestDelete(record: CloudRecord) {
    confirmState = { kind: 'delete', record };
  }

  let confirmText = $derived(
    confirmState === null
      ? null
      : confirmState.kind === 'restore'
        ? restoreConfirmText(saveType, gameTitle, confirmState.record.restore_tooltip ?? '')
        : deleteConfirmText(
            saveType,
            cloudRecordTitle(confirmState.record, saveType),
            sharedScopeWarning(panelInfo.scope, confirmState.record.emulator)
          )
  );

  async function confirmAction() {
    if (!confirmState) return;
    const { kind, record } = confirmState;
    confirmState = null;
    actionPendingId = record.id;
    actionError = null;
    try {
      if (kind === 'restore') {
        const report = await api.cloudRestore(game, saveType, String(record.id));
        if (report.ok) {
          await loadRecords();
        } else {
          actionError = report.messages.map((m) => m.text).join(' ') || 'Restore failed.';
        }
      } else {
        await api.cloudDelete(saveType, record.id);
        await loadRecords();
      }
    } catch (err) {
      actionError = errorMessage(err);
    } finally {
      actionPendingId = null;
    }
  }

  async function handleAddManualPath() {
    const path = manualPathInput.trim();
    if (!path) return;
    manualPathPending = true;
    manualPathError = null;
    try {
      await api.nativeAddManualSavePath(game, path);
      manualPathInput = '';
      await loadNativePaths();
      await loadRecords();
    } catch (err) {
      manualPathError = errorMessage(err);
    } finally {
      manualPathPending = false;
    }
  }

  async function browseManualPath() {
    const picked = await pickFolder('Select Save Folder');
    if (picked === null) return;
    manualPathInput = picked;
    await handleAddManualPath();
  }

  // Both lists' rows remove the same way (`_pcgw_remove_path_for_game`,
  // details_view_mixin.py:1218-1230): the backend deletes a manual path and
  // suppresses a PCGW one, so the caller passes only the row's raw value.
  async function handleRemoveSavePath(path: string) {
    manualPathPending = true;
    manualPathError = null;
    try {
      await api.nativeRemoveSavePath(game, path);
      await loadNativePaths();
      await loadRecords();
    } catch (err) {
      manualPathError = errorMessage(err);
    } finally {
      manualPathPending = false;
    }
  }
</script>

<div data-testid="cloud-panel" class="cloud-panel">
  <div class="cloud-header">
    <!-- No `aria-label`: it duplicated the visible word and overrode it, so
         voice control ("click Back") did not match the button. The icon is
         `aria-hidden`, so the visible text is the accessible name. -->
    <button data-testid="cloud-back" class="back" onclick={onBack}>
      <Icon name="arrowLeft" size={16} />
      Back
    </button>
    <h3>{panelLabel}</h3>
  </div>

  {#if nativeStateBlocked}
    <p data-testid="cloud-native-states-unsupported" class="hint">Save states are not supported for native games.</p>
    <p data-testid="cloud-native-states-note" class="hint">Only save file backups are supported for native games.</p>
  {:else}
    <button
      data-testid="cloud-upload"
      class="upload"
      disabled={!uploadEnabled}
      title={uploadTooltip}
      onclick={handleUpload}
    >
      {uploadPending ? 'Uploading…' : uploadButtonLabel(saveType, panelLabel)}
    </button>

    {#if uploadError}<p data-testid="cloud-upload-error" class="error" role="alert">{uploadError}</p>{/if}
    {#each uploadMessages as message, i (i)}
      <p class="message" class:warn={message.severity === 'warning'}>{message.text}</p>
    {/each}

    {#if !nativeSave && !panelInfo.supported}
      <p data-testid="cloud-block-reason" class="hint">{panelInfo.block_reason}</p>
    {/if}

    {#if actionError}<p data-testid="cloud-action-error" class="error" role="alert">{actionError}</p>{/if}

    {#if !recordsBlocked}
      {#if nativeSave}
        <h4 data-testid="cloud-native-saves-label" class="section-label">Cloud Saves</h4>
      {/if}

      <div class="records">
        {#if recordsLoading}
          <p data-testid="cloud-loading" class="hint">Loading cloud {kindLabel}…</p>
        {:else if recordsError}
          <p data-testid="cloud-records-error" class="error" role="alert">Could not load cloud {kindLabel}: {recordsError}</p>
        {:else if records.length === 0}
          <p data-testid="cloud-empty" class="hint">No cloud {kindLabel} were found for this game yet.</p>
        {:else}
          {#each records as record (record.id)}
            <div data-testid={`cloud-record-${record.id}`} class="record">
              <div class="record-info">
                <p class="record-title">{cloudRecordTitle(record, saveType)}</p>
                <p class="record-summary">{cloudRecordSummary(record, saveType)}</p>
                <p class="record-time">{uploadedLine(record)}</p>
              </div>
              <div class="record-actions">
                <button
                  data-testid={`cloud-restore-${record.id}`}
                  disabled={!record.restorable || actionPendingId === record.id}
                  title={record.restore_tooltip ?? ''}
                  onclick={() => requestRestore(record)}
                >
                  Restore
                </button>
                <button
                  data-testid={`cloud-delete-${record.id}`}
                  class="danger"
                  disabled={actionPendingId === record.id}
                  onclick={() => requestDelete(record)}
                >
                  Delete
                </button>
              </div>
            </div>
          {/each}
          <p data-testid="cloud-records-status" class="hint records-status">
            {recordsStatusLine(records.length, saveType)}
          </p>
        {/if}
      </div>

      {#if nativeSave}
        <hr data-testid="cloud-native-separator" class="native-separator" />
        <div class="native-paths">
          <h4>Save Locations</h4>
          <p data-testid="cloud-native-status" class="hint">
            {nativePathsStatusLine(nativePhase, nativePathCount)}
          </p>
          {#if nativePhase === 'loading'}
            <p data-testid="cloud-native-fetching" class="hint">{nativePathsEmptyLabel(nativePhase)}</p>
          {:else if nativePaths}
            <ul class="path-list">
              {#each nativePaths.pcgw as entry (entry.raw)}
                <li data-testid={`cloud-native-path-pcgw-${entry.raw}`} title={entry.expanded}>
                  <span>{entry.raw}</span>
                  <button
                    data-testid={`cloud-native-path-remove-${entry.raw}`}
                    class="remove icon-btn"
                    disabled={manualPathPending}
                    onclick={() => handleRemoveSavePath(entry.raw)}
                    aria-label={`Remove ${entry.raw}`}
                    title="Remove this path"
                  >
                    <Icon name="close" size={14} />
                  </button>
                </li>
              {/each}
              {#each nativePaths.manual as entry (entry.raw)}
                <li data-testid={`cloud-native-path-manual-${entry.raw}`} title={entry.expanded}>
                  <span>{entry.raw}</span>
                  <button
                    data-testid={`cloud-native-path-remove-${entry.raw}`}
                    class="remove icon-btn"
                    disabled={manualPathPending}
                    onclick={() => handleRemoveSavePath(entry.raw)}
                    aria-label={`Remove ${entry.raw}`}
                    title="Remove this path"
                  >
                    <Icon name="close" size={14} />
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
          <div class="add-path">
            <input
              data-testid="cloud-native-path-input"
              bind:value={manualPathInput}
              placeholder="/path/to/save/folder"
              disabled={manualPathPending}
            />
            <button
              data-testid="cloud-native-path-add"
              disabled={manualPathPending || !manualPathInput.trim()}
              onclick={handleAddManualPath}
            >
              Add
            </button>
            <button
              data-testid="cloud-native-path-browse"
              disabled={manualPathPending}
              onclick={browseManualPath}
              title="Add a custom save folder for this game"
            >
              Browse…
            </button>
          </div>
          {#if manualPathError}<p class="error" role="alert">{manualPathError}</p>{/if}
        </div>
      {/if}
    {/if}
  {/if}

  {#if confirmState && confirmText}
    <div data-testid="cloud-confirm" class="confirm-overlay" role="alertdialog" aria-modal="true" aria-label={confirmText.title}>
      <p class="confirm-title">{confirmText.title}</p>
      <p class="confirm-message">{confirmText.message}</p>
      <div class="confirm-actions">
        <button data-testid="cloud-confirm-yes" onclick={confirmAction}>Yes</button>
        <button data-testid="cloud-confirm-no" onclick={() => (confirmState = null)}>No</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .cloud-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    text-align: left;
  }

  .cloud-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .cloud-header h3 {
    margin: 0;
    font-size: 16px;
    color: var(--text-h);
  }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    padding: 4px 8px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  button.upload {
    width: 100%;
    font: inherit;
    padding: 10px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  button.upload:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .hint {
    margin: 0;
    color: var(--text);
    opacity: 0.75;
    font-size: 13px;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .message {
    margin: 0;
    font-size: 13px;
    color: var(--text);
  }

  .message.warn {
    color: var(--warning);
  }

  .native-paths h4,
  .section-label {
    margin: 0 0 4px;
    font-size: 14px;
    font-weight: 700;
    color: var(--text-h);
  }

  .native-separator {
    width: 100%;
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }

  .path-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .path-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    font-size: 13px;
    word-break: break-all;
  }

  /* Was 22×22 with a raw `#e5484d` that did not track the theme. `.icon-btn`
     gives it the 28×28 minimum target and `var(--r-chip)`; the colour is now
     the danger token. */
  .remove {
    flex: none;
    color: var(--danger);
  }

  .add-path {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }

  .add-path input {
    flex: 1;
    font: inherit;
    padding: 6px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .add-path button {
    font: inherit;
    padding: 6px 12px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .records-status {
    margin-top: 0.25rem;
  }

  .records {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .record {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px;
    border-radius: 8px;
    border: 1px solid var(--border);
  }

  .record-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .record-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-h);
    word-break: break-word;
  }

  .record-summary,
  .record-time {
    margin: 0;
    font-size: 12px;
    color: var(--text);
    opacity: 0.75;
  }

  .record-actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: none;
  }

  .record-actions button {
    font: inherit;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .record-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .record-actions button.danger {
    color: var(--danger);
    border-color: var(--danger);
  }

  .confirm-overlay {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    background: rgba(0, 0, 0, 0.6);
    z-index: 30;
    text-align: center;
  }

  .confirm-title {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: #fff;
  }

  .confirm-message {
    margin: 0;
    max-width: 360px;
    white-space: pre-wrap;
    color: #fff;
  }

  .confirm-actions {
    display: flex;
    gap: 10px;
  }

  .confirm-actions button {
    font: inherit;
    padding: 8px 16px;
    border-radius: 6px;
    border: none;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .confirm-actions button:last-child {
    background: transparent;
    border: 1px solid #fff;
  }
</style>
