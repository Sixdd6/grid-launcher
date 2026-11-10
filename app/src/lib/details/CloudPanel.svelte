<script lang="ts">
  import { type CloudPanelInfo, type CloudRecord, type InstalledGame, type SaveType } from '../api';
  import Icon from '../Icon.svelte';
  import {
    cloudRecordSummary,
    cloudRecordTitle,
    recordsStatusLine,
    uploadButtonLabel,
    uploadedLine,
  } from './cloud';
  import { nativePathsEmptyLabel, nativePathsStatusLine } from './nativePaths';
  import { CloudStore } from './cloudStore.svelte';
  import { pickFolder } from '../pickers';

  let {
    game,
    gameTitle,
    saveType,
    panelInfo,
    isNative,
  }: {
    game: InstalledGame;
    gameTitle: string;
    saveType: SaveType;
    panelInfo: CloudPanelInfo;
    isNative: boolean;
  } = $props();

  // The panel's data and actions (cloudStore.svelte.ts). The aliases keep
  // the markup reading as before.
  const c = new CloudStore(() => ({ game, gameTitle, saveType, panelInfo, isNative }));

  let nativeSave = $derived(c.nativeSave);
  let nativeStateBlocked = $derived(c.nativeStateBlocked);
  let recordsBlocked = $derived(c.recordsBlocked);
  let panelLabel = $derived(c.panelLabel);
  let sharedNote = $derived(c.sharedNote);
  let kindLabel = $derived(c.kindLabel);
  let records = $derived(c.records);
  let recordsLoading = $derived(c.recordsLoading);
  let recordsError = $derived(c.recordsError);
  let nativePaths = $derived(c.nativePaths);
  let manualPathPending = $derived(c.manualPathPending);
  let manualPathError = $derived(c.manualPathError);
  let uploadPending = $derived(c.uploadPending);
  let uploadMessages = $derived(c.uploadMessages);
  let uploadError = $derived(c.uploadError);
  let actionPendingId = $derived(c.actionPendingId);
  let actionError = $derived(c.actionError);
  let actionNotice = $derived(c.actionNotice);
  let confirmState = $derived(c.confirmState);
  let nativePhase = $derived(c.nativePhase);
  let nativePathCount = $derived(c.nativePathCount);
  let uploadEnabled = $derived(c.uploadEnabled);
  let uploadTooltip = $derived(c.uploadTooltip);
  let confirmText = $derived(c.confirmText);

  $effect(() => {
    c.load();
  });

  function handleUpload() {
    void c.upload();
  }

  function requestRestore(record: CloudRecord) {
    c.requestRestore(record);
  }

  function requestDelete(record: CloudRecord) {
    c.requestDelete(record);
  }

  function confirmAction() {
    void c.confirmAction();
  }

  function handleAddManualPath() {
    void c.addManualPath();
  }

  async function browseManualPath() {
    const picked = await pickFolder('Select Save Folder');
    if (picked === null) return;
    c.manualPathInput = picked;
    await c.addManualPath();
  }

  function handleRemoveSavePath(path: string) {
    void c.removeSavePath(path);
  }
</script>

<div data-testid="cloud-panel" class="cloud-panel">
  <!-- The title, with the upload as a secondary action beside it: a full-
       width primary bar here competed with Play. -->
  <div class="cloud-header">
    <h3>{panelLabel}</h3>
    {#if !nativeStateBlocked}
      <button
        data-testid="cloud-upload"
        class="upload"
        disabled={!uploadEnabled}
        title={uploadTooltip}
        onclick={handleUpload}
      >
        {uploadPending ? 'Uploading…' : uploadButtonLabel(saveType, panelInfo.scope)}
      </button>
    {/if}
  </div>

  {#if sharedNote}
    <p data-testid="cloud-shared-note" class="shared-note">{sharedNote}</p>
  {/if}

  {#if nativeStateBlocked}
    <p data-testid="cloud-native-states-unsupported" class="hint">Save states are not supported for native games.</p>
    <p data-testid="cloud-native-states-note" class="hint">Only save file backups are supported for native games.</p>
  {:else}

    {#if uploadError}<p data-testid="cloud-upload-error" class="error" role="alert">{uploadError}</p>{/if}
    {#each uploadMessages as message, i (i)}
      <p class="message" class:warn={message.severity === 'warning'}>{message.text}</p>
    {/each}

    {#if !nativeSave && !panelInfo.supported}
      <p data-testid="cloud-block-reason" class="hint">{panelInfo.block_reason}</p>
    {/if}

    {#if actionError}<p data-testid="cloud-action-error" class="error" role="alert">{actionError}</p>{/if}
    {#if actionNotice}<p data-testid="cloud-action-notice" class="message">{actionNotice}</p>{/if}

    {#if !recordsBlocked}
      <div class="records">
        {#if recordsLoading}
          <p data-testid="cloud-loading" class="hint">Loading cloud {kindLabel}…</p>
        {:else if recordsError}
          <p data-testid="cloud-records-error" class="error" role="alert">Could not load cloud {kindLabel}: {recordsError}</p>
        {:else if records.length === 0}
          <p data-testid="cloud-empty" class="hint">No cloud {kindLabel} were found for this game yet.</p>
        {:else}
          {#each records as record, i (record.id)}
            <div data-testid={`cloud-record-${record.id}`} class="record">
              <div class="record-info">
                <p class="record-title">
                  {cloudRecordTitle(record, saveType)}
                  {#if i === 0}
                    <!-- Records arrive newest first. -->
                    <span data-testid="cloud-record-latest" class="latest">Latest</span>
                  {/if}
                </p>
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
              bind:value={c.manualPathInput}
              placeholder="/path/to/save/folder"
              disabled={manualPathPending}
            />
            <button
              data-testid="cloud-native-path-add"
              disabled={manualPathPending || !c.manualPathInput.trim()}
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
        <button data-testid="cloud-confirm-no" onclick={() => c.cancelConfirm()}>No</button>
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
    justify-content: space-between;
    gap: 12px;
  }

  .cloud-header h3 {
    margin: 0;
    font-size: 15px;
    color: var(--text-h);
  }

  /* A secondary button (D-UI convention): outlined, `--surface` on hover. */
  button.upload {
    flex: none;
    font: inherit;
    font-size: 13px;
    padding: 6px 14px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  button.upload:hover:not(:disabled) {
    background: var(--surface);
  }

  .shared-note {
    margin: 0;
    padding: 8px 12px;
    border-radius: var(--r-row);
    border: 1px solid var(--warning);
    color: var(--text);
    font-size: 13px;
  }

  .latest {
    margin-left: 6px;
    padding: 1px 7px;
    border-radius: var(--r-pill);
    background: var(--primary);
    color: var(--on-primary);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    vertical-align: 1px;
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
    color: var(--warning-text);
  }

  .native-paths h4 {
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
    border-radius: var(--r-chip);
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
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  .add-path button {
    font: inherit;
    padding: 6px 12px;
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
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
    border-radius: var(--r-row);
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
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .record-actions button:hover:not(:disabled) {
    background: var(--surface);
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
    border-radius: var(--r-chip);
    border: none;
    background: var(--primary);
    color: var(--on-primary);
    cursor: pointer;
  }

  .confirm-actions button:last-child {
    background: transparent;
    border: 1px solid #fff;
  }
</style>
