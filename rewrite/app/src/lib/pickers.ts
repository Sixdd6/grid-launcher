// The ONE place `@tauri-apps/plugin-dialog` is imported. Components call
// `pickFolder`/`pickFile` instead, for three reasons: the plugin's `open`
// returns a union that every call site would otherwise have to narrow; a
// single seam keeps the capability (`dialog:allow-open`) auditable; and both
// outcomes that aren't a picked path — the user cancelling and `open`
// throwing — degrade to `null` so a component's click handler never throws.
//
// Every Browse button in the app is ADDITIVE: the text input beside it stays
// and remains the path E2E drives, so no spec ever needs a real dialog. The
// E2E build has no desktop portal behind it, so `open` always throws there;
// that's expected, not a failure worth telling the user about.
import { open } from '@tauri-apps/plugin-dialog';
import { pushToast } from './stores/toasts.svelte';

const FAILURE_MESSAGE = 'Could not open a file dialog. Enter the path by hand.';

// Reported once per process: a user who hits it once (no portal, a sandboxed
// desktop) will hit it on every subsequent Browse click, and repeating the
// same toast adds noise without adding information.
let reported = false;

function reportFailure(): void {
  if (import.meta.env.VITE_E2E || reported) return;
  reported = true;
  pushToast(FAILURE_MESSAGE, 'error');
}

/** One existing directory, or `null` when the user cancelled or the dialog failed to open. */
export async function pickFolder(title: string): Promise<string | null> {
  try {
    const picked = await open({ directory: true, multiple: false, title });
    return typeof picked === 'string' ? picked : null;
  } catch {
    reportFailure();
    return null;
  }
}

/**
 * One existing file, or `null` when the user cancelled or the dialog failed
 * to open. `filters` is passed straight through; omit it to offer every file
 * (an emulator entry may point at a bare executable, an AppImage or a
 * downloadable archive).
 */
export async function pickFile(
  title: string,
  filters?: { name: string; extensions: string[] }[]
): Promise<string | null> {
  try {
    const picked = await open({ directory: false, multiple: false, title, filters });
    return typeof picked === 'string' ? picked : null;
  } catch {
    reportFailure();
    return null;
  }
}
