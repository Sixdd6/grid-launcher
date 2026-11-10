// Settings › Appearance › Open GRID on (TV mode README § Decided "Where GRID
// opens"): one setting, `[ui] open_on`, shared by the desktop and TV mode.
// Pure; the store persists and `shell.ts` `initialView` applies it.
import type { OpenOn, TvAssignment, TvPrompts } from '../api';

export const OPEN_ON_CHOICES = ['automatic', 'discover', 'library'] as const satisfies readonly OpenOn[];

const LABELS: Record<OpenOn, string> = {
  automatic: 'Automatic: Discover when connected, else Library',
  discover: 'Server › Discover',
  library: 'Library',
};

export const OPEN_ON_HINT = 'Here and in TV mode';

export function openOnLabel(choice: OpenOn): string {
  return LABELS[choice];
}

/** The stored value, or `automatic` for anything unrecognised. Matches the
 *  Rust `normalize_open_on`, including its case sensitivity. */
export function normalizeOpenOn(raw: string): OpenOn {
  const v = raw.trim();
  return v === 'discover' || v === 'library' ? v : 'automatic';
}

const PROMPTS: readonly TvPrompts[] = ['match', 'xbox', 'ps', 'nintendo', '8bitdo'];

/** Matches the Rust `normalize_tv_prompts`. */
export function normalizeTvPrompts(raw: string): TvPrompts {
  const v = raw.trim();
  return (PROMPTS as readonly string[]).includes(v) ? (v as TvPrompts) : 'match';
}

/** Matches the Rust normalisation of `tv_assignments`: unknown styles and
 *  empty keys are dropped. */
export function normalizeTvAssignments(raw: unknown): Record<string, TvAssignment> {
  const out: Record<string, TvAssignment> = {};
  if (raw === null || typeof raw !== 'object') return out;
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    const k = key.trim();
    const v = typeof value === 'string' ? value.trim() : '';
    if (k !== '' && (v === 'xbox' || v === 'nintendo')) out[k] = v;
  }
  return out;
}
