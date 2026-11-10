// What TV Details keeps for the session (tvmode.js `S.detTab`,
// `S.saveKind`): the tab and the Saves | States choice, shared by every
// Details page, so coming back from a page pushed from Details (Game
// settings, Link, the folder browser), which remounts it, keeps them.
import type { SaveType } from '../../api';
import type { DetailsTab } from '../../details/tabs';

export const det = $state<{ tab: DetailsTab; saveKind: SaveType }>({ tab: 'overview', saveKind: 'save' });
