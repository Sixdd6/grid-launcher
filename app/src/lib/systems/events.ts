// App-lifetime emulator notifications for every emulator install (Settings ›
// Systems, first-run setup, a server package installed from Server).
// Registered once by Shell.svelte, which is mounted exactly once and never
// hidden, so each event toasts once however many views are mounted.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  EMULATOR_INSTALLED_EVENT,
  SETUP_DEFAULTS_EVENT,
  type EmulatorInstalledEvent,
  type SetupOutcome,
} from '../api';
import { installedToastText } from '../emulators/update';
import { markCurrent } from '../stores/emulatorUpdates.svelte';
import { setupRun } from '../stores/setupRun.svelte';
import { pushToast } from '../stores/toasts.svelte';

/**
 * The notices a Systems install's defaults deserve when no setup card is
 * there to say them: a RetroArch pick that found no core leaves the system
 * without an emulator, which would otherwise pass in silence.
 */
export function outcomeToasts(outcomes: readonly SetupOutcome[]): string[] {
  return outcomes.flatMap((o) =>
    o.outcome === 'core_missing'
      ? [`${o.emulator} installed, but it has no core for ${o.platform}, so ${o.platform} still has no emulator.`]
      : [],
  );
}

export function initEmulatorEvents(): () => void {
  const pending: Promise<UnlistenFn>[] = [
    // The completion notice for an install OR an update, worded by the
    // backend's `fresh` flag. The install runs in the background, so the view
    // it started from may be long gone by then.
    listen<EmulatorInstalledEvent>(EMULATOR_INSTALLED_EVENT, (event) => {
      pushToast(installedToastText(event.payload.name, event.payload.fresh));
      // A fresh install or an update both put the newest build on disk.
      markCurrent(event.payload.name);
    }),
    listen<SetupOutcome[]>(SETUP_DEFAULTS_EVENT, (event) => {
      // The setup card reports these itself while it is up.
      if (setupRun.active) return;
      for (const text of outcomeToasts(event.payload)) pushToast(text, 'error');
    }),
  ];
  return () => {
    for (const p of pending) void p.then((off) => off());
  };
}
