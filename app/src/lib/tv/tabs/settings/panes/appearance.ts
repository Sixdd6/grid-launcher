// Settings › Appearance in TV mode (checklist items 10, 12, 13, 234–240,
// 527; tvmode.js `paneRows('appearance')`). Open GRID on first (README §
// Consultant decisions). Theme and Game card display are shared with the
// desktop; here "Follow system" shows dark (ruling 22). Card size and Game
// labels are TV mode's own, stored apart from the desktop's (user decision
// 2026-11-14, which replaced the "More grid options · In desktop mode" row:
// TV flows never send the player to desktop mode).
import type { OpenOn } from '../../../../api';
import { CARD_DISPLAYS, cardDisplayLabel, type CardDisplay } from '../../../../cards/layout';
import { OPEN_ON_CHOICES, openOnLabel } from '../../../../settings/openOn';
import { artVisible, BLUR_MAX, type ThemeChoice } from '../../../../theme';
import {
  TV_CARD_LABEL_MODES,
  TV_CARD_SIZES,
  tvCardLabelsLabel,
  tvCardSizeLabel,
  type TvCardLabels,
  type TvCardSize,
} from '../../../gridSettings';
import type { Row } from '../../../rows';
import { choiceRow, type OpenChoice } from './choice';

export type AppearanceState = {
  openOn: OpenOn;
  theme: ThemeChoice;
  cardDisplay: CardDisplay;
  tvCardSize: TvCardSize;
  tvCardLabels: TvCardLabels;
  fade: number;
  blur: number;
  hardwareAcceleration: boolean;
};

export type AppearanceActions = {
  open: OpenChoice;
  setOpenOn: (v: OpenOn) => void;
  setTheme: (v: ThemeChoice) => void;
  setCardDisplay: (v: CardDisplay) => void;
  setTvCardSize: (v: TvCardSize) => void;
  setTvCardLabels: (v: TvCardLabels) => void;
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

/** The line under Theme while it is Follow system: TV mode shows that as
 *  dark (ruling 22). Dark and Light apply as chosen, so they have no line. */
export const THEME_LINE = 'Dark in TV mode';

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
    choiceRow({ key: 'theme', label: 'Theme', sub: s.theme === 'system' ? THEME_LINE : undefined }, THEME_OPTIONS, s.theme, (v) => a.setTheme(v as ThemeChoice), a.open),
    choiceRow(
      { key: 'card-display', label: 'Game card display', sub: 'Here and in desktop mode' },
      CARD_DISPLAYS.map((d) => [d, cardDisplayLabel(d)] as const),
      s.cardDisplay,
      (v) => a.setCardDisplay(v as CardDisplay),
      a.open,
    ),
    choiceRow(
      { key: 'tv-card-size', label: 'Card size', sub: 'Every grid in TV mode' },
      TV_CARD_SIZES.map((v) => [v, tvCardSizeLabel(v)] as const),
      s.tvCardSize,
      (v) => a.setTvCardSize(v as TvCardSize),
      a.open,
    ),
    choiceRow(
      { key: 'tv-card-labels', label: 'Game labels', sub: 'The title under each cover' },
      TV_CARD_LABEL_MODES.map((v) => [v, tvCardLabelsLabel(v)] as const),
      s.tvCardLabels,
      (v) => a.setTvCardLabels(v as TvCardLabels),
      a.open,
    ),
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
      sub: 'Uses the graphics card. Restart to apply.',
      on: s.hardwareAcceleration,
      set: a.setHardwareAcceleration,
    },
  ];
}
