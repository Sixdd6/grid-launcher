// Settings › TV mode (checklist items 48, 53, 70, 261–267; README § Decided
// "Button assignments", "Button prompts", "Entering and leaving";
// tvmode.js `paneRows('tv')`). Button assignments names the controller it
// applies to, the one pressed last, and shows its own choice or the style it
// starts on; a choice is kept for that controller only.
import type { TvAssignment, TvPrompts } from '../../../../api';
import type { Row } from '../../../rows';
import { choiceRow, type OpenChoice } from './choice';

export type TvPaneState = {
  prompts: TvPrompts;
  /** The controller in use: its display name, or null with none. */
  padName: string | null;
  /** Its button assignment (its own choice, or its default). */
  assignment: TvAssignment;
};

export type TvPaneActions = {
  open: OpenChoice;
  setPrompts: (v: TvPrompts) => void;
  setAssignment: (v: TvAssignment) => void;
  toDesktop: () => void;
  quit: () => void;
};

export const PROMPT_OPTIONS = [
  ['match', 'Match my controller'],
  ['xbox', 'Xbox'],
  ['ps', 'PlayStation'],
  ['nintendo', 'Nintendo'],
  ['8bitdo', '8BitDo (Pro series)'],
] as const;

export const ASSIGN_OPTIONS = [
  ['xbox', 'Xbox style'],
  ['nintendo', 'Nintendo style'],
] as const;

export const ASSIGN_SUBS: Record<TvAssignment, string> = {
  xbox: 'Bottom selects, right goes back',
  nintendo: 'Right selects, bottom goes back',
};

export const NO_CONTROLLER = 'None found · the keyboard works too';
/** A on Button assignments with no controller connected (consultant decision;
 *  checklist 532): the row is disabled and reads "Xbox style". */
export const ASSIGN_NEEDS_PAD = 'Connect a controller to choose its buttons. Each controller keeps its own choice.';

/** "For <controller>", or "For the controller in use" with none (item 262). */
export function assignLine(padName: string | null): string {
  return padName ? `For ${padName}` : 'For the controller in use';
}

export function tvRows(s: TvPaneState, a: TvPaneActions): Row[] {
  return [
    choiceRow(
      { key: 'prompts', label: 'Button prompts', sub: 'Which controller the on-screen buttons look like' },
      PROMPT_OPTIONS,
      s.prompts,
      (v) => a.setPrompts(v as TvPrompts),
      a.open,
    ),
    choiceRow(
      {
        key: 'assign',
        label: 'Button assignments',
        sub: assignLine(s.padName),
        listSub: s.padName ?? 'The controller in use',
        subs: ASSIGN_SUBS,
        ...(s.padName === null ? { disabled: true, why: ASSIGN_NEEDS_PAD } : {}),
      },
      ASSIGN_OPTIONS,
      s.padName === null ? 'xbox' : s.assignment,
      (v) => a.setAssignment(v as TvAssignment),
      a.open,
    ),
    { type: 'info', key: 'tv-controller', label: 'Controller', value: s.padName ?? NO_CONTROLLER },
    {
      type: 'action',
      key: 'tv-desktop',
      label: 'Switch to desktop mode',
      sub: 'Also in the Menu. To come back: the TV button in the desktop top bar, or start GRID with --tv',
      run: a.toDesktop,
    },
    { type: 'action', key: 'tv-quit', label: 'Quit GRID', run: a.quit },
  ];
}
