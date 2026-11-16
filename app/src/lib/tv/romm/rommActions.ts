// What TV mode's RomM buttons do (TVRommStorefront README § The RomM page,
// § The RomM sign-in page, § Library › RomM › Installed): open the RomM page
// or the sign-in page, Retry / Reconnect with their cards, Disconnect with
// its confirmation, Open in browser over TV mode. Never a desktop-mode
// fallback.
import { api } from '../../api';
import { type SignInMode, ROMM_DISCONNECT_CONFIRM, rommHost } from '../../romm/words';
import { disconnectRomm, reconnectRomm, romm } from '../../stores/romm.svelte';
import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
import { pushPage } from '../nav.svelte';
import { confirmBox } from '../overlays/overlays.svelte';
import { openingCard, reconnectCard } from './rommTv';
import { showRommRow } from './tvRomm.svelte';
import TvRommPage from './TvRommPage.svelte';
import TvRommSignIn from './TvRommSignIn.svelte';

/** Where a sign-in page was opened from: the welcome's RomM row, or anywhere
 *  in the shell (the RomM page, a banner, a needs-RomM panel). */
export type SignInFrom = 'welcome' | 'settings';

function card(title: string, sub: string, error = false, action?: { label: string; run: () => void }): void {
  pushToast(title, error ? 'error' : 'success', TOAST_DURATION_MS, { ...(sub ? { sub } : {}), ...(action ? { action } : {}) });
}

/** Settings › Game stores › RomM. */
export function openRommPage(): void {
  void pushPage({ title: 'RomM', component: TvRommPage, props: {} });
}

/** The RomM sign-in page: Connect, Edit or Sign in again. */
export function openSignIn(mode: SignInMode, from: SignInFrom = 'settings'): void {
  const title = mode === 'edit' ? 'Edit RomM sign-in' : mode === 'again' ? 'Sign in to RomM again' : 'Connect to RomM';
  void pushPage({ title, component: TvRommSignIn, props: { mode, from } });
}

/** The card for a 401 mid-session, with View "Sign in again…". */
export function expiredCard(): void {
  const c = reconnectCard({ ...romm.status, connection: 'expired' });
  card(c.title, c.sub, true, { label: 'Sign in again…', run: () => openSignIn('again') });
}

/** The card for a RomM that stopped answering, with View "Retry". */
export function unreachableCard(): void {
  const c = reconnectCard({ ...romm.status, connection: 'unreachable' });
  card(c.title, c.sub, true, { label: 'Retry', run: () => void retryRomm() });
}

/**
 * Reconnect (the RomM page) and Retry (the Installed banner, the needs-RomM
 * panels): "Reconnecting to RomM…", then one card for the outcome. Does
 * nothing while a probe already runs.
 */
export async function retryRomm(): Promise<void> {
  if (romm.status.busy) return;
  card('Reconnecting to RomM…', rommHost(romm.status));
  await reconnectRomm();
  const c = reconnectCard(romm.status);
  card(c.title, c.sub, c.error, c.signIn ? { label: 'Sign in again…', run: () => openSignIn('again') } : undefined);
}

/** Disconnect…: confirmed, then GRID stays where it is (decision 5). */
export function disconnectBox(): void {
  void confirmBox({
    title: ROMM_DISCONNECT_CONFIRM.title,
    body: ROMM_DISCONNECT_CONFIRM.text,
    ok: ROMM_DISCONNECT_CONFIRM.confirm,
    danger: true,
    onOk: () =>
      void disconnectRomm().then((err) => {
        if (err !== null) card(err, '', true);
        else card('RomM disconnected', 'Installed RomM games stay and still play.');
      }),
  });
}

/** Open in browser: RomM's page opens over TV mode. */
export function openRommInBrowser(): void {
  const c = openingCard(rommHost(romm.status));
  card(c.title, c.sub);
  api.openServerPage().catch((err: unknown) => card(err instanceof Error ? err.message : String(err), '', true));
}

/** Show in Library: pages close, the Library shows RomM › Installed. */
export function showRommInLibrary(): void {
  void showRommRow('romm:installed');
}
