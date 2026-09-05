// The app-wide transient message surface — the port of `show_toast`
// (grid_launcher/ui/toast.py:97) and its `ToastWidget` (toast.py:7-95).
// Module-scoped `$state` so any component can push without prop drilling,
// mirroring `stores/appUpdate.svelte.ts`.
//
// The `append*`/`remove*` pair below is pure and exported for vitest; the
// `$state` wrapper under it is the part components read. Same split as
// `stores/updates.svelte.ts`'s `labelFor`.

export type ToastLevel = 'success' | 'error';

export type Toast = { id: number; text: string; level: ToastLevel };

/**
 * Python shows exactly one toast at a time (a single reused `ToastWidget`,
 * toast.py:100-105). The rewrite keeps a short stack instead so a fan-out
 * that reports twice in quick succession does not lose the first line; three
 * is the most that fits above the download strip without covering content.
 */
export const TOAST_LIMIT = 3;

/**
 * `ToastWidget.__init__`'s `duration_ms` default is 2400 (toast.py:15). The
 * rewrite uses 4000: a WebDriver round trip plus a command's own latency
 * eats a visible slice of 2400 ms, and E2E asserts the "Added emulator"
 * text. Deliberate, documented deviation.
 */
export const TOAST_DURATION_MS = 4000;

/** `show_message` ignores a blank message (toast.py:64-66); so does this. */
export function appendToast(list: Toast[], next: Toast, limit: number = TOAST_LIMIT): Toast[] {
  if (next.text.trim() === '') return list;
  return [...list, next].slice(-limit);
}

export function removeToast(list: Toast[], id: number): Toast[] {
  return list.filter((t) => t.id !== id);
}

const state = $state<{ list: Toast[] }>({ list: [] });

let nextId = 0;

export const toasts = {
  get list() {
    return state.list;
  },
};

/**
 * Shows `text` for [`TOAST_DURATION_MS`]. Returns the new toast's id, or
 * `null` when the message was blank and nothing was shown.
 */
export function pushToast(text: string, level: ToastLevel = 'success'): number | null {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  nextId += 1;
  const id = nextId;
  state.list = appendToast(state.list, { id, text: trimmed, level });
  setTimeout(() => dismissToast(id), TOAST_DURATION_MS);
  return id;
}

export function dismissToast(id: number): void {
  state.list = removeToast(state.list, id);
}
