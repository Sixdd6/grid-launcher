// Which prompt set the screen shows, and which glyph each button draws
// (TVMode README § Decided "Button prompts"; checklist items 56, 67–69).
// Pure; the tvmode.js `scheme()` and `gl()`.
import type { TvAssignment, TvPrompts } from '../../api';
import type { GlyphButton, PromptSet } from '../glyphs/glyphs';
import type { PadBrand } from './controllers';

/**
 * A set chosen by hand wins for every controller. "Match my controller"
 * follows the controller pressed last as it is built, never its button
 * assignment: a Nintendo-layout pad (`layout` 'nintendo') shows the
 * Nintendo set (the 8BitDo Pro set on an 8BitDo pad); every other pad shows
 * the Xbox set (the PlayStation set on a PlayStation pad). With no
 * controller, `layout` is Xbox and `brand` Xbox (item 56).
 */
export function promptSet(setting: TvPrompts, layout: TvAssignment, brand: PadBrand): PromptSet {
  if (setting !== 'match') return setting;
  if (layout === 'nintendo') return brand === '8bitdo' ? '8bitdo' : 'nintendo';
  return brand === 'ps' ? 'ps' : 'xbox';
}

/** Whose face-button layout a prompt set's glyphs show. */
function setLayout(set: PromptSet): TvAssignment {
  return set === 'nintendo' || set === '8bitdo' ? 'nintendo' : 'xbox';
}

const SWAP: Partial<Record<GlyphButton, GlyphButton>> = { A: 'B', B: 'A', X: 'Y', Y: 'X' };

/**
 * The glyph a prompt for `button` (what the button does: A selects, B goes
 * back) draws: the physical button bound to it under `assign`, as the set
 * prints it. The assignment changes what each button does, never how the
 * buttons look: a Nintendo pad in Xbox style selects with its bottom button,
 * which the Nintendo set prints as B. Only the four face buttons move.
 */
export function faceGlyph(button: GlyphButton, assign: TvAssignment, set: PromptSet): GlyphButton {
  return setLayout(set) === assign ? button : (SWAP[button] ?? button);
}
