// The fullscreen media viewer (checklist items 302–305; tvmode.js `viewer`):
// a layer over Details. Screenshots page with LB / RB and Left / Right,
// stopping at the ends; B closes. While a video shows — a server video, or a
// trailer in the YouTube player — A plays and pauses it, LB / RB seek 10 s,
// LT / RT step its own volume (held, they repeat), Left / Right page and X
// opens a trailer on YouTube (user decision 2026-11-10, replacing ruling E6).
import { api } from '../../api';
import type { MediaItem } from '../../details/media';
import { stepVolume, toElementVolume, VOLUME_READOUT_MS } from '../../details/videoVolume';
import { embedPaused, embedShowing, initialEmbed, type EmbedState, type RelayCommand } from '../../details/youtubeRelay';
import { pushToast } from '../../stores/toasts.svelte';
import { setVideoVolume, uiSettings } from '../../stores/uiSettings.svelte';
import type { Hint } from '../hints';
import type { TvButton } from '../input/buttons';
import { openLayer } from '../nav.svelte';
import {
  seekTarget,
  VIEWER_BUTTONS,
  viewerAction,
  viewerHints,
  viewerRepeats,
  viewerShow,
  viewerStep,
  watchOffered,
  type ViewerShow,
} from './details';
import TvMediaViewer from './TvMediaViewer.svelte';

export type ViewerModel = {
  items: MediaItem[];
  index: number;
  title: string;
  cover: string | null;
  onFail: (url: string) => void;
  /** A server video on show, and whether it is paused. */
  video?: HTMLVideoElement | null;
  paused?: boolean;
  /** The YouTube player on show (its relay), and where it stands. */
  yt?: { send: (c: RelayCommand) => void } | null;
  embed?: EmbedState;
  /** The "Volume N%" readout is showing. */
  readout?: boolean;
  /** A trailer asked to play hasn't started within STALL_MS. */
  stalled?: boolean;
  /** Counts A's requests to play a paused trailer (each restarts the
   *  stall check). */
  playAsk?: number;
};

/** The viewer's hints with nothing on show (item 305). */
export const VIEWER_HINTS: Hint[] = viewerHints(null, false);

function current(model: ViewerModel): MediaItem | null {
  return model.items[model.index] ?? null;
}

/** What the viewer shows now, for its controls. */
export function showOf(model: ViewerModel): ViewerShow {
  const item = current(model);
  return viewerShow(item?.kind ?? null, embedShowing(model.embed ?? initialEmbed()));
}

/** Paused, for A's label: the server video's element, or the YouTube
 *  player's last reported state. */
export function pausedOf(model: ViewerModel): boolean {
  if (current(model)?.kind === 'youtube') return embedPaused(model.embed ?? initialEmbed());
  return model.paused === true;
}

/** Whether the trailer on show offers "(X) Watch on YouTube" now. */
export function watchShown(model: ViewerModel): boolean {
  return current(model)?.kind === 'youtube' && watchOffered(showOf(model), model.stalled === true);
}

/** A on a video: play or pause it. On a trailer's poster: open YouTube. */
export function toggleVideo(model: ViewerModel): void {
  const item = current(model);
  if (item?.kind === 'youtube') {
    if (showOf(model) === 'video') {
      if (pausedOf(model)) model.playAsk = (model.playAsk ?? 0) + 1;
      model.yt?.send({ cmd: 'toggle' });
    } else watchOnYoutube(item.videoId);
    return;
  }
  const v = model.video;
  if (!v) return;
  if (v.paused) void v.play().catch(() => {});
  else v.pause();
}

export function watchOnYoutube(videoId: string): void {
  api.openYoutubeVideo(videoId).catch((e: unknown) => pushToast(e instanceof Error ? e.message : String(e), 'error'));
}

/** LB / RB on a video: 10 s back or forward. */
export function seekVideo(model: ViewerModel, delta: number): void {
  const item = current(model);
  if (item?.kind === 'youtube') {
    model.yt?.send({ cmd: 'seek', by: delta });
    return;
  }
  const v = model.video;
  if (!v || item?.kind !== 'video') return;
  v.currentTime = seekTarget(v.currentTime, v.duration, delta);
}

let readoutTimer: ReturnType<typeof setTimeout> | null = null;

/** LT / RT on a video: one 10-point step of the player's own volume, stored
 *  (debounced) as `ui.video_volume`, and the readout beside the player. */
export function stepViewerVolume(model: ViewerModel, dir: -1 | 1): void {
  const value = stepVolume(uiSettings.videoVolume, dir);
  setVideoVolume(value);
  applyVolume(model, value);
  model.readout = true;
  if (readoutTimer !== null) clearTimeout(readoutTimer);
  readoutTimer = setTimeout(() => {
    readoutTimer = null;
    model.readout = false;
  }, VOLUME_READOUT_MS);
}

/** Sets the player on show to `value` (0–100). */
export function applyVolume(model: ViewerModel, value: number): void {
  const item = current(model);
  if (item?.kind === 'youtube') model.yt?.send({ cmd: 'volume', value });
  else if (model.video) {
    model.video.volume = toElementVolume(value);
    if (value > 0) model.video.muted = false;
  }
}

function act(model: ViewerModel, b: TvButton): void {
  const item = current(model);
  const a = viewerAction(b as (typeof VIEWER_BUTTONS)[number], showOf(model), item?.kind === 'youtube', watchShown(model));
  switch (a) {
    case 'prev':
      model.index = viewerStep(model.index, model.items.length, -1);
      return;
    case 'next':
      model.index = viewerStep(model.index, model.items.length, 1);
      return;
    case 'back':
      seekVideo(model, -10);
      return;
    case 'forward':
      seekVideo(model, 10);
      return;
    case 'volDown':
      stepViewerVolume(model, -1);
      return;
    case 'volUp':
      stepViewerVolume(model, 1);
      return;
    case 'watch':
      if (item?.kind === 'youtube') watchOnYoutube(item.videoId);
      return;
    default:
      return;
  }
}

export function openViewer(spec: ViewerModel): Promise<number> {
  // `paused` follows the element's play / pause events (not `bind:paused`,
  // which would call play() on a video that can't play and leave an
  // unhandled rejection).
  const model: ViewerModel = $state({ ...spec, video: null, paused: true, yt: null, embed: initialEmbed(), readout: false });
  const binds: Partial<Record<TvButton, () => void>> = {};
  for (const b of VIEWER_BUTTONS) binds[b] = () => act(model, b);
  return openLayer({
    component: TvMediaViewer,
    props: { model },
    cls: 'viewer-l',
    hints: () => viewerHints(showOf(model), pausedOf(model)),
    binds,
    dirRepeat: (b) => viewerRepeats(b, showOf(model)),
  });
}
