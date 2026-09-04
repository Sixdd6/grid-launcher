// The Server platform header's text (design §6). Pure; Task 6 adds the
// firmware chip's label to this module.

/** "42 games · 7 installed" — the header's counts line. */
export function platformCountsLine(romCount: number, installedCount: number): string {
  const games = romCount === 1 ? '1 game' : `${romCount} games`;
  return `${games} · ${installedCount} installed`;
}

/** The default-emulator chip's text. */
export function emulatorChipLabel(name: string): string {
  const trimmed = name.trim();
  return trimmed === '' ? 'No default emulator' : `Emulator: ${trimmed}`;
}
