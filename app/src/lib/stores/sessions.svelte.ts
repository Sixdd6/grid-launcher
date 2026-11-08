import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type GameSession, type SessionsSnapshot } from '../api';

const state = $state<{ sessions: GameSession[] }>({ sessions: [] });

/** Pure lookup (exported for unit tests): the session for `romId`, if any. */
export function findSession(sessions: GameSession[], romId: number): GameSession | undefined {
  return sessions.find((s) => s.rom_id === romId);
}

export const sessions = {
  get list() {
    return state.sessions;
  },
  sessionFor(romId: number): GameSession | undefined {
    return findSession(state.sessions, romId);
  },
};

/** Called with the list before and the snapshot that replaced it. A snapshot
 *  is the whole, authoritative list: it replaces, never merges. Its
 *  one-shot `warning` (a game that exited inside its early-exit window) is
 *  the notifications' to show (notifications/notifier.ts). */
export type SessionsListener = (before: GameSession[], snap: SessionsSnapshot) => void;

const listeners = new Set<SessionsListener>();

export function onSessionsSnapshot(listener: SessionsListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function apply(snap: SessionsSnapshot): void {
  const before = state.sessions;
  state.sessions = snap.sessions;
  for (const listener of listeners) listener(before, snap);
}

export async function init(): Promise<UnlistenFn> {
  const snapshot = await api.listSessions();
  apply(snapshot);
  return listen<SessionsSnapshot>('sessions-changed', (e) => {
    apply(e.payload);
  });
}
