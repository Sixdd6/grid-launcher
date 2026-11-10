// Settings › RetroAchievements' commands, for the desktop page and the TV
// pane alike. The username, token and password stay in the caller's own
// field state: they are passed straight through to the commands and never
// kept, logged or returned here (the backend never returns the token).
import { api, type RaStatus } from '../api';
import {
  canLogin,
  CREDENTIALS_CLEARED_TOAST,
  fanOutSummary,
  LOGIN_MISSING_FIELDS_TOAST,
  loginFailedToast,
  loginToast,
  rommSyncNotice,
} from '../emulators/retroachievements';
import { pushToast } from '../stores/toasts.svelte';

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** The saved account: the username and whether a token is present. */
export function fetchRaStatus(): Promise<RaStatus> {
  return api.getRetroachievementsStatus();
}

// U6: a saved username also goes to the RomM account (username only, never
// the token). Fire and forget: the local save already succeeded, and a
// RomM failure is a short notice, never an error on the form.
export function syncUsernameToRomm(): void {
  api
    .syncRaUsernameToRomm()
    .then((outcome) => {
      const notice = rommSyncNotice(outcome);
      if (notice) pushToast(notice, 'error');
    })
    .catch((err) => {
      const notice = rommSyncNotice({ kind: 'failed', error: errorMessage(err) });
      if (notice) pushToast(notice, 'error');
    });
}

/** Saves a username and API token to every emulator that takes them.
 *  Answers the per-emulator summary line; throws the refusal. */
export async function saveRaToken(username: string, token: string): Promise<string> {
  const rows = await api.setRetroachievementsCredentials(username, token);
  syncUsernameToRomm();
  return fanOutSummary(rows);
}

export type RaLoginResult =
  /** Logged in: the per-emulator summary line. */
  | { kind: 'ok'; line: string }
  /** A field is blank; a card said so and nothing was sent. */
  | { kind: 'missing' }
  | { kind: 'failed'; error: string };

/** Logs in with a username and password, which the backend trades for a
 *  token. Its card and a failure's card are sent here. */
export async function raLogin(username: string, password: string): Promise<RaLoginResult> {
  if (!canLogin(username, password)) {
    pushToast(LOGIN_MISSING_FIELDS_TOAST, 'error');
    return { kind: 'missing' };
  }
  try {
    const result = await api.retroachievementsLogin(username, password);
    pushToast(loginToast(result.username));
    syncUsernameToRomm();
    return { kind: 'ok', line: fanOutSummary(result.fan_out) };
  } catch (err) {
    const error = errorMessage(err);
    pushToast(loginFailedToast(error), 'error');
    return { kind: 'failed', error };
  }
}

/** Clears the saved credentials everywhere, with its card. Throws the refusal. */
export async function clearRaCredentials(): Promise<void> {
  await api.clearRetroachievementsCredentials();
  pushToast(CREDENTIALS_CLEARED_TOAST);
}
