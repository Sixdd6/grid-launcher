// The one line the user sees after their Python settings are carried over.
// Pure, so the wording is pinned by vitest rather than by reading a toast.
import type { PythonImportReport } from './api';

function count(n: number, singular: string, plural: string): string {
  if (n === 0) return `no ${plural}`;
  return `${n} ${n === 1 ? singular : plural}`;
}

/**
 * The import toast. `skipped_games` is deliberately not shown: it is a
 * diagnostic for the log, and a user who never saw those rows in the old app
 * would not recognise the number.
 */
export function importToastText(report: PythonImportReport): string {
  const sentences = [
    `Imported ${count(report.emulators, 'emulator', 'emulators')} and ` +
      `${count(report.games, 'game', 'games')} from the previous version.`,
    'Enter your RomM token to reconnect.',
  ];
  if (report.retroachievements) sentences.push('Enter your RetroAchievements token as well.');
  return sentences.join(' ');
}
