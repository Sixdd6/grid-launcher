// The app-lifetime wiring for the download, install and launch notification
// cards. Registered once per shell lifetime by `lifetime.svelte.ts`
// (whichever shell is mounted), so every event shows one card whichever
// view started it or is open. The wording and batching live in
// `notices.ts`. Nothing here navigates: a card's action that leads
// somewhere goes through `stores/navigation.svelte.ts`, so it works in
// either shell.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  EMULATOR_INSTALLED_EVENT,
  type EmulatorInstalledEvent,
  type OsChangeOutcome,
  type PackOs,
} from '../api';
import { BackupBatches, osChangeHint } from '../backups/notices';
import { toLaunchFailure } from '../launchError';
import { markCurrent } from '../stores/emulatorUpdates.svelte';
import { onSnapshot } from '../stores/downloads.svelte';
import { onSessionsSnapshot } from '../stores/sessions.svelte';
import { setupRun } from '../stores/setupRun.svelte';
import { pushToast } from '../stores/toasts.svelte';
import {
  Coalescer,
  diffSnapshot,
  earlyExitNotice,
  emulatorInstalledNotice,
  launchFailedNotice,
  launchingNotice,
  noticesFor,
  queueRefusedNotice,
  type Hint,
  type Notice,
  type StatusBook,
} from './notices';

const coalescer = new Coalescer((text, level) => pushToast(text, level));

/** Shows `notice`, merged with any of its group arriving at the same time. */
export function notify(notice: Notice): void {
  coalescer.add(notice);
}

/** How long a registered hint waits for its queue entry to appear. */
const HINT_TTL_MS = 60_000;

const hints = new Map<string, { hint: Hint; at: number }>();

/**
 * Registers how the queue entry for `sourceId` (an emulator or compat tool's
 * catalog source) should be worded, before the command that queues it runs:
 * the entry can reach the frontend before the command returns.
 */
export function expectQueued(sourceId: string, hint: Hint): void {
  if (sourceId.trim() === '') return;
  hints.set(sourceId, { hint, at: Date.now() });
}

function hintFor(sourceId: string): Hint | undefined {
  const held = hints.get(sourceId);
  if (!held) return undefined;
  if (Date.now() - held.at > HINT_TTL_MS) {
    hints.delete(sourceId);
    return undefined;
  }
  return held.hint;
}

function dropHint(sourceId: string): void {
  hints.delete(sourceId);
}

// One card per backup batch (backups/notices.ts), app-wide like the rest.
const backups = new BackupBatches();
let backupTimer: ReturnType<typeof setTimeout> | null = null;

/** Shows the queued cards of batches that stopped growing, and re-arms for the next. */
function pumpBackups(): void {
  if (backupTimer !== null) clearTimeout(backupTimer);
  backupTimer = null;
  for (const n of backups.due(Date.now())) notify(n);
  const wait = backups.nextDueIn(Date.now());
  if (wait !== null) backupTimer = setTimeout(pumpBackups, wait);
}

/**
 * Words the backup batch `set_drive_os` queued as an OS change: its queued
 * card reads "Windows builds of 2 emulators added to the backup queue for
 * Couch kit" (and its done card "Windows builds of 2 emulators backed up to
 * Couch kit") instead of the generic batch wording.
 *
 * Settings › Backup drives calls it with the command's answer and the OSes
 * the user ticked that the drive did not have yet, right after
 * `api.setDriveOs` resolves. Does nothing when `outcome.batch` is 0 (nothing
 * was queued). The batch's queued card waits `BATCH_SETTLE_MS` after its
 * last row arrives, so a call made as soon as the command resolves is in
 * time; a later call still words the done card.
 */
export function expectOsChangeBatch(outcome: OsChangeOutcome, added: readonly PackOs[]): void {
  const hint = osChangeHint(outcome, added);
  if (hint) backups.hint(outcome.batch, hint);
}

/**
 * Launches `romId`, announcing it, and reports a refusal as an error card.
 * Rethrows the failure (as a `LaunchFailure`) so the caller can still offer
 * the umu-launcher install beside the button.
 */
export async function launchGame(romId: number, title: string): Promise<void> {
  notify(launchingNotice(title));
  try {
    await api.launchGame(romId);
  } catch (err) {
    const failure = toLaunchFailure(err);
    notify(launchFailedNotice(title, failure.message));
    throw failure;
  }
}

/**
 * Queues `romId`'s install. The card that it was queued comes from the
 * downloads snapshot; only a refusal (nothing queued) is reported here.
 * Rethrows so the caller can react to the reason (an unset library folder).
 */
export async function installGame(romId: number, title: string): Promise<void> {
  try {
    await api.installGame(romId);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    notify(queueRefusedNotice(title, message));
    throw err;
  }
}

export function initNotifications(): () => void {
  let book: StatusBook | null = null;
  const offDownloads = onSnapshot((entries, first) => {
    for (const n of backups.observe(entries, first, Date.now())) notify(n);
    pumpBackups();
    const { book: next, transitions } = diffSnapshot(first ? null : book, entries);
    book = next;
    for (const t of transitions) {
      const sourceId = t.entry.source_id;
      const hint = sourceId === '' ? undefined : hintFor(sourceId);
      for (const n of noticesFor(t, { setupActive: setupRun.active, hint })) notify(n);
      if (sourceId !== '' && (t.entry.status === 'completed' || t.entry.status === 'failed' || t.entry.status === 'cancelled')) {
        dropHint(sourceId);
      }
    }
  });
  const offSessions = onSessionsSnapshot((before, snap) => {
    if (snap.warning !== null && snap.warning.trim() !== '') {
      notify(earlyExitNotice(before, snap.sessions, snap.warning));
    }
  });
  const pending: Promise<UnlistenFn>[] = [
    // The completion card for an emulator install OR update, worded by the
    // backend's `fresh` flag. The setup card counts its own installs.
    listen<EmulatorInstalledEvent>(EMULATOR_INSTALLED_EVENT, (event) => {
      if (!setupRun.active) notify(emulatorInstalledNotice(event.payload.name, event.payload.fresh));
      // A fresh install or an update both put the newest build on disk.
      markCurrent(event.payload.name);
    }),
  ];
  return () => {
    offDownloads();
    if (backupTimer !== null) clearTimeout(backupTimer);
    backupTimer = null;
    offSessions();
    coalescer.flush();
    for (const p of pending) void p.then((off) => off());
  };
}
