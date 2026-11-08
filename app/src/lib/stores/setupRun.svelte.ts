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
  errors: string[];
  outcomes: SetupOutcome[];
  /** Each queued install's name, as the chooser showed it. */
  names: Record<string, string>;
  /** The Proton install queued for Windows games, if any. */
  compatQueued: string | null;
  /** The Proton pick was the default at once. */
  compatSet: boolean;
}>({ active: false, queued: [], errors: [], outcomes: [], names: {}, compatQueued: null, compatSet: false });

export const setupRun = {
  get active() {
    return state.active;
  },
  get queued() {
    return state.queued;
  },
  get errors() {
    return state.errors;
  },
  get outcomes() {
    return state.outcomes;
  },
  get names() {
    return state.names;
  },
  get compat() {
    return { queued: state.compatQueued, set: state.compatSet };
  },
};

/** Called with `startPlatformSetup`'s answer as the chooser hands over,
 *  with the name it showed for each install. */
export function startSetupRun(started: SetupStarted, names: Record<string, string> = {}): void {
  state.queued = started.queued;
  state.errors = started.errors;
  state.outcomes = started.applied;
  state.names = names;
  state.compatQueued = started.compat_queued;
  state.compatSet = started.compat_set;
  state.active =
    started.queued.length > 0 ||
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
