// Pure theme + background-fade resolution (design §4). No store and no DOM
// imports here: this module is the unit-testable half of the appearance
// settings, and `stores/uiSettings.svelte.ts` is the reactive half.

export type ThemeChoice = 'system' | 'dark' | 'light';
export type ResolvedTheme = 'dark' | 'light';

/** Design §3: the Appearance slider's range and its default. */
export const FADE_DEFAULT = 25;
export const FADE_MAX = 60;

const CHOICES: ThemeChoice[] = ['system', 'dark', 'light'];

/**
 * `ui.theme` is stored as a free string so an unknown value written by a
 * newer build round-trips through the config instead of failing the load
 * (grid-core's `UiSettings`). Anything this build does not recognize reads
 * back as "follow the OS".
 */
export function normalizeTheme(raw: string): ThemeChoice {
  const trimmed = raw.trim();
  return (CHOICES as string[]).includes(trimmed) ? (trimmed as ThemeChoice) : 'system';
}

export function resolveTheme(choice: ThemeChoice, prefersDark: boolean): ResolvedTheme {
  if (choice === 'dark' || choice === 'light') return choice;
  return prefersDark ? 'dark' : 'light';
}

/**
 * What belongs in `<html data-theme>`: `null` for "system", so the CSS
 * `prefers-color-scheme` media query is left to decide, and the explicit
 * theme otherwise. app.css keys its override blocks off this attribute.
 */
export function themeAttribute(choice: ThemeChoice): ResolvedTheme | null {
  return choice === 'system' ? null : choice;
}

export function clampFade(value: number): number {
  if (!Number.isFinite(value)) return FADE_DEFAULT;
  return Math.min(FADE_MAX, Math.max(0, Math.round(value)));
}

/**
 * localStorage key mirroring the last applied explicit theme, so the app
 * can paint the right scheme before the config round-trip finishes (index.html
 * reads the same key inline, before the bundle loads). The mirror is a hint
 * only — the config from `api.getUiSettings()` is still authoritative once it
 * arrives.
 */
export const THEME_STORAGE_KEY = 'grid.ui.theme';

/**
 * Maps a raw `localStorage.getItem(THEME_STORAGE_KEY)` result to the explicit
 * theme it names, or `null` when there is nothing worth pre-applying — the
 * value is missing, garbage, or "system" (the media query already renders
 * that case correctly with no attribute at all).
 */
export function themeFromStorageValue(raw: string | null): ResolvedTheme | null {
  return raw === 'dark' || raw === 'light' ? raw : null;
}
