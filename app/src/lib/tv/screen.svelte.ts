// How a screen (a tab, or a page) registers its hooks with the shell: TvStage
// wraps each screen in a context naming it, and the screen calls
// `useScreen(hooks)` from its script. The hooks object is read lazily, so a
// screen may answer from its own reactive state.
import { getContext, setContext, untrack } from 'svelte';
import { registerScreen, type ScreenHooks } from './nav.svelte';

const KEY = Symbol('tv-screen');

export function provideScreen(key: () => string): void {
  setContext(KEY, key);
}

/** Registers `hooks` for the screen this component is in, while it is mounted. */
export function useScreen(hooks: ScreenHooks): void {
  const key = getContext<(() => string) | undefined>(KEY);
  if (!key) return;
  // Untracked: registering bumps a counter it also reads, which would make
  // this effect depend on itself and loop.
  $effect(() => {
    const k = key();
    return untrack(() => registerScreen(k, hooks));
  });
}
