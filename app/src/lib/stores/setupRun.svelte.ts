import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { SETUP_DEFAULTS_EVENT, type SetupOutcome, type SetupStarted } from '../api';

// The first-run setup that was started this session: what it queued and what
// has happened to each platform's default since. `SetupProgress.svelte` shows
// it in the shell until dismissed. Nothing here persists: after a restart the
// pending defaults still apply (they live in config.toml), only the card is
// gone.
const state = $state<{
  active: boolean;
  queued: string[];
  queuedServer: number[];
  errors: string[];
  outcomes: SetupOutcome[];
}>({ active: false, queued: [], queuedServer: [], errors: [], outcomes: [] });

export const setupRun = {
  get active() {
    return state.active;
  },
  get queued() {
    return state.queued;
  },
  get queuedServer() {
    return state.queuedServer;
  },
  get errors() {
    return state.errors;
  },
  get outcomes() {
    return state.outcomes;
  },
};

/** Called with `startPlatformSetup`'s answer as the picker hands over. */
export function startSetupRun(started: SetupStarted): void {
  state.queued = started.queued;
  state.queuedServer = started.queued_server;
  state.errors = started.errors;
  state.outcomes = started.applied;
  state.active =
    started.queued.length > 0 ||
    started.queued_server.length > 0 ||
    started.errors.length > 0 ||
    started.applied.length > 0;
}

export function dismissSetupRun(): void {
  state.active = false;
}

/** Collects the `setup-defaults` outcomes as emulators install. */
export function initSetupRun(): Promise<UnlistenFn> {
  return listen<SetupOutcome[]>(SETUP_DEFAULTS_EVENT, (e) => {
    state.outcomes = [...state.outcomes, ...e.payload];
  });
}
