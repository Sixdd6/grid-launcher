// The `Ctrl+F` chord that focuses a grid view's search box (design §3).
// Pure, and shared by the Library and Server views so the two cannot
// disagree about when the chord applies.
//
// The blocking rules mirror `Shell.svelte`'s `chordBlocked` for `Ctrl+1..5`:
// a modal dialog owns the screen, or focus already sits in a text-entry
// control where the chord may mean something to the editor.

/** The subset of `KeyboardEvent` the chord test reads. */
export type ChordEvent = {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
};

/** What the document looks like at the moment the key arrives. */
export type ChordContext = {
  /** A `[role="dialog"]` is open. */
  dialogOpen: boolean;
  /** The focused element's tag name, uppercase, or null when nothing is focused. */
  activeTag: string | null;
  /** The focused element is `contenteditable`. */
  activeEditable: boolean;
};

const TEXT_ENTRY_TAGS = ['INPUT', 'TEXTAREA', 'SELECT'];

/**
 * `Ctrl+F` (or `Cmd+F`, so the same accelerator works on macOS). Alt and
 * Shift are excluded so this never steals a window-manager or text-editing
 * chord, exactly as the `Ctrl+<n>` view accelerators do.
 */
export function isSearchChord(e: ChordEvent): boolean {
  if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return false;
  return e.key.toLowerCase() === 'f';
}

/** True while the chord must stay out of the way. */
export function chordBlocked(ctx: ChordContext): boolean {
  if (ctx.dialogOpen) return true;
  if (ctx.activeEditable) return true;
  return ctx.activeTag !== null && TEXT_ENTRY_TAGS.includes(ctx.activeTag);
}

/** The one call a view makes: should this keydown focus the search box? */
export function shouldFocusSearch(e: ChordEvent, ctx: ChordContext): boolean {
  return isSearchChord(e) && !chordBlocked(ctx);
}

/** Reads the live context out of a document. Thin by design: everything
 *  worth testing is in the pure predicates above. */
export function chordContext(doc: Document): ChordContext {
  const el = doc.activeElement as HTMLElement | null;
  return {
    dialogOpen: doc.querySelector('[role="dialog"]') !== null,
    activeTag: el === null ? null : el.tagName,
    activeEditable: el?.isContentEditable === true,
  };
}
