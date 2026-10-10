// Pure helpers for the Details Achievements tab. The data is RomM's: the
// ROM's RA achievement list joined with the account's RA progress
// (`get_achievements`, grid_core::romm::achievements).
import type { AchievementsView } from '../api';

/** Shown when the RomM account has no RA username, so progress is unknown. */
export const PROGRESS_HINT = 'Set your RetroAchievements username in Settings to see your progress.';

function count(n: number, singular: string, plural: string): string {
  return `${n} ${n === 1 ? singular : plural}`;
}

/** The summary line above the rows. */
export function summaryLine(view: AchievementsView): string {
  const s = view.summary;
  if (!view.progress_known) {
    return `${count(s.total, 'achievement', 'achievements')} · ${count(s.points_total, 'point', 'points')}`;
  }
  const parts = [`${s.earned} of ${s.total} unlocked`, `${s.points_earned} of ${s.points_total} points`];
  if (s.hardcore > 0) parts.push(`${s.hardcore} hardcore`);
  return parts.join(' · ');
}

export function showsProgressHint(view: AchievementsView): boolean {
  return !view.progress_known;
}

/** The calendar date of a RomM unlock timestamp (`2024-03-02 10:00:00`). */
export function unlockDate(raw: string | null): string {
  if (!raw) return '';
  const match = /^(\d{4}-\d{2}-\d{2})[ T]/.exec(raw.trim());
  return match ? match[1] : raw.trim();
}
