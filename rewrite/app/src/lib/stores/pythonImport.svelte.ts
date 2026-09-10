// The startup Python-config import's one toast. Module-scoped so the toast
// survives a Shell remount without repeating: the import itself already
// happened once, before the webview existed.
import { api } from '../api';
import { importToastText } from '../pythonImport';
import { pushToast } from './toasts.svelte';

let shown = false;

/**
 * Pulls `python_import_notice` once and, when there was an import, shows one
 * toast. There is no event to listen for — the import finishes inside the
 * Rust `run()` before the window is created — so this is a pull only, unlike
 * `initAppUpdate`.
 *
 * Returns nothing to unsubscribe from; `App.svelte` calls it from an effect
 * like the other startup stores and ignores the result.
 */
export async function initPythonImport(): Promise<void> {
  if (shown) return;
  try {
    const report = await api.pythonImportNotice();
    if (report === null) return;
    shown = true;
    pushToast(importToastText(report));
  } catch {
    // A failed read is never surfaced: the import either happened or did
    // not, and the user finds their library either way.
  }
}
