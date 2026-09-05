// The static per-emulator setup notes the reference renders under each
// Installed row (emulator_ui_mixin.py:712-720, 721-728, 749-757, 758-766,
// 767-775). Text is verbatim, including the arrows, the middle dot and the
// em dash.
//
// Matching: a case-folded SUBSTRING test of the token against the entry
// name. That is the second half of `_emulator_matches_tokens`
// (cloud_mixin.py:1349-1363, ported at
// crates/grid-core/src/autoconfig/mod.rs:232-239). The first half — the
// autoprofile `match_tokens` lookup — is not available here: the frontend's
// `ProfileSummary` carries only `{ name, args }` (api.ts:153), no token
// list. Every catalog install names its entry after its profile, so the
// substring test covers them; the same simplification is already in
// Emulators.svelte's `isRpcs3`.
//
// The reference's dynamic Eden notes (prod.keys and Switch firmware
// presence, emulator_ui_mixin.py:729-748) are deliberately NOT here: they
// need backend file probes that do not exist yet, and are deferred by the
// 2026-09-05 controller rulings.

export type EmulatorNote = { key: string; text: string };

/** Token → note, in the order the reference emits them. */
const NOTES: readonly EmulatorNote[] = [
  {
    key: 'azahar',
    text: 'Controller setup: Settings → Controls → Auto Map  ·  Press Esc to close emulator',
  },
  { key: 'eden', text: 'Controller setup: Controls → Configure → Map Controller' },
  {
    key: 'xemu',
    text: 'Controller setup: required to connect a controller first — layout is auto-detected',
  },
  {
    key: 'duckstation',
    text: 'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
  },
  { key: 'rpcs3', text: 'Controller setup: Configure controllers via Config → Pads' },
];

export function emulatorNotes(name: string): EmulatorNote[] {
  const haystack = name.trim().toLowerCase();
  if (haystack === '') return [];
  return NOTES.filter((note) => haystack.includes(note.key));
}
