// The settings-style row model (TVMode README § Tabs › Settings: "Rows are
// switches (A flips them), choices (A opens a list), sliders (left / right
// adjust while focused), text (A opens the keyboard) and actions"; tvmode.js
// `srow`, `srowAdj`). Pure; TvRows draws a list of these.

type Base = {
  /** Stable identity (`data-k`), unique on its screen. */
  key: string;
  label: string;
  sub?: string;
  danger?: boolean;
};

export type SwitchRow = Base & { type: 'switch'; on: boolean; set: (on: boolean) => void };
export type ChoiceRow = Base & {
  type: 'choice';
  value: string;
  options: readonly (readonly [value: string, label: string])[];
  /** A opens the list (P6's TvList); answered with the chosen value. */
  choose: () => void;
};
export type SliderRow = Base & {
  type: 'slider';
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  set: (v: number) => void;
};
export type TextRow = Base & { type: 'text'; value: string; mask?: boolean; edit: () => void };
export type ActionRow = Base & { type: 'action'; value?: string; run: () => void; disabled?: boolean; why?: string };
export type InfoRow = Base & { type: 'info'; value: string };

export type Row = SwitchRow | ChoiceRow | SliderRow | TextRow | ActionRow | InfoRow;

/** The A hint on a row (item 129). Info rows aren't focusable. */
export function rowA(row: Row): string | null {
  switch (row.type) {
    case 'switch':
      return 'Switch';
    case 'text':
      return 'Edit';
    case 'info':
      return null;
    default:
      return 'Select';
  }
}

export function sliderPercent(row: Pick<SliderRow, 'value' | 'min' | 'max'>): number {
  if (row.max <= row.min) return 0;
  return ((row.value - row.min) / (row.max - row.min)) * 100;
}

/** One Left (-1) or Right (+1) step on a slider, clamped to its range. */
export function sliderStep(row: Pick<SliderRow, 'value' | 'min' | 'max' | 'step'>, delta: -1 | 1): number {
  return Math.max(row.min, Math.min(row.max, row.value + delta * row.step));
}

/** What a choice row shows: the chosen option's label. */
export function choiceLabel(row: Pick<ChoiceRow, 'value' | 'options'>): string {
  return row.options.find(([v]) => v === row.value)?.[1] ?? '';
}

/** What a text row shows: dots for a masked value, "Not set" for none. */
export function textValue(row: Pick<TextRow, 'value' | 'mask'>): { text: string; unset: boolean } {
  if (row.value === '') return { text: 'Not set', unset: true };
  return { text: row.mask ? '••••••••' : row.value, unset: false };
}
