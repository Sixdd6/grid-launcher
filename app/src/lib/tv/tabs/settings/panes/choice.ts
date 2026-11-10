// A Settings choice row and the list it opens (tvmode.js `srow`, `srowAct`:
// "choices open a list with a check on the current value"). Pure: the pane
// passes `open`, which TvSettings implements with TvList.
import type { ChoiceRow } from '../../../rows';

export type Options = readonly (readonly [value: string, label: string])[];

/** What the list shows: its title (the row's label), an optional line, each
 *  option with an optional muted line, the current one checked. */
export type ChoiceSpec = {
  title: string;
  sub?: string;
  options: Options;
  value: string;
  /** Muted lines beside options, by value. */
  subs?: Readonly<Record<string, string>>;
  pick: (value: string) => void;
};

export type OpenChoice = (spec: ChoiceSpec) => void;

export function choiceRow(
  o: { key: string; label: string; sub?: string; disabled?: boolean; why?: string; listSub?: string; subs?: Readonly<Record<string, string>> },
  options: Options,
  value: string,
  pick: (value: string) => void,
  open: OpenChoice,
): ChoiceRow {
  return {
    type: 'choice',
    key: o.key,
    label: o.label,
    ...(o.sub ? { sub: o.sub } : {}),
    ...(o.disabled ? { disabled: true, why: o.why } : {}),
    value,
    options,
    choose: () => open({ title: o.label, ...(o.listSub ? { sub: o.listSub } : {}), options, value, ...(o.subs ? { subs: o.subs } : {}), pick }),
  };
}
