// What TV Details keeps for the session (tvmode.js `S.detTab`,
// `S.saveKind`): the tab and the Saves | States choice, shared by every
// Details page, so coming back from a page pushed from Details (Game
// settings, Link, the folder browser), which remounts it, keeps them.
import type { SaveType } from '../../api';
import type { DetailsTab } from '../../details/tabs';

// `locations` is bumped when a save folder is added from the folder browser:
// popping the browser remounts Details, so the new Saves panel reloads.
export const det = $state<{ tab: DetailsTab; saveKind: SaveType; locations: number }>({ tab: 'overview', saveKind: 'save', locations: 0 });
