// A RomM platform's header (checklist items 197–205, moved under Library ›
// RomM; TVRommStorefront README § RomM › a platform; tvmode.js `srvPaint`,
// tvromm.js `rPaintPlatform`). Pure, over the desktop's chip rules
// (server/header.ts) and RomM's shared words (romm/words.ts).
import { emulatorChipLabel, firmwareChipLabel, firmwareChipLinks, showsChips, type FirmwareChipState } from '../../../../server/header';
import { NO_EMULATOR_MARKER } from '../../../../emulators/defaults';
import { rommPlatformCountsLine } from '../../../../romm/words';

/**
 * The counts line (decision 14): RomM's games, how many of them are
 * installed, and the installed games only on this computer, in desktop's
 * words ("3 games on RomM · 1 installed · 1 only on this computer"); while a
 * search narrows the grid, "2 of 4 games match" over everything it shows.
 */
export function platformCount(c: { onRomm: number; installed: number; onlyHere: number }, shown: number, isSearching: boolean): string {
  if (isSearching) {
    const total = c.onRomm + c.onlyHere;
    return `${shown} of ${total === 1 ? '1 game' : `${total} games`} match`;
  }
  return rommPlatformCountsLine(c.onRomm, c.installed, c.onlyHere);
}

export type EmulatorChip = { label: string; name: string | null; warn: boolean };
export type FirmwareChip = { label: string; focusable: boolean };

/**
 * The emulator and firmware chips, or null on a native platform (Windows,
 * Linux) and the Emulators shelf, which have neither (item 201). The
 * emulator chip reads "Emulator: <name>", or "No default emulator" in the
 * warning style (198); the firmware chip opens the system page unless the
 * server has no firmware, when it is plain (199).
 */
export function headerChips(
  platformLabel: string,
  platformSlug: string,
  defaultEmulator: string,
  firmware: FirmwareChipState,
): { emu: EmulatorChip; fw: FirmwareChip } | null {
  if (!showsChips(platformLabel, platformSlug)) return null;
  const name = defaultEmulator.trim();
  const has = name !== '' && name !== NO_EMULATOR_MARKER;
  return {
    emu: { label: emulatorChipLabel(name), name: has ? name : null, warn: !has },
    fw: { label: firmwareChipLabel(firmware), focusable: firmwareChipLinks(firmware) },
  };
}

/** The header's Backup button (item 202). */
export function backupAllLabel(total: number): string {
  return `Backup all ${total}…`;
}

/** Y's keyboard title on a platform (item 204). */
export function searchTitle(platformLabel: string): string {
  return `Search ${platformLabel}`;
}

/** Whether a query narrows the grid: a blank one is no filter (consultant
 *  ruling, the same rule as the Library's). */
export function searching(query: string): boolean {
  return query.trim() !== '';
}

/** A search with no match (item 205). */
export function noMatch(query: string): string {
  return `No games match “${query.trim()}”`;
}

/** Titles containing `query`, case-insensitively (tvmode.js `srvPaint`). */
export function matchesTitle(title: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  return q === '' || title.toLowerCase().includes(q);
}
