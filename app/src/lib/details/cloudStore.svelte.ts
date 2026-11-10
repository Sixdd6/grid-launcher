// One cloud saves (or states) panel for a game: the records list, Upload
// latest, restore and delete with their confirm step, and the native PC
// save-folder list. The desktop CloudPanel and the TV Saves page both read
// it. One `CloudStore` per panel, created with a getter for its inputs.
//
// Every request is guarded (`createRequestGuard`): an answer that a newer
// request superseded is dropped. Wording comes from `cloud.ts` and
// `nativePaths.ts`.
import {
  api,
  type CloudMessage,
  type CloudPanelInfo,
  type CloudRecord,
  type InstalledGame,
  type NativeSavePaths,
  type SaveType,
} from '../api';
import {
  cloudRecordTitle,
  cloudSectionTitle,
  createRequestGuard,
  deleteConfirmText,
  restoreAllConfirmText,
  restoreConfirmText,
  sharedScopeWarning,
} from './cloud';
import { nativeUploadEnabled, nativeUploadTooltip, type NativePathsPhase } from './nativePaths';

export type CloudInput = {
  game: InstalledGame;
  gameTitle: string;
  saveType: SaveType;
  panelInfo: CloudPanelInfo;
  isNative: boolean;
};

export type CloudConfirm = {
  kind: 'restore' | 'restore-all' | 'delete';
  record: CloudRecord;
  folders?: string[];
};

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export class CloudStore {
  readonly #input: () => CloudInput;

  constructor(input: () => CloudInput) {
    this.#input = input;
  }

  get input(): CloudInput {
    return this.#input();
  }

  // Native saves bypass the emulator-resolution block reason entirely: the
  // path-list section drives its own enablement and messages. States are
  // never supported for native games, so that pairing is only defensive.
  nativeSave = $derived(this.input.isNative && this.input.saveType === 'save');
  nativeStateBlocked = $derived(this.input.isNative && this.input.saveType === 'state');
  // An unsupported emulator-resolution gate shows the block reason only:
  // upload stays visible but disabled, and no records fetch is attempted.
  recordsBlocked = $derived(this.nativeStateBlocked || (!this.nativeSave && !this.input.panelInfo.supported));

  panelLabel = $derived(cloudSectionTitle(this.input.saveType, this.input.panelInfo.scope));
  /** Shared emulator media: restoring or deleting one entry affects every
   *  game on that emulator, so the list says so up front. */
  sharedNote = $derived(this.input.saveType === 'save' ? sharedScopeWarning(this.input.panelInfo.scope, '') : '');
  kindLabel = $derived(this.input.saveType === 'save' ? 'saves' : 'states');

  records = $state<CloudRecord[]>([]);
  recordsLoading = $state(true);
  recordsError = $state<string | null>(null);

  nativePaths = $state<NativeSavePaths | null>(null);
  nativePathsLoading = $state(false);
  nativePathsAttempted = $state(false);
  manualPathInput = $state('');
  manualPathPending = $state(false);
  manualPathError = $state<string | null>(null);

  uploadPending = $state(false);
  uploadMessages = $state<CloudMessage[]>([]);
  uploadError = $state<string | null>(null);

  actionPendingId = $state<number | null>(null);
  actionError = $state<string | null>(null);
  // G1-Q: a restore that skipped save folders not matched to the game says
  // so here, and the panel offers a confirmed "restore all" (`folders`).
  actionNotice = $state<string | null>(null);
  confirmState = $state<CloudConfirm | null>(null);

  readonly #recordsGuard = createRequestGuard();
  readonly #nativeGuard = createRequestGuard();

  // The lookup is "loading" until `loadNativePaths()` settles, success or
  // failure: the lookup can itself error, leaving `nativePaths` null for
  // good, so the phase tracks the attempt rather than a non-null result.
  nativePhase = $derived<NativePathsPhase>(
    this.nativePathsLoading || !this.nativePathsAttempted ? 'loading' : 'loaded',
  );
  nativePathCount = $derived((this.nativePaths?.pcgw.length ?? 0) + (this.nativePaths?.manual.length ?? 0));
  nativeUploadIsEnabled = $derived(
    nativeUploadEnabled(this.nativePhase, this.nativePathCount, this.input.game.rom_id !== null, this.uploadPending),
  );
  nativeUploadHint = $derived(nativeUploadTooltip(this.nativePhase, this.nativePathCount, this.input.game.rom_id !== null));

  uploadEnabled = $derived(
    this.nativeSave
      ? this.nativeUploadIsEnabled
      : !this.nativeStateBlocked && this.input.panelInfo.supported && !this.uploadPending,
  );
  uploadTooltip = $derived(
    this.nativeSave ? this.nativeUploadHint : this.input.panelInfo.supported ? '' : this.input.panelInfo.block_reason,
  );

  confirmText = $derived.by(() => {
    const c = this.confirmState;
    if (c === null) return null;
    const { saveType, gameTitle, panelInfo } = this.input;
    if (c.kind === 'restore') return restoreConfirmText(saveType, gameTitle, c.record.restore_tooltip ?? '');
    if (c.kind === 'restore-all') return restoreAllConfirmText(gameTitle, c.folders ?? []);
    return deleteConfirmText(
      saveType,
      cloudRecordTitle(c.record, saveType),
      sharedScopeWarning(panelInfo.scope, c.record.emulator),
    );
  });

  /** Loads the records (and, for native saves, the save folders). Reads the
   *  game and kind first, so an `$effect` calling it re-runs on either. */
  load(): void {
    void this.input.game;
    void this.input.saveType;
    void this.loadRecords();
    if (this.nativeSave && !this.recordsBlocked) void this.loadNativePaths();
  }

  async loadRecords(): Promise<void> {
    if (this.recordsBlocked) {
      this.recordsLoading = false;
      return;
    }
    const { game, saveType } = this.input;
    const id = this.#recordsGuard.next();
    this.recordsLoading = true;
    this.recordsError = null;
    try {
      const result = await api.cloudRecords(game, saveType);
      if (!this.#recordsGuard.isCurrent(id)) return; // a newer fetch superseded this one
      this.records = result;
    } catch (err) {
      if (!this.#recordsGuard.isCurrent(id)) return;
      this.recordsError = errorMessage(err);
    } finally {
      if (this.#recordsGuard.isCurrent(id)) this.recordsLoading = false;
    }
  }

  async loadNativePaths(): Promise<void> {
    const game = this.input.game;
    const id = this.#nativeGuard.next();
    this.nativePathsLoading = true;
    try {
      const result = await api.nativeSavePaths(game);
      if (!this.#nativeGuard.isCurrent(id)) return;
      this.nativePaths = result;
    } catch (err) {
      if (!this.#nativeGuard.isCurrent(id)) return;
      this.manualPathError = errorMessage(err);
    } finally {
      if (this.#nativeGuard.isCurrent(id)) {
        this.nativePathsLoading = false;
        this.nativePathsAttempted = true;
      }
    }
  }

  /** Upload latest: the report's lines show; the list reloads when anything went up. */
  async upload(): Promise<void> {
    const { game, saveType } = this.input;
    this.uploadPending = true;
    this.uploadMessages = [];
    this.uploadError = null;
    try {
      const report = await api.cloudUpload(game, saveType);
      this.uploadMessages = report.messages;
      if (report.uploaded > 0) await this.loadRecords();
    } catch (err) {
      this.uploadError = errorMessage(err);
    } finally {
      this.uploadPending = false;
    }
  }

  /** Asks to restore `record`; a record that cannot be restored is ignored. */
  requestRestore(record: CloudRecord): void {
    if (!record.restorable) return;
    this.confirmState = { kind: 'restore', record };
  }

  requestDelete(record: CloudRecord): void {
    this.confirmState = { kind: 'delete', record };
  }

  cancelConfirm(): void {
    this.confirmState = null;
  }

  /** Runs the confirmed restore, restore-all or delete. A restore that
   *  skipped folders then offers restore-all. */
  async confirmAction(): Promise<void> {
    if (!this.confirmState) return;
    const { kind, record } = this.confirmState;
    const { game, saveType } = this.input;
    this.confirmState = null;
    this.actionPendingId = record.id;
    this.actionError = null;
    this.actionNotice = null;
    try {
      if (kind === 'restore' || kind === 'restore-all') {
        const restoreAll = kind === 'restore-all';
        const report = await api.cloudRestore(game, saveType, String(record.id), restoreAll);
        const text = report.messages.map((m) => m.text).join(' ');
        if (report.ok) {
          if (report.skipped_folders.length > 0) this.actionNotice = text;
          await this.loadRecords();
        } else {
          this.actionError = text || 'Restore failed.';
        }
        // G1-Q: offer every folder only after the user saw which were skipped.
        if (!restoreAll && report.skipped_folders.length > 0) {
          this.confirmState = { kind: 'restore-all', record, folders: report.skipped_folders };
        }
      } else {
        await api.cloudDelete(saveType, record.id);
        await this.loadRecords();
      }
    } catch (err) {
      this.actionError = errorMessage(err);
    } finally {
      this.actionPendingId = null;
    }
  }

  /** Adds the typed save folder (`manualPathInput`), then reloads both lists. */
  async addManualPath(): Promise<void> {
    const path = this.manualPathInput.trim();
    if (!path) return;
    this.manualPathPending = true;
    this.manualPathError = null;
    try {
      await api.nativeAddManualSavePath(this.input.game, path);
      this.manualPathInput = '';
      await this.loadNativePaths();
      await this.loadRecords();
    } catch (err) {
      this.manualPathError = errorMessage(err);
    } finally {
      this.manualPathPending = false;
    }
  }

  /** Removes a save folder row: the backend deletes a manual path and
   *  suppresses a PCGW one, so both pass only the row's raw value. */
  async removeSavePath(path: string): Promise<void> {
    this.manualPathPending = true;
    this.manualPathError = null;
    try {
      await api.nativeRemoveSavePath(this.input.game, path);
      await this.loadNativePaths();
      await this.loadRecords();
    } catch (err) {
      this.manualPathError = errorMessage(err);
    } finally {
      this.manualPathPending = false;
    }
  }
}
