// App-lifetime notices about the defaults an emulator install applied
// (Settings › Systems, a server package installed from Server). Registered
// once per shell lifetime by `lifetime.svelte.ts`, so each event toasts once
// however many views are mounted. The install's own
// queued/installed cards are notifications/notifier.ts's.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { SETUP_DEFAULTS_EVENT, type SetupOutcome } from '../api';
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
