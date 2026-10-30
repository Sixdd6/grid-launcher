// The size the background art is built at: the window in device pixels,
// rounded UP to 128px steps (`build_background_fill` in grid-core). The art
// is then drawn 1:1, with no scaling: scaling a 960px image up to a 4K window
// on every scrolled frame was the largest single cost of a frame when the
// webview composites on the CPU (perf probe A/B, 2026-10-30).
//
// Rounding to steps means a small window resize does not rebuild the art;
// the slightly larger image is centred and its edges cropped by the layer.

/** The step the size is rounded up to, in device pixels. */
export const FILL_STEP = 128;
/** The largest size asked for per side (an 8K screen), matching the
 *  backend's `BACKGROUND_FILL_MAX`. */
export const FILL_MAX = 7680;

export type Fill = { width: number; height: number; dpr: number };

/** The fill for a `width` x `height` CSS-pixel window at `dpr`. Pure. */
export function fillFor(width: number, height: number, dpr: number): Fill {
  const ratio = Number.isFinite(dpr) && dpr > 0 ? dpr : 1;
  const step = (px: number) =>
    Math.min(FILL_MAX, Math.max(FILL_STEP, Math.ceil((Math.max(1, px) * ratio) / FILL_STEP) * FILL_STEP));
  return { width: step(width), height: step(height), dpr: ratio };
}

const state = $state<{ fill: Fill }>({
  fill: typeof window === 'undefined' ? fillFor(1920, 1080, 1) : fillFor(window.innerWidth, window.innerHeight, window.devicePixelRatio),
});

/** The current fill. Reactive. */
export function viewportFill(): Fill {
  return state.fill;
}

/** Follows window resizes (and a moved-to screen's pixel ratio). Returns the
 *  disposer. Writes only when the rounded size changes. */
export function trackViewportFill(): () => void {
  const update = () => {
    const next = fillFor(window.innerWidth, window.innerHeight, window.devicePixelRatio);
    const cur = state.fill;
    if (next.width !== cur.width || next.height !== cur.height || next.dpr !== cur.dpr) state.fill = next;
  };
  update();
  window.addEventListener('resize', update);
  return () => window.removeEventListener('resize', update);
}
