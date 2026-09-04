import type { RestoreOutcome } from './api';

/** The five first-class views, in pill order (design §3). The index in this
 *  array is also the `Ctrl+<n>` accelerator. */
export const VIEWS = ['library', 'server', 'downloads', 'emulators', 'settings'] as const;
export type View = (typeof VIEWS)[number];
export type SessionPhase = 'loading' | 'none' | 'shell';
export type ShellSession = {
  phase: SessionPhase;
  connected: boolean;
  serverUrl: string;
  username: string;
  lastError: string | null;
};

export function applyRestore(outcome: RestoreOutcome): ShellSession {
  switch (outcome.kind) {
    case 'no_session':
      return { phase: 'none', connected: false, serverUrl: '', username: '', lastError: null };
    case 'connected':
      return { phase: 'shell', connected: true, serverUrl: outcome.state.server_url, username: outcome.state.username, lastError: null };
    case 'unreachable':
      return { phase: 'shell', connected: false, serverUrl: outcome.server_url, username: outcome.username, lastError: outcome.error };
  }
}

/** R2: Server first when connected (E2E specs wait for platform-btn-1 after connecting), Library when offline. */
export function initialView(connected: boolean): View {
  return connected ? 'server' : 'library';
}

export function viewLabel(view: View): string {
  return view.charAt(0).toUpperCase() + view.slice(1);
}

/**
 * The view a `Ctrl+<key>` accelerator selects, or `null` when the key is
 * not one of `1`..`5`. Takes the raw `KeyboardEvent.key` so the caller does
 * no parsing of its own; a multi-character key (`"F1"`, `"11"`) never
 * matches because the lookup is by exact string.
 */
export function viewForDigit(key: string): View | null {
  const index = VIEWS.findIndex((_, i) => String(i + 1) === key);
  return index === -1 ? null : VIEWS[index];
}

export function hostOf(serverUrl: string): string {
  try {
    return new URL(serverUrl).host || serverUrl;
  } catch {
    return serverUrl;
  }
}

export function chipLabel(s: ShellSession): string {
  return s.connected ? `${s.username} @ ${hostOf(s.serverUrl)}` : 'Not connected';
}
