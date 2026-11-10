// Settings › Library › Change… (Q2, 9b) as a flow both UIs walk: the new
// folder checked as it is typed, then Move existing files (preview, start)
// or Start fresh (keep or delete the old files, with the delete preview).
// Steps: choose → move-confirm, or choose → fresh-ask → delete-confirm.
// With no current library folder Start fresh switches straight away.
//
// One `LibraryChange` per open dialog or page. Focus, the step's layout and
// the Tab trap stay with the view; `meta` is the step's wording.
import {
  api,
  type LibraryMoveStartOutcome,
  type LibraryPathCheck,
  type MovePreview,
  type StartFreshOutcome,
  type StartFreshPreview,
} from '../api';
import { canMove, canStartFresh, type ChangeStep } from './library';

/** How the move-confirm summary reads from the preview alone. */
export type MoveTone = 'ok' | 'warn' | 'bad';

export type StepMeta = { n: number; eyebrow: string; title: string; desc: string };

/** What the header says per step: the eyebrow, the title (the dialog's
 *  accessible name) and the description. */
export const STEP_META: Record<ChangeStep, StepMeta> = {
  choose: {
    n: 1,
    eyebrow: 'Step 1 · Choose folder',
    title: 'Change library folder',
    desc: 'Choose the folder for your library. Type its full path or pick it with Browse….',
  },
  'fresh-ask': {
    n: 2,
    eyebrow: 'Step 2 · Old games',
    title: 'Delete the old game files, or leave them on disk?',
    desc: 'Either way, the games in the old folder leave your library. Emulators and saves stay where they are.',
  },
  'delete-confirm': {
    n: 3,
    eyebrow: 'Step 3 · Final check',
    title: 'Delete these game files?',
    desc: 'This cannot be undone. Only the folders listed below are removed.',
  },
  'move-confirm': {
    n: 2,
    eyebrow: 'Step 2 · Move',
    title: 'Move your library to the new folder?',
    desc: 'Games, emulators and saves move in the background. You can follow the move in Downloads.',
  },
};

/**
 * Presentation only; the backend still refuses a move that does not fit. A
 * same-drive move copies nothing, so space is no concern. A copy needs
 * `required_bytes` (the copied bytes plus a safety margin): below that the
 * drive is short, within a quarter above it the space is tight, and an
 * unreadable figure is flagged rather than assumed to be fine.
 */
export function moveTone(p: MovePreview | null): MoveTone {
  if (p === null || p.copy_bytes === 0) return 'ok';
  if (p.free_bytes === null) return 'warn';
  if (p.free_bytes < p.required_bytes) return 'bad';
  if (p.free_bytes < p.required_bytes * 1.25) return 'warn';
  return 'ok';
}

/** The line under the move preview; `''` before it arrives or when it is empty. */
export function moveHint(p: MovePreview | null): string {
  if (p === null || p.items.length === 0) return '';
  if (p.copy_bytes === 0) return 'Each folder is renamed. Nothing is copied.';
  const tone = moveTone(p);
  if (tone === 'bad') {
    return 'The new drive does not have enough free space for this move. Free up space or choose another folder.';
  }
  if (p.free_bytes === null) return 'GRID could not read the free space on the new drive.';
  if (tone === 'warn') return 'Space is tight: the new drive has little room to spare.';
  return 'Files are copied and checked first. The old copies are removed afterwards.';
}

/** How long the typed folder settles before the backend checks it. */
export const CHECK_DELAY_MS = 250;

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export type LibraryChangeEvents = {
  /** A Start fresh call returned; `deleted` says which answer was given. */
  onDone: (outcome: StartFreshOutcome, deleted: boolean) => void;
  /** The move started in the background. */
  onMoveStarted: (outcome: LibraryMoveStartOutcome) => void;
  onClose: () => void;
};

export class LibraryChange {
  readonly #currentPath: () => string;
  readonly #events: LibraryChangeEvents;

  /** `currentPath` is the stored library path, verbatim: it is sent back as
   *  the expected old root. */
  constructor(currentPath: () => string, events: LibraryChangeEvents) {
    this.#currentPath = currentPath;
    this.#events = events;
  }

  get currentPath(): string {
    return this.#currentPath();
  }

  step = $state<ChangeStep>('choose');
  typed = $state('');
  check = $state<LibraryPathCheck | null>(null);
  checking = $state(false);
  preview = $state<StartFreshPreview | null>(null);
  movePreview = $state<MovePreview | null>(null);
  pending = $state(false);
  error = $state<string | null>(null);

  ready = $derived(canStartFresh(this.check, this.typed) && !this.checking);
  moveReady = $derived(canMove(this.check, this.typed, this.currentPath) && !this.checking);
  hasOldRoot = $derived(this.currentPath.trim() !== '');
  moveTone = $derived(moveTone(this.movePreview));
  moveHint = $derived(moveHint(this.movePreview));
  meta = $derived(STEP_META[this.step]);

  /**
   * Asks the backend about the typed folder, `CHECK_DELAY_MS` after the last
   * change. Reads `typed` first, so an `$effect` calling it re-runs on every
   * change; answers the function that drops a stale answer.
   */
  checkTyped(): () => void {
    const value = this.typed.trim();
    this.check = null;
    if (value === '') {
      this.checking = false;
      return () => {};
    }
    this.checking = true;
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .checkLibraryPath(value)
        .then((result) => {
          if (!cancelled) this.check = result;
        })
        .catch((err) => {
          if (!cancelled) this.error = errorMessage(err);
        })
        .finally(() => {
          if (!cancelled) this.checking = false;
        });
    }, CHECK_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }

  /** Start fresh, keeping (`false`) or deleting (`true`) the old files. */
  async run(deleteFiles: boolean): Promise<void> {
    const check = this.check;
    if (this.pending || check === null || check.status !== 'ok') return;
    this.pending = true;
    this.error = null;
    try {
      const outcome = deleteFiles
        ? await api.libraryStartFreshDelete(check.path, this.currentPath)
        : await api.libraryStartFreshKeep(check.path, this.currentPath);
      this.#events.onDone(outcome, deleteFiles);
    } catch (err) {
      this.error = errorMessage(err);
    } finally {
      this.pending = false;
    }
  }

  /** Start fresh: asks about the old files, or switches at once when there are none. */
  startFresh(): void {
    if (!this.ready) return;
    this.error = null;
    if (this.hasOldRoot) this.step = 'fresh-ask';
    else void this.run(false);
  }

  /** "Delete": the final check, with the folders it removes and their size. */
  async askDelete(): Promise<void> {
    if (this.pending) return;
    this.step = 'delete-confirm';
    this.preview = null;
    this.error = null;
    try {
      this.preview = await api.libraryStartFreshPreview();
    } catch (err) {
      this.error = errorMessage(err);
    }
  }

  /** Move existing files (9b): the preview first. */
  async askMove(): Promise<void> {
    const check = this.check;
    if (this.pending || !this.moveReady || check === null || check.status !== 'ok') return;
    this.step = 'move-confirm';
    this.movePreview = null;
    this.error = null;
    try {
      const outcome = await api.libraryMovePreview(check.path);
      if (outcome.status === 'ok') this.movePreview = outcome.preview;
      else this.error = outcome.message;
    } catch (err) {
      this.error = errorMessage(err);
    }
  }

  /** Start move: a background job with its own Downloads row. */
  async startMove(): Promise<void> {
    const check = this.check;
    if (this.pending || this.movePreview === null || check === null || check.status !== 'ok') return;
    this.pending = true;
    this.error = null;
    try {
      const outcome = await api.libraryMoveStart(check.path);
      if (outcome.status === 'started') this.#events.onMoveStarted(outcome);
      else this.error = outcome.message;
    } catch (err) {
      this.error = errorMessage(err);
    } finally {
      this.pending = false;
    }
  }

  /** One step back; from the first step, closes. Ignored while a call runs. */
  back(): void {
    if (this.pending) return;
    this.error = null;
    if (this.step === 'delete-confirm') this.step = 'fresh-ask';
    else if (this.step === 'fresh-ask' || this.step === 'move-confirm') this.step = 'choose';
    else this.#events.onClose();
  }

  /** Closes, unless a call is running. */
  cancel(): void {
    if (!this.pending) this.#events.onClose();
  }
}
