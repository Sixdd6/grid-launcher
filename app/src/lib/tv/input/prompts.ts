// Which prompt set the screen shows (TVMode README § Decided "Button
// prompts"; checklist items 56, 67–69). Pure; the tvmode.js `scheme()`.
import type { TvAssignment, TvPrompts } from '../../api';
import type { PromptSet } from '../glyphs/glyphs';
import type { PadBrand } from './controllers';

/**
 * A set chosen by hand wins for every controller. "Match my controller"
 * follows the bindings of the controller pressed last: Xbox style shows the
 * Xbox set (the PlayStation set on a PlayStation pad); Nintendo style shows
 * the Nintendo set (the 8BitDo Pro set on an 8BitDo pad). With no
 * controller, `assign` is Xbox style and `brand` Xbox (item 56).
 */
export function promptSet(setting: TvPrompts, assign: TvAssignment, brand: PadBrand): PromptSet {
  if (setting !== 'match') return setting;
  if (assign === 'nintendo') return brand === '8bitdo' ? '8bitdo' : 'nintendo';
  return brand === 'ps' ? 'ps' : 'xbox';
}
