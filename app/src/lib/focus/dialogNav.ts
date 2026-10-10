// Gamepad `nav` routing for modal dialogs. The Rust side emits one `nav` event
// per press; `App.svelte` hands it to `Shell.handleNav`, which asks this
// registry first. A dialog registers its own `handleNav` while it is mounted,
// so while any dialog is open the topmost one gets the action and nothing
// reaches Library / Server / Details underneath it.
//
// The registry is a stack. Dialogs close in any order (a successful link
// unmounts the picker on its own), so removal is by token, not by `pop`.

import type { NavDirection } from './grid';

export type NavAction = NavDirection | 'accept' | 'back';

/** Returns true when the dialog acted on the action. Either way the action
 *  is consumed: an open dialog owns the screen. */
export type DialogNavHandler = (action: NavAction) => boolean;

const stack: { handler: DialogNavHandler }[] = [];

/** Registers an open dialog as the topmost. Returns its unregister function
 *  (call it from the component's teardown). */
export function registerDialog(handler: DialogNavHandler): () => void {
  const entry = { handler };
  stack.push(entry);
  return () => {
    const i = stack.indexOf(entry);
    if (i !== -1) stack.splice(i, 1);
  };
}

export function hasOpenDialog(): boolean {
  return stack.length > 0;
}

/** Sends the action to the topmost dialog. Returns true when a dialog is
 *  open (the caller must stop routing), false when none is. */
export function routeDialogNav(action: NavAction): boolean {
  const top = stack[stack.length - 1];
  if (!top) return false;
  top.handler(action);
  return true;
}

/** Test seam: forget every registered dialog. */
export function resetDialogsForTest(): void {
  stack.length = 0;
}

/**
 * Which item gets focus after `action`, in a list of `count` items where
 * `current` is the focused one (-1 when none is). Clamped, no wrap — the same
 * rule as the grids. `up`/`down` always step; `left`/`right` step only for a
 * row of controls (`horizontal`). Any other action leaves the index alone.
 */
export function stepIndex(current: number, action: NavAction, count: number, horizontal = false): number {
  if (count <= 0) return -1;
  const prev = action === 'up' || (horizontal && action === 'left');
  const next = action === 'down' || (horizontal && action === 'right');
  if (!prev && !next) return current;
  if (current < 0) return 0;
  return Math.min(Math.max(current + (next ? 1 : -1), 0), count - 1);
}

/** Moves DOM focus along `items` for a directional action. Returns true when
 *  the action was a move this helper owns. */
export function moveDialogFocus(items: HTMLElement[], action: NavAction, horizontal = false): boolean {
  if (action !== 'up' && action !== 'down' && !(horizontal && (action === 'left' || action === 'right'))) {
    return false;
  }
  const current = items.indexOf(document.activeElement as HTMLElement);
  items[stepIndex(current, action, items.length, horizontal)]?.focus();
  return true;
}

/** Activates the focused control for `accept`: a button is clicked; a
 *  `<select>` opens its list where the engine allows it. Text inputs are left
 *  to the caller. */
export function activateFocused(panel: HTMLElement | null): boolean {
  const el = document.activeElement;
  if (!panel || !(el instanceof HTMLElement) || !panel.contains(el)) return false;
  if (el instanceof HTMLSelectElement) {
    try {
      el.showPicker();
    } catch {
      el.click();
    }
    return true;
  }
  if (el instanceof HTMLButtonElement) {
    if (el.getAttribute('aria-disabled') !== 'true') el.click();
    return true;
  }
  return false;
}
