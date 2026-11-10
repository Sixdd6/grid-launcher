// Settings › Appearance in TV mode (checklist items 10, 12, 13, 234–240,
// 527; tvmode.js `paneRows('appearance')`). Open GRID on first (README §
// Consultant decisions); Theme is the one setting shared with the desktop,
// and here "Follow system" shows dark (ruling 22); card size, game labels
// and card display stay desktop-only, which an information row says.
import type { OpenOn } from '../../../../api';
import { OPEN_ON_CHOICES, openOnLabel } from '../../../../settings/openOn';
import { artVisible, BLUR_MAX, type ThemeChoice } from '../../../../theme';
import type { Row } from '../../../rows';
import { choiceRow, type OpenChoice } from './choice';

export type AppearanceState = { openOn: OpenOn; theme: ThemeChoice; fade: number; blur: number; hardwareAcceleration: boolean };

export type AppearanceActions = {
  open: OpenChoice;
  setOpenOn: (v: OpenOn) => void;
  setTheme: (v: ThemeChoice) => void;
  setArt: (on: boolean) => void;
  setFade: (v: number) => void;
  /** The stored blur (0–BLUR_MAX), not the percentage the row shows. */
  setBlur: (v: number) => void;
  setHardwareAcceleration: (on: boolean) => void;
};

export const THEME_OPTIONS = [
  ['system', 'Follow system'],
  ['dark', 'Dark'],
  ['light', 'Light'],
] as const;

/** The line under Theme: TV mode shows Follow system as dark (ruling 22). */
export const THEME_LINE = 'Follow system shows dark here';

export function fadeText(v: number): string {
  return v === 100 ? '100% · off' : `${v}%`;
}

/** The stored blur (0–BLUR_MAX) as the row's 0–100 percentage. */
export function blurPercent(blur: number): number {
  return Math.round((blur * 100) / BLUR_MAX);
}

/** The row's percentage back to the stored blur. */
export function blurFromPercent(pct: number): number {
  return Math.round((pct * BLUR_MAX) / 100);
}

export function appearanceRows(s: AppearanceState, a: AppearanceActions): Row[] {
  return [
    choiceRow(
      { key: 'open-on', label: 'Open GRID on', sub: 'Here and in desktop mode' },
      OPEN_ON_CHOICES.map((c) => [c, openOnLabel(c)] as const),
      s.openOn,
      (v) => a.setOpenOn(v as OpenOn),
      a.open,
    ),
    choiceRow({ key: 'theme', label: 'Theme', sub: THEME_LINE }, THEME_OPTIONS, s.theme, (v) => a.setTheme(v as ThemeChoice), a.open),
    {
      type: 'switch',
      key: 'art',
      label: 'Background art',
      sub: "The focused game's art, blurred, behind everything",
      on: artVisible(s.fade),
      set: a.setArt,
    },
    { type: 'slider', key: 'fade', label: 'Background fade', value: s.fade, min: 0, max: 100, step: 5, format: fadeText, set: a.setFade },
    {
      type: 'slider',
      key: 'blur',
      label: 'Background blur',
      value: blurPercent(s.blur),
      min: 0,
      max: 100,
      step: 10,
      format: (v) => `${v}%`,
      set: (v) => a.setBlur(blurFromPercent(v)),
    },
    {
      type: 'switch',
      key: 'hw',
      label: 'Hardware acceleration',
      sub: 'Renders on the graphics card. Applies after a restart.',
      on: s.hardwareAcceleration,
      set: a.setHardwareAcceleration,
    },
    { type: 'info', key: 'cards', label: 'Card size, game labels and card display', value: 'Desktop grids only' },
  ];
}
