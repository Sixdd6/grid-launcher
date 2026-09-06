// Appearance settings: the config-backed half of `lib/theme.ts`. Module
// scoped, like `appUpdate.svelte.ts`, so the resolved theme survives Shell
// remounts and every view reads one source.
import { api } from '../api';
import type { UiSettings } from '../api';
import { clearVariantMemo } from '../backgroundPrefetch';
import { normalizeCardSize, type CardSize } from '../cards/size';
import { fadeForToggle, rememberFade } from '../settings/appearance';
import {
  BLUR_DEFAULT,
  clampBlur,
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
  backgroundBlur: number;
  prefersDark: boolean;
  cardSizeLibrary: UiSettings['card_size_library'];
  cardSizeServer: UiSettings['card_size_server'];
}>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  backgroundBlur: BLUR_DEFAULT,
  prefersDark: false,
  cardSizeLibrary: 'medium',
  cardSizeServer: 'medium',
});

// What "background art on" restores (design §10): the last non-zero fade
// this session saw — loaded from config or dragged on the slider. Module
// scoped like `state`, not persisted: fade 0 in config means off, and the
// default is what a fresh "on" goes back to.
let rememberedFade = FADE_DEFAULT;

/** The one place the whole `UiSettings` payload is assembled, so no writer
 *  can drop a field another writer owns. */
function payload(): UiSettings {
  return {
    theme: state.theme,
    background_fade: state.backgroundFade,
    background_blur: state.backgroundBlur,
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
  get backgroundBlur(): number {
    return state.backgroundBlur;
  },
  get resolved(): ResolvedTheme {
    return resolveTheme(state.theme, state.prefersDark);
  },
  get cardSizeLibrary(): CardSize {
    return state.cardSizeLibrary;
  },
  get cardSizeServer(): CardSize {
    return state.cardSizeServer;
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
    state.backgroundBlur = clampBlur(stored.background_blur);
    rememberedFade = rememberFade(state.backgroundFade, rememberedFade);
    state.cardSizeLibrary = normalizeCardSize(stored.card_size_library);
    state.cardSizeServer = normalizeCardSize(stored.card_size_server);
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
  rememberedFade = rememberFade(state.backgroundFade, rememberedFade);
}

/** Slider release: persists whatever the preview settled on. */
export async function commitBackgroundFade(value: number): Promise<void> {
  previewBackgroundFade(value);
  await api.setUiSettings(payload());
}

/**
 * Slider release on the blur control. There is no preview half: the sigma is
 * baked into a file the backend builds, so a distinct value means a distinct
 * variant — previewing every intermediate drag position would build dozens of
 * images nobody sees. `BackgroundArt` re-fetches through its own effect once
 * this lands.
 *
 * The memo is cleared BEFORE the store value changes, so the effect's
 * re-fetch — which the change triggers — can never read a path built at the
 * old sigma and since deleted by `remove_stale_variants`.
 */
export async function commitBackgroundBlur(value: number): Promise<void> {
  clearVariantMemo();
  state.backgroundBlur = clampBlur(value);
  await api.setUiSettings(payload());
}

/**
 * The size control on a grid toolbar. Applies immediately — the grid
 * re-flows on the next frame — then persists. A failed save leaves the
 * grid at the new size for this session and the config at the old one;
 * that is the same trade `setTheme` makes, and reverting a grid under the
 * user's cursor would be worse than a setting that did not stick.
 */
export async function setCardSize(view: 'library' | 'server', size: CardSize): Promise<void> {
  if (view === 'library') state.cardSizeLibrary = size;
  else state.cardSizeServer = size;
  await api.setUiSettings(payload());
}

/**
 * The Appearance page's on/off checkbox (design §10). Off persists fade 0;
 * on persists the remembered value, so toggling off and on returns the art
 * exactly as it was.
 */
export async function setBackgroundEnabled(enabled: boolean): Promise<void> {
  await commitBackgroundFade(fadeForToggle(enabled, rememberedFade));
}
