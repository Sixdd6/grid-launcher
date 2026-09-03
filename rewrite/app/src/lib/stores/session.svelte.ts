import { api, type SessionState } from '../api';

export const session = $state<{ state: SessionState | null; error: string | null; busy: boolean }>({
  state: null,
  error: null,
  busy: false,
});

export async function restore() {
  try {
    const outcome = await api.restoreSession();
    session.state = outcome.kind === 'connected' ? outcome.state : null;
  } catch {
    session.state = null; // silent: no stored session is normal
  }
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true;
  session.error = null;
  try {
    session.state = await api.connect(serverUrl, username, secret, useToken);
  } catch (e) {
    session.error = String(e);
  } finally {
    session.busy = false;
  }
}
