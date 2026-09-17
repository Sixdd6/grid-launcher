// The startup library-layout migration's one failure toast. Module-scoped so
// the toast survives a Shell remount without repeating: the migration itself
// already ran once, before the webview existed.
import { api } from '../api';
import { migrationToastText } from '../layoutMigration';
import { pushToast } from './toasts.svelte';

/** The notice explains a half-done library move, so it stays up far longer
 *  than a routine 4 s toast. */
export const MIGRATION_TOAST_DURATION_MS = 20000;

let shown = false;

/**
 * Pulls `layout_migration_notice` once and, when the migration failed, shows
 * one toast. There is no event to listen for — the migration finishes inside
 * the Rust `run()` before the window is created — so this is a pull only,
 * the same shape as `initPythonImport`.
 */
export async function initLayoutMigration(): Promise<void> {
  if (shown) return;
  // Claimed before the await, so two calls in the same tick cannot both get
  // past the guard and push the toast twice.
  shown = true;
  try {
    const notice = await api.layoutMigrationNotice();
    if (notice === null) return;
    pushToast(migrationToastText(notice.message), 'error', MIGRATION_TOAST_DURATION_MS);
  } catch {
    // A failed read is never surfaced: the migration retries next start
    // either way.
  }
}
