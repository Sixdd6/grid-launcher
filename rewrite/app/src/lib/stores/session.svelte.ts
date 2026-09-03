import { api } from '../api';
import { applyRestore, type ShellSession } from '../shell';

export const session = $state<ShellSession & { error: string | null; busy: boolean }>({
  phase: 'loading', connected: false, serverUrl: '', username: '', lastError: null, error: null, busy: false,
});

function assign(next: ShellSession) {
  session.phase = next.phase; session.connected = next.connected; session.serverUrl = next.serverUrl;
  session.username = next.username; session.lastError = next.lastError;
}

export async function restore() {
  try { assign(applyRestore(await api.restoreSession())); }
  catch { assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null }); }
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true; session.error = null;
  try {
    const state = await api.connect(serverUrl, username, secret, useToken);
    assign({ phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null });
  } catch (e) { session.error = String(e); }
  finally { session.busy = false; }
}

export async function retry() {
  session.busy = true;
  try {
    const state = await api.retryConnect();
    assign({ phase: 'shell', connected: true, serverUrl: state.server_url, username: state.username, lastError: null });
  } catch (e) { session.lastError = String(e); }
  finally { session.busy = false; }
}

export async function disconnect() {
  try { await api.disconnect(); } finally {
    assign({ phase: 'none', connected: false, serverUrl: '', username: '', lastError: null });
  }
}
