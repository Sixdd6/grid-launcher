// Emulator update availability for Settings › Systems, cached for the
// session. The checks ask GitHub (or the server) once per emulator, so they
// run once per session, the first time Systems opens — never on a scroll or
// a re-render — and again only when the user asks (Check for updates).
// Nothing persists: the next session checks afresh.
import { api, type EmulatorEntry, type VersionCheck } from '../api';

const state = $state<{ checks: Record<string, VersionCheck>; running: boolean; started: boolean }>({
  checks: {},
  running: false,
  started: false,
});

export const emulatorUpdates = {
  /** The cached answer for an emulator, or `undefined` when unchecked. */
  check(name: string): VersionCheck | undefined {
    return state.checks[name];
  },
  /** Whether a newer build of `name` is out, as last checked. */
  hasUpdate(name: string): boolean {
    const check = state.checks[name];
    return check !== undefined && updateIsOut(check);
  },
  get running() {
    return state.running;
  },
  get started() {
    return state.started;
  },
};

/**
 * Whether a check found a newer build. A direct-download source can never
 * say ("Unknown (direct source)"), and an install with no recorded version
 * ("unknown") cannot be compared: neither is an update that is out.
 */
export function updateIsOut(check: VersionCheck): boolean {
  if (check.up_to_date) return false;
  if (check.installed_display.trim().toLowerCase() === 'unknown') return false;
  if (check.available_display.trim().toLowerCase().startsWith('unknown')) return false;
  return check.available_display !== check.installed_display;
}

/** The emulators an update check can answer for: installed from a source. */
export function checkable(entries: readonly EmulatorEntry[]): string[] {
  return entries.filter((e) => (e.source_id ?? '').trim() !== '').map((e) => e.name);
}

async function run(names: readonly string[]): Promise<void> {
  state.running = true;
  try {
    // One at a time: a dozen emulators must not fire a dozen requests at
    // the same rate-limited API in the same instant.
    for (const name of names) {
      try {
        state.checks = { ...state.checks, [name]: await api.checkEmulatorUpdate(name) };
      } catch {
        // Unanswered (offline, rate-limited): no claim either way.
        const { [name]: _gone, ...rest } = state.checks;
        state.checks = rest;
      }
    }
  } finally {
    state.running = false;
  }
}

/** The once-per-session check: the first call runs, every later one is a no-op. */
export function checkOncePerSession(entries: readonly EmulatorEntry[]): Promise<void> {
  if (state.started) return Promise.resolve();
  state.started = true;
  return run(checkable(entries));
}

/** Check for updates, asked for: checks every emulator again. */
export function recheck(entries: readonly EmulatorEntry[]): Promise<void> {
  if (state.running) return Promise.resolve();
  state.started = true;
  return run(checkable(entries));
}

/** An update or reinstall of `name` finished: it is current now. */
export function markCurrent(name: string): void {
  const check = state.checks[name];
  if (check === undefined) return;
  state.checks = {
    ...state.checks,
    [name]: { ...check, installed_display: check.available_display, up_to_date: true },
  };
}

/** For tests: back to a fresh session. */
export function resetForTests(): void {
  state.checks = {};
  state.running = false;
  state.started = false;
}
