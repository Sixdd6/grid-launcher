// Appearance settings: the config-backed half of `lib/theme.ts`. Module
// scoped, like `appUpdate.svelte.ts`, so the resolved theme survives Shell
// remounts and every view reads one source.
import { api } from '../api';
import type { UiSettings } from '../api';
import {
  clampFade,
  FADE_DEFAULT,
  normalizeTheme,
  resolveTheme,
  themeAttribute,
  themeFromStorageValue,
  THEME_STORAGE_KEY,
  type ResolvedTheme,
  type ThemeChoice,
} from '../theme';

const state = $state<{
  theme: ThemeChoice;
  backgroundFade: number;
  prefersDark: boolean;
  cardSizeLibrary: UiSettings['card_size_library'];
  cardSizeServer: UiSettings['card_size_server'];
}>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  prefersDark: false,
  cardSizeLibrary: 'medium',
  cardSizeServer: 'medium',
});

/** The one place the whole `UiSettings` payload is assembled, so no writer
 *  can drop a field another writer owns. */
function payload(): UiSettings {
  return {
    theme: state.theme,
    background_fade: state.backgroundFade,
    card_size_library: state.cardSizeLibrary,
    card_size_server: state.cardSizeServer,
  };
}

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

/**
 * Best-effort write of the localStorage hint that index.html reads inline.
 * Only ever called once a choice is known to match `config.toml` — after a
 * successful save, or after the config load reconciles the hint. A mirror
 * written ahead of a failed save would pre-paint an unsaved theme on the
 * next launch and then flip, which is the flash the mirror exists to stop.
 */
function mirrorTheme(choice: ThemeChoice): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    // No storage (private mode, disabled): the hint is an optimization,
    // not a requirement — the config load still resolves the theme.
  }
}

/** Reads the mirrored hint back, tolerating a missing/blocked localStorage. */
function readStoredTheme(): ResolvedTheme | null {
  try {
    return themeFromStorageValue(localStorage.getItem(THEME_STORAGE_KEY));
  } catch {
    return null;
  }
}

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
 *
 * The `api.getUiSettings()` round-trip is IPC + disk, not one frame, so an
 * explicit theme is pre-applied synchronously from the localStorage mirror
 * before that `await` — otherwise the shell paints under whichever scheme
 * `prefers-color-scheme` and the light defaults produce, then flips once the
 * config arrives. The mirror is reconciled against the config value once it
 * loads, applying again only if the two disagree.
 */
export async function initUiSettings(): Promise<() => void> {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  state.prefersDark = media.matches;
  const onChange = (e: MediaQueryListEvent) => {
    state.prefersDark = e.matches;
  };
  media.addEventListener('change', onChange);

  const hint = readStoredTheme();
  if (hint !== null) {
    state.theme = hint;
    applyTheme(hint);
  }

  try {
    const stored = await api.getUiSettings();
    state.theme = normalizeTheme(stored.theme);
    state.backgroundFade = clampFade(stored.background_fade);
    state.cardSizeLibrary = stored.card_size_library;
    state.cardSizeServer = stored.card_size_server;
  } catch {
    // Defaults/mirror already in `state`.
  }
  if (state.theme !== hint) {
    applyTheme(state.theme);
    mirrorTheme(state.theme);
  }

  return () => media.removeEventListener('change', onChange);
}

/**
 * Applies immediately, then persists. The localStorage mirror is written
 * only once the save resolves, so a failed write leaves the hint agreeing
 * with `config.toml`.
 */
export async function setTheme(choice: ThemeChoice): Promise<void> {
  state.theme = choice;
  applyTheme(choice);
  await api.setUiSettings(payload());
  mirrorTheme(choice);
}

/** Slider drag: updates the live preview without touching the config. */
export function previewBackgroundFade(value: number): void {
  state.backgroundFade = clampFade(value);
}

/** Slider release: persists whatever the preview settled on. */
export async function commitBackgroundFade(value: number): Promise<void> {
  previewBackgroundFade(value);
  await api.setUiSettings(payload());
}
