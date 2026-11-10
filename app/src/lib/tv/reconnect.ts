// Menu › Reconnect (checklist item 364; the wording of tvmode.js `reconnect`,
// item 208): "Reconnecting to <URL>…", then "Connected to <URL>". Used
// without a server there is nothing to reconnect to, so it opens Connect, as
// the desktop's "Connect to a server…" does. A rejected credential opens
// Connect with the reason (stores/session.svelte.ts `retry`).
import { connectToServer, retry, session } from '../stores/session.svelte';
import { pushToast } from '../stores/toasts.svelte';

export async function reconnect(): Promise<void> {
  if (session.offline && !session.connected) {
    connectToServer();
    return;
  }
  const url = session.serverUrl;
  pushToast(`Reconnecting to ${url}…`);
  await retry();
  if (session.connected) pushToast(`Connected to ${url}`);
  else if (session.phase === 'shell') {
    // TODO(design): no source words a failed Reconnect; the prototype's
    // offline card ("Can't reach <URL>") with the reason under it is used.
    pushToast(`Can't reach ${url}`, 'error', undefined, { sub: session.lastError ?? undefined });
  }
}
