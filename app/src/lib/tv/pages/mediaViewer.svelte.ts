// The fullscreen media viewer (checklist items 302–305; tvmode.js `viewer`):
// a layer over Details. LB / RB (and Left / Right) step through the
// screenshots and videos as one sequence, stopping at the ends; B closes.
import type { MediaItem } from '../../details/media';
import type { Hint } from '../hints';
import { openLayer } from '../nav.svelte';
import { viewerStep } from './details';
import TvMediaViewer from './TvMediaViewer.svelte';

export type ViewerModel = {
  items: MediaItem[];
  index: number;
  title: string;
  cover: string | null;
  onFail: (url: string) => void;
};

/** The viewer's hints (item 305). */
export const VIEWER_HINTS: Hint[] = [
  { button: 'LB', label: 'Previous' },
  { button: 'RB', label: 'Next' },
  { button: 'B', label: 'Close' },
];

export function openViewer(spec: ViewerModel): Promise<number> {
  const model: ViewerModel = $state({ ...spec });
  const prev = () => (model.index = viewerStep(model.index, model.items.length, -1));
  const next = () => (model.index = viewerStep(model.index, model.items.length, 1));
  return openLayer({
    component: TvMediaViewer,
    props: { model },
    cls: 'viewer-l',
    hints: VIEWER_HINTS,
    binds: { LB: prev, RB: next, LEFT: prev, RIGHT: next },
  });
}
