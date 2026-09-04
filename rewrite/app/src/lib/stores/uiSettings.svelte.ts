// Appearance settings: the config-backed half of `lib/theme.ts`. Module
// scoped, like `appUpdate.svelte.ts`, so the resolved theme survives Shell
// remounts and every view reads one source.
import { api } from '../api';
import {
  clampFade,
  FADE_DEFAULT,
  normalizeTheme,
  resolveTheme,
  themeAttribute,
  type ResolvedTheme,
  type ThemeChoice,
} from '../theme';

const state = $state<{ theme: ThemeChoice; backgroundFade: number; prefersDark: boolean }>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  prefersDark: false,
});

export const uiSettings = {
  get theme(): ThemeChoice {
    return state.theme;
  },
  get backgroundFade(): number {
    return state.backgroundFade;
  },
  get resolved(): ResolvedTheme {
    return resolveTheme(state.theme, state.prefersDark);
  },
};

/** The single writer of `<html data-theme>`. */
function applyTheme(choice: ThemeChoice): void {
  const attribute = themeAttribute(choice);
  if (attribute === null) delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = attribute;
}

/**
 * Loads the stored settings, applies the attribute, and follows the OS
 * scheme for as long as the returned disposer is not called. Returns a
 * plain function (not a promise of one) so `$effect` teardown is trivial.
 * A failed load is NOT surfaced: the defaults are a perfectly usable
 * shell, and a missing config is the normal first-run case.
 */
export async function initUiSettings(): Promise<() => void> {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  state.prefersDark = media.matches;
  const onChange = (e: MediaQueryListEvent) => {
    state.prefersDark = e.matches;
  };
  media.addEventListener('change', onChange);

  try {
    const stored = await api.getUiSettings();
    state.theme = normalizeTheme(stored.theme);
    state.backgroundFade = clampFade(stored.background_fade);
  } catch {
    // Defaults already in `state`.
  }
  applyTheme(state.theme);

  return () => media.removeEventListener('change', onChange);
}

/** Applies immediately, then persists. */
export async function setTheme(choice: ThemeChoice): Promise<void> {
  state.theme = choice;
  applyTheme(choice);
  await api.setUiSettings({ theme: choice, background_fade: state.backgroundFade });
}

/** Slider drag: updates the live preview without touching the config. */
export function previewBackgroundFade(value: number): void {
  state.backgroundFade = clampFade(value);
}

/** Slider release: persists whatever the preview settled on. */
export async function commitBackgroundFade(value: number): Promise<void> {
  previewBackgroundFade(value);
  await api.setUiSettings({ theme: state.theme, background_fade: state.backgroundFade });
}
