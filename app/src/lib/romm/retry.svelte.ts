// Reconnect / Retry with the stored credential, shared by the RomM card and
// the needs-RomM panels: one run at a time, with its notification
// ("Reconnected to …" / "Connected to RomM again · …"). A failed run shows
// in the status (Can't reach, Sign-in expired), not as a card.
import { pushToast, TOAST_DURATION_MS } from '../stores/toasts.svelte';
import { reconnectRomm, romm } from '../stores/romm.svelte';
import { reconnectedToast } from './signIn';

export const rommRetry = $state<{ running: boolean }>({ running: false });

export async function retryRomm(): Promise<void> {
  if (rommRetry.running) return;
  rommRetry.running = true;
  const was = romm.status.connection;
  try {
    await reconnectRomm();
  } finally {
    rommRetry.running = false;
  }
  const text = reconnectedToast(was, romm.status);
  if (text !== null) pushToast(text, 'success', TOAST_DURATION_MS);
}
