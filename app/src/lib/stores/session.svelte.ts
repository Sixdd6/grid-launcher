import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, SESSION_UNAUTHORIZED_EVENT, type SessionUnauthorized } from '../api';
import { applyRestore, applyUnauthorizedEvent, type ShellSession } from '../shell';

export const session = $state<ShellSession & { error: string | null; busy: boolean }>({
  phase: 'loading', connected: false, serverUrl: '', username: '', lastError: null, error: null, busy: false,
});

function assign(next: ShellSession) {
  session.phase = next.phase; session.connected = next.connected; session.serverUrl = next.serverUrl;
  session.username = next.username; session.lastError = next.lastError; session.rejected = next.rejected;
}

export async function restore() {
  try { assign(applyRestore(await api.restoreSession())); }
  catch { assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null }); }
}

/**
 * Q6: the backend emits `session-unauthorized` once, on the first 401 of a
 * live session. The payload is URL, username and auth kind — never a secret.
 */
export function listenUnauthorized(): Promise<UnlistenFn> {
  return listen<SessionUnauthorized>(SESSION_UNAUTHORIZED_EVENT, (e) => {
    const next = applyUnauthorizedEvent(session.phase, e.payload);
    if (next !== null) {
      session.error = null;
      assign(next);
    }
  });
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true; session.error = null;
  try {
    const state = await api.connect(serverUrl, username, secret, useToken);
    assign({ phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null });
  } catch (e) { session.error = String(e); }
  finally { session.busy = false; }
}

/** The chip's Retry: a typed outcome, so a rejected credential opens Connect. */
export async function retry() {
  session.busy = true;
  try {
    assign(applyRestore(await api.retryConnect()));
  } catch (e) { session.lastError = String(e); }
  finally { session.busy = false; }
}

export async function disconnect() {
  try { await api.disconnect(); } finally {
    assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null });
  }
}
