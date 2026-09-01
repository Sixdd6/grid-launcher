import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type GameSession, type SessionsSnapshot } from '../api';

const state = $state<{ sessions: GameSession[]; lastWarning: string | null }>({
  sessions: [],
  lastWarning: null,
});

/**
 * Pure snapshot-application logic (exported for unit tests). A snapshot
 * always REPLACES the current session list — it never merges — because the
 * backend snapshot is already the full, authoritative list.
 *
 * `lastWarning` is sticky: it is overwritten only when the incoming snapshot
 * itself carries a warning. A `null`-warning snapshot never clears a
 * previously captured warning, because back-to-back snapshots happen when
 * several games exit in the same tick, and a later warning-less snapshot
 * must not erase a warning a just-prior snapshot delivered. Use
 * `dismissWarning()` to clear it explicitly.
 */
export function applySnapshot(
  current: { sessions: GameSession[]; lastWarning: string | null },
  snap: SessionsSnapshot,
): { sessions: GameSession[]; lastWarning: string | null } {
  return {
    sessions: snap.sessions,
    lastWarning: snap.warning !== null ? snap.warning : current.lastWarning,
  };
}

/** Pure lookup (exported for unit tests): the session for `romId`, if any. */
export function findSession(sessions: GameSession[], romId: number): GameSession | undefined {
  return sessions.find((s) => s.rom_id === romId);
}

export const sessions = {
  get list() {
    return state.sessions;
  },
  get lastWarning() {
    return state.lastWarning;
  },
  sessionFor(romId: number): GameSession | undefined {
    return findSession(state.sessions, romId);
  },
  dismissWarning(): void {
    state.lastWarning = null;
  },
};

function apply(snap: SessionsSnapshot): void {
  const next = applySnapshot(state, snap);
  state.sessions = next.sessions;
  state.lastWarning = next.lastWarning;
}

export async function init(): Promise<UnlistenFn> {
  const snapshot = await api.listSessions();
  apply(snapshot);
  return listen<SessionsSnapshot>('sessions-changed', (e) => {
    apply(e.payload);
  });
}
