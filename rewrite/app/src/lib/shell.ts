import type { RestoreOutcome } from './api';

export type Section = 'library' | 'server';
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
export function initialSection(connected: boolean): Section {
  return connected ? 'server' : 'library';
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
