// Appearance settings: the config-backed half of `lib/theme.ts`. Module
// scoped, like `appUpdate.svelte.ts`, so the resolved theme survives Shell
// remounts and every view reads one source.
import { api } from '../api';
import type { OpenOn, TvAssignment, TvPrompts, UiSettings } from '../api';
import { clearVariantMemo } from '../backgroundPrefetch';
import { normalizeCardLabels, type CardLabels } from '../cards/labels';
import { normalizeCardDisplay, type CardDisplay } from '../cards/layout';
import { normalizeCardSize, type CardSize } from '../cards/size';
import { fadeForToggle, rememberFade } from '../settings/appearance';
import { normalizeOpenOn, normalizeTvAssignments, normalizeTvPrompts } from '../settings/openOn';
import {
  BLUR_DEFAULT,
  clampBlur,
  clampFade,
  easesThemeSwitch,
  FADE_DEFAULT,
  nextTheme,
  normalizeTheme,
  THEME_EASE_WINDOW_MS,
  resolveTheme,
  themeAttribute,
  themeFromStorageValue,
  THEME_STORAGE_KEY,
  type ResolvedTheme,
  type ThemeChoice,
} from '../theme';
import { tvTheme } from '../tv/theme';
import { clampVolume, debounced, VOLUME_DEFAULT, VOLUME_SAVE_MS } from '../details/videoVolume';

const state = $state<{
  theme: ThemeChoice;
  backgroundFade: number;
  backgroundBlur: number;
  prefersDark: boolean;
  cardSizeLibrary: UiSettings['card_size_library'];
  cardSizeServer: UiSettings['card_size_server'];
  cardLabels: CardLabels;
  cardDisplay: CardDisplay;
  hardwareAcceleration: boolean;
  openOn: OpenOn;
  tvPrompts: TvPrompts;
  tvAssignments: Record<string, TvAssignment>;
  tvWelcomed: boolean;
  videoVolume: number;
  /** The stored settings have been read (or the read failed and the
   *  defaults stand). The shell waits for it: Open GRID on decides the view
   *  it opens on. */
  loaded: boolean;
}>({
  theme: 'system',
  backgroundFade: FADE_DEFAULT,
  backgroundBlur: BLUR_DEFAULT,
  prefersDark: false,
  cardSizeLibrary: 'medium',
  cardSizeServer: 'medium',
  cardLabels: 'always',
  cardDisplay: 'rows',
  hardwareAcceleration: true,
  openOn: 'automatic',
  tvPrompts: 'match',
  tvAssignments: {},
  tvWelcomed: false,
  videoVolume: VOLUME_DEFAULT,
  loaded: false,
});

// What "background art on" restores (design §10): the last fade short of off
// this session saw — loaded from config or dragged on the slider. Module
// scoped like `state`, not persisted: a full fade in config means off, and
// the default is what a fresh "on" goes back to.
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
    card_labels: state.cardLabels,
    card_display: state.cardDisplay,
    hardware_acceleration: state.hardwareAcceleration,
    open_on: state.openOn,
    tv_prompts: state.tvPrompts,
    tv_assignments: { ...state.tvAssignments },
    tv_welcomed: state.tvWelcomed,
    video_volume: state.videoVolume,
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
  get cardLabels(): CardLabels {
    return state.cardLabels;
  },
  get cardDisplay(): CardDisplay {
    return state.cardDisplay;
  },
  get hardwareAcceleration(): boolean {
    return state.hardwareAcceleration;
  },
  get openOn(): OpenOn {
    return state.openOn;
  },
  get tvPrompts(): TvPrompts {
    return state.tvPrompts;
  },
  get tvAssignments(): Readonly<Record<string, TvAssignment>> {
    return state.tvAssignments;
  },
  get tvWelcomed(): boolean {
    return state.tvWelcomed;
  },
  /** The video players' own volume, 0–100. */
  get videoVolume(): number {
    return state.videoVolume;
  },
  get loaded(): boolean {
    return state.loaded;
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

// TV mode shows "Follow system" as dark (ruling 22); TvShell turns this on
// while it is mounted.
let tvThemeActive = false;

/** The single writer of `<html data-theme>`. */
function applyTheme(choice: ThemeChoice): void {
  const attribute = tvThemeActive ? tvTheme(choice) : themeAttribute(choice);
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
    state.cardLabels = normalizeCardLabels(stored.card_labels ?? '');
    state.cardDisplay = normalizeCardDisplay(stored.card_display ?? '');
    state.hardwareAcceleration = stored.hardware_acceleration ?? true;
    state.openOn = normalizeOpenOn(stored.open_on ?? '');
    state.tvPrompts = normalizeTvPrompts(stored.tv_prompts ?? '');
    state.tvAssignments = normalizeTvAssignments(stored.tv_assignments);
    state.tvWelcomed = stored.tv_welcomed === true;
    state.videoVolume = clampVolume(stored.video_volume ?? VOLUME_DEFAULT);
  } catch {
    // Defaults/mirror already in `state`.
  }
  state.loaded = true;
  if (state.theme !== hint) {
    applyTheme(state.theme);
    mirrorTheme(state.theme);
  }

  return () => media.removeEventListener('change', onChange);
}

let themingTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * Puts `theming` on `<html>` for `THEME_EASE_WINDOW_MS`, so the colours ease
 * across a user's switch (app.css) and nothing else ever animates them.
 * Skipped under `prefers-reduced-motion`: the switch is instant.
 */
function easeThemeSwitch(): void {
  const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
  const classes = document.documentElement.classList;
  if (!easesThemeSwitch(reduced) || !classes) return;
  classes.add('theming');
  clearTimeout(themingTimer);
  themingTimer = setTimeout(() => classes.remove('theming'), THEME_EASE_WINDOW_MS);
}

/**
 * A user's theme switch, from the Appearance select or the light / dark
 * toggle: eases the colours, applies immediately, then persists. The
 * localStorage mirror is written only once the save resolves, so a failed
 * write leaves the hint agreeing with `config.toml`.
 */
export async function setTheme(choice: ThemeChoice): Promise<void> {
  easeThemeSwitch();
  state.theme = choice;
  applyTheme(choice);
  await api.setUiSettings(payload());
  mirrorTheme(choice);
}

/**
 * The light / dark toggle: switches to the other mode as an explicit choice,
 * so Settings › Appearance › Theme reads the same afterwards.
 */
export function toggleTheme(): Promise<void> {
  return setTheme(nextTheme(resolveTheme(state.theme, state.prefersDark)));
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

/** The "Hardware acceleration" switch. Persisted only: the webview reads it
 *  once at startup, so it applies after a restart. */
export async function setHardwareAcceleration(on: boolean): Promise<void> {
  state.hardwareAcceleration = on;
  await api.setUiSettings(payload());
}

/** Open GRID on (Settings › Appearance, shared with TV mode). Takes effect
 *  the next time GRID opens; persisted only. */
export async function setOpenOn(choice: OpenOn): Promise<void> {
  state.openOn = choice;
  await api.setUiSettings(payload());
}

/** The "Game card display" select: applies at once, then persists. */
export async function setCardDisplay(display: CardDisplay): Promise<void> {
  state.cardDisplay = display;
  await api.setUiSettings(payload());
}

/** The "Game labels" select: applies at once (both grids re-flow), then
 *  persists, with the same trade as `setCardSize`. */
export async function setCardLabels(mode: CardLabels): Promise<void> {
  state.cardLabels = mode;
  await api.setUiSettings(payload());
}

/**
 * The Appearance page's on/off checkbox (design §10). Off persists a full
 * fade (`FADE_MAX`);
 * on persists the remembered value, so toggling off and on returns the art
 * exactly as it was.
 */
export async function setBackgroundEnabled(enabled: boolean): Promise<void> {
  await commitBackgroundFade(fadeForToggle(enabled, rememberedFade));
}

/** TvShell mounted (true) or unmounted (false): re-applies the theme with
 *  TV mode's rule for "Follow system", or the desktop's. */
export function setTvThemeActive(on: boolean): void {
  tvThemeActive = on;
  applyTheme(state.theme);
}

/** Settings › TV mode › Button prompts: applies at once (the prompts
 *  redraw), then persists. */
export async function setTvPrompts(choice: TvPrompts): Promise<void> {
  state.tvPrompts = choice;
  await api.setUiSettings(payload());
}

/** TV mode's welcome card was shown: it doesn't show by itself again. */
export async function setTvWelcomed(): Promise<void> {
  if (state.tvWelcomed) return;
  state.tvWelcomed = true;
  await api.setUiSettings(payload());
}

/** Settings › TV mode › Button assignments, for one controller model (its
 *  key): applies at once, then persists. */
export async function setTvAssignment(key: string, style: TvAssignment): Promise<void> {
  state.tvAssignments = { ...state.tvAssignments, [key]: style };
  await api.setUiSettings(payload());
}

// One write per settled change: LT / RT held down, or a volume slider
// dragged, sends a level every few frames.
const saveVolume = debounced(() => {
  void (async () => {
    try {
      await api.setUiSettings(payload());
    } catch {
      // The level still applies for this session.
    }
  })();
}, VOLUME_SAVE_MS);

/** A video player's volume changed (LT / RT in TV mode, the player's own
 *  controls on the desktop): applies at once, then persists about 500 ms
 *  after the last change. */
export function setVideoVolume(value: number): void {
  const v = clampVolume(value);
  if (v === state.videoVolume) return;
  state.videoVolume = v;
  saveVolume();
}
