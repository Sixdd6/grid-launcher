// The display's device pixel ratio, reactive: moving the window to a screen
// with another scale, or zooming, changes which cover a card needs
// (`cards/cover.ts`). Module-scoped `$state`, like `inputMode.svelte.ts`.

function read(): number {
  const ratio = typeof window === 'undefined' ? 1 : window.devicePixelRatio;
  return Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
}

const state = $state({ current: read() });

/**
 * Re-reads the ratio whenever it moves off the current value. A resolution
 * media query matches only the value it was made with, so each change
 * re-arms the watch at the new value.
 */
function watch(): void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
  const query = window.matchMedia(`(resolution: ${state.current}dppx)`);
  const onChange = () => {
    query.removeEventListener('change', onChange);
    state.current = read();
    watch();
  };
  query.addEventListener('change', onChange);
}

watch();

export const pixelRatio = {
  get current(): number {
    return state.current;
  },
};
