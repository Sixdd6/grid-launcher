// Details for a game whose system has no emulator (ruling A, then the
// user's decision of 2026-11-10 for the desktop too): the line both Details
// show in place of the install reason, and what Install brings with the
// game. Pure: `noEmulatorCheck.svelte.ts` reads the Systems store, and the
// desktop Details and TV Details lay the line out.
import { shortName, type SystemRow } from '../systems/rows';

/** A system with no emulator: its label, the recommended emulator's short
 *  name (null with none) and whether that recommendation is installing now. */
export type NoEmulator = { system: string; rec: string | null; installing: boolean };

/** The Systems row a game's platform belongs to: by label, server name or slug. */
export function systemRowFor(rows: readonly SystemRow[], platformName: string, slug: string): SystemRow | undefined {
  const name = platformName.trim();
  return rows.find((r) => (name !== '' && (r.key === name || r.name === name)) || (slug !== '' && r.slug === slug));
}

/** Whether `row` (the game's system) has no emulator, and what Install
 *  brings. A system set to None on purpose is left alone. */
export function noEmulatorFor(row: SystemRow | undefined, installing: boolean): NoEmulator | null {
  if (!row || row.emulator !== '' || row.explicitNone) return null;
  return { system: row.label, rec: row.recommended ? shortName(row.recommended.name) : null, installing };
}

/** The line's bold head: "No emulator for <system>." */
export function noEmulatorHead(n: NoEmulator): string {
  return `No emulator for ${n.system}.`;
}

/** What Install does about it: "Install adds <rec> too.", "<rec> is
 *  installing." or "Add one in Settings › Systems." */
export function noEmulatorTail(n: NoEmulator): string {
  if (n.rec === null) return 'Add one in Settings › Systems.';
  return n.installing ? `${n.rec} is installing.` : `Install adds ${n.rec} too.`;
}

/** The whole line, as one string. */
export function noEmulatorText(n: NoEmulator): string {
  return `${noEmulatorHead(n)} ${noEmulatorTail(n)}`;
}

/** Whether Install also installs the recommendation. */
export function installsRecommended(n: NoEmulator | null): boolean {
  return n !== null && n.rec !== null && !n.installing;
}

/** The queued card's second line when Install brings the recommendation
 *  ("<rec> installs with it"); TV mode's cards show it. */
export function noEmulatorSub(n: NoEmulator | null): string | null {
  return installsRecommended(n) ? `${n!.rec} installs with it` : null;
}
