// Settings › Appearance additions (design §10): "background art on/off" as
// a rule over the existing fade value (no separate config key: a full fade,
// `FADE_MAX`, IS off), and the two card-size defaults. Pure; the store
// persists.
import { artVisible, FADE_DEFAULT, FADE_MAX } from '../theme';

export function backgroundEnabled(fade: number): boolean {
  return artVisible(fade);
}

/** The value "on" goes back to: the last fade short of off seen this session. */
export function rememberFade(fade: number, remembered: number): number {
  return artVisible(fade) ? fade : remembered;
}

export function fadeForToggle(enabled: boolean, remembered: number): number {
  if (!enabled) return FADE_MAX;
  return artVisible(remembered) ? remembered : FADE_DEFAULT;
}

/** D-UI-9's card size, one row since the Server view went (RommStorefront
 *  README; plan 0.3): the RomM screens use the Library's size, and
 *  `card_size_server` stays in the config, unread. */
export const CARD_SIZE_VIEWS = [
  { view: 'library', label: 'Card size', testId: 'card-size-library' },
] as const satisfies readonly { view: 'library'; label: string; testId: string }[];
