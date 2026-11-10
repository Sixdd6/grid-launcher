// The fullscreen media viewer (checklist items 302–305; tvmode.js `viewer`):
// a layer over Details. LB / RB (and Left / Right) step through the
// screenshots and videos as one sequence, stopping at the ends; B closes.
import type { MediaItem } from '../../details/media';
import type { Hint } from '../hints';
import { openLayer } from '../nav.svelte';
import { seekTarget, viewerHints, viewerStep } from './details';
import TvMediaViewer from './TvMediaViewer.svelte';

export type ViewerModel = {
  items: MediaItem[];
  index: number;
  title: string;
  cover: string | null;
  onFail: (url: string) => void;
  /** A server video on show, and whether it is paused (ruling E6). */
  video?: HTMLVideoElement | null;
  paused?: boolean;
};

/** The viewer's hints (item 305; a server video's, ruling E6). */
export const VIEWER_HINTS: Hint[] = viewerHints(null, false);

/** A on a server video: play or pause it. */
export function toggleVideo(model: ViewerModel): void {
  const v = model.video;
  if (!v) return;
  if (v.paused) void v.play().catch(() => {});
  else v.pause();
}

/** LT / RT on a server video: 10 s back or forward. */
export function seekVideo(model: ViewerModel, delta: number): void {
  const v = model.video;
  if (!v || model.items[model.index]?.kind !== 'video') return;
  v.currentTime = seekTarget(v.currentTime, v.duration, delta);
}

export function openViewer(spec: ViewerModel): Promise<number> {
  const model: ViewerModel = $state({ ...spec, video: null, paused: false });
  const prev = () => (model.index = viewerStep(model.index, model.items.length, -1));
  const next = () => (model.index = viewerStep(model.index, model.items.length, 1));
  return openLayer({
    component: TvMediaViewer,
    props: { model },
    cls: 'viewer-l',
    hints: () => viewerHints(model.items[model.index]?.kind ?? null, model.paused === true),
    binds: { LB: prev, RB: next, LEFT: prev, RIGHT: next, LT: () => seekVideo(model, -10), RT: () => seekVideo(model, 10) },
  });
}
