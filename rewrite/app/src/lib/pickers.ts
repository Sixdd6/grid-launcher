// The ONE place `@tauri-apps/plugin-dialog` is imported. Components call
// `pickFolder`/`pickFile` instead, for three reasons: the plugin's `open`
// returns a union that every call site would otherwise have to narrow; a
// single seam keeps the capability (`dialog:allow-open`) auditable; and a
// failure to open a dialog — including the E2E build, which has no desktop
// portal behind it — degrades to "the user cancelled" rather than throwing
// into a component's click handler.
//
// Every Browse button in the app is ADDITIVE: the text input beside it stays
// and remains the path E2E drives, so no spec ever needs a real dialog.
import { open } from '@tauri-apps/plugin-dialog';

/** One existing directory, or `null` when the user cancelled. */
export async function pickFolder(title: string): Promise<string | null> {
  try {
    const picked = await open({ directory: true, multiple: false, title });
    return typeof picked === 'string' ? picked : null;
  } catch {
    return null;
  }
}

/**
 * One existing file, or `null` when the user cancelled. `filters` is passed
 * straight through; omit it to offer every file (an emulator entry may point
 * at a bare executable, an AppImage or a downloadable archive).
 */
export async function pickFile(
  title: string,
  filters?: { name: string; extensions: string[] }[]
): Promise<string | null> {
  try {
    const picked = await open({ directory: false, multiple: false, title, filters });
    return typeof picked === 'string' ? picked : null;
  } catch {
    return null;
  }
}
