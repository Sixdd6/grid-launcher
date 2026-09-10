// Settings › Appearance additions (design §10): "background art on/off" as
// a rule over the existing fade value (no separate config key: fade 0 IS
// off), and the two card-size defaults. Pure; the store persists.
import { FADE_DEFAULT } from '../theme';

export function backgroundEnabled(fade: number): boolean {
  return fade > 0;
}

/** The value "on" goes back to: the last non-zero fade seen this session. */
export function rememberFade(fade: number, remembered: number): number {
  return fade > 0 ? fade : remembered;
}

export function fadeForToggle(enabled: boolean, remembered: number): number {
  if (!enabled) return 0;
  return remembered > 0 ? remembered : FADE_DEFAULT;
}

/** D-UI-9: "Size control Small / Medium / Large per view, remembered". */
export const CARD_SIZE_VIEWS = [
  { view: 'library', label: 'Library cards', testId: 'card-size-library' },
  { view: 'server', label: 'Server cards', testId: 'card-size-server' },
] as const satisfies readonly { view: 'library' | 'server'; label: string; testId: string }[];
