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
// presence, emulator_ui_mixin.py:729-748) live in `dynamicEmulatorNotes`
// below: they depend on backend file probes, so they take the
// `emulator_facts` answer as a parameter and stay pure here.

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

/** The `emulator_facts` answer for one entry (api.ts's `EmulatorFacts`). */
type EdenFacts = { eden_keys_present: boolean; eden_firmware_present: boolean };

/** The two advisory Eden notes (emulator_ui_mixin.py:729-748), verbatim and
 *  in reference order. Empty for a non-Eden name, for an Eden entry whose
 *  keys and firmware are both present, and while `facts` is still
 *  undefined (the probe answer has not arrived — say nothing rather than
 *  flash a warning that a moment later turns out to be wrong). */
export function dynamicEmulatorNotes(name: string, facts?: EdenFacts): EmulatorNote[] {
  if (!facts) return [];
  if (!name.trim().toLowerCase().includes('eden')) return [];
  const notes: EmulatorNote[] = [];
  if (!facts.eden_keys_present) {
    notes.push({
      key: 'eden-keys',
      text: 'Switch keys (prod.keys) must be placed in user/keys/ before playing games.',
    });
  }
  if (!facts.eden_firmware_present) {
    notes.push({
      key: 'eden-firmware',
      text: 'Switch firmware must be installed via Emulation → Install Firmware before playing games.',
    });
  }
  return notes;
}
