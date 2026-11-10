// Menu › Reconnect (checklist item 364; the wording of tvmode.js `reconnect`,
// item 208): "Reconnecting to <URL>…", then "Connected to <URL>". Used
// without a server there is nothing to reconnect to, so it opens Connect, as
// the desktop's "Connect to a server…" does. A rejected credential opens
// Connect with the reason (stores/session.svelte.ts `retry`), with no card.
// A retry that leaves the shell not connected shows "Can't reach <URL>" with
// the backend's reason as its second line (README § Consultant decisions).
// While a probe is already running, Reconnect does nothing.
import { connectToServer, retry, session } from '../stores/session.svelte';
import { pushToast } from '../stores/toasts.svelte';

export async function reconnect(): Promise<void> {
  if (session.offline && !session.connected) {
    connectToServer();
    return;
  }
  if (session.busy) return;
  const url = session.serverUrl;
  pushToast(`Reconnecting to ${url}…`);
  await retry();
  if (session.connected) pushToast(`Connected to ${url}`);
  else if (session.phase === 'shell') pushToast(`Can't reach ${url}`, 'error', undefined, { sub: session.lastError ?? undefined });
}
