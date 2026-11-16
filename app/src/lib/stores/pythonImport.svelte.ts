// The startup Python-config import's one toast, and the report the welcome
// reads to show its import notice (decision 12: "Your settings from the
// previous version are filled in."). Module-scoped so the toast survives a
// remount without repeating: the import itself already happened once,
// before the webview existed.
import { api, type PythonImportReport } from '../api';
import { importToastText } from '../pythonImport';
import { pushToast } from './toasts.svelte';

/** The notice is a one-time instruction to re-enter two tokens, so it stays
 *  up far longer than a routine 10 s card. */
export const IMPORT_TOAST_DURATION_MS = 20000;

let shown = false;

/** The import's report, `null` until pulled or when nothing was imported. */
export const pythonImport = $state<{ report: PythonImportReport | null }>({ report: null });

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
  // Claimed before the await, so two calls in the same tick cannot both get
  // past the guard and push the toast twice.
  shown = true;
  try {
    const report = await api.pythonImportNotice();
    if (report === null) return;
    pythonImport.report = report;
    pushToast(importToastText(report), 'success', IMPORT_TOAST_DURATION_MS);
  } catch {
    // A failed read is never surfaced: the import either happened or did
    // not, and the user finds their library either way.
  }
}
