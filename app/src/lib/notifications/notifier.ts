// The app-lifetime wiring for the download, install and launch notification
// cards. Registered once by Shell.svelte (mounted exactly once, never
// hidden), so every event shows one card whichever view started it or is
// open. The wording and batching live in `notices.ts`.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, EMULATOR_INSTALLED_EVENT, type EmulatorInstalledEvent } from '../api';
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
    offSessions();
    coalescer.flush();
    for (const p of pending) void p.then((off) => off());
  };
}
