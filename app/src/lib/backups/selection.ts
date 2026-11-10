// Select mode on a grid (GameBackups README, "Select mode on the Server and
// Library grids"; plan Task 19). Pure: a view holds a `Selection` in its
// own state and passes in the ids it shows, in the order it shows them.
// Shared by Server and (Task 20) Library, so the two pick the same way.

/** The picked game ids, in the order they were picked, and the last one clicked (the Shift-click anchor). */
export type Selection = { readonly ids: readonly number[]; readonly anchor: number | null };

export const EMPTY_SELECTION: Selection = { ids: [], anchor: null };

export function isPicked(sel: Selection, id: number): boolean {
  return sel.ids.includes(id);
}

/** A plain click: picks `id`, or unpicks it when it was picked. */
export function toggle(sel: Selection, id: number): Selection {
  const ids = sel.ids.includes(id) ? sel.ids.filter((x) => x !== id) : [...sel.ids, id];
  return { ids, anchor: id };
}

/**
 * A click on a cover in select mode. With Shift, and an anchor still on
 * screen, it picks every cover from the anchor to `id` in the visible order
 * (and never unpicks); otherwise it toggles `id`.
 */
export function pick(sel: Selection, id: number, shift: boolean, visible: readonly number[]): Selection {
  if (!shift || sel.anchor === null) return toggle(sel, id);
  const a = visible.indexOf(sel.anchor);
  const z = visible.indexOf(id);
  if (a < 0 || z < 0) return toggle(sel, id);
  const range = visible.slice(Math.min(a, z), Math.max(a, z) + 1);
  const ids = [...sel.ids];
  for (const x of range) if (!ids.includes(x)) ids.push(x);
  return { ids, anchor: id };
}

/** Every visible cover is picked (and there is at least one). */
export function allPicked(sel: Selection, visible: readonly number[]): boolean {
  return visible.length > 0 && visible.every((id) => sel.ids.includes(id));
}

/**
 * "Select all N": picks every visible cover. Pressed again once they all
 * are ("All N selected"), it unpicks them.
 */
export function selectAll(sel: Selection, visible: readonly number[]): Selection {
  if (allPicked(sel, visible)) return { ids: sel.ids.filter((id) => !visible.includes(id)), anchor: null };
  const ids = [...sel.ids];
  for (const id of visible) if (!ids.includes(id)) ids.push(id);
  return { ids, anchor: sel.anchor };
}

export function clear(): Selection {
  return EMPTY_SELECTION;
}

/**
 * Drops ids that are no longer in the view's list (a reload removed a
 * game), so the count and the size never include a game the user can't see.
 */
export function keepOnly(sel: Selection, present: readonly number[]): Selection {
  const ids = sel.ids.filter((id) => present.includes(id));
  if (ids.length === sel.ids.length) return sel;
  return { ids, anchor: sel.anchor !== null && present.includes(sel.anchor) ? sel.anchor : null };
}

/** The picked games' total size. `sizeOf` answers 0 for a size the server did not state. */
export function totalBytes(sel: Selection, sizeOf: (id: number) => number): number {
  return sel.ids.reduce((sum, id) => sum + Math.max(0, sizeOf(id) || 0), 0);
}

/** The key that leaves select mode, when nothing else (a dialog, a text field) has it. */
export function leavesSelectMode(key: string, selecting: boolean, blocked: boolean): boolean {
  return selecting && !blocked && key === 'Escape';
}

// ── The selection bar's text ─────────────────────────────────────────────

/** "3 selected" with the size beside it, or the hint before anything is picked. */
export function selectionSummary(count: number, size: string): { title: string; note: string } {
  if (count === 0) return { title: 'Select games', note: 'Click covers to pick them · Shift-click picks a range' };
  return { title: `${count} selected`, note: size };
}

/** "Select all 11", or "All 11 selected" once they all are. */
export function selectAllLabel(visibleCount: number, all: boolean): string {
  return all ? `All ${visibleCount} selected` : `Select all ${visibleCount}`;
}
