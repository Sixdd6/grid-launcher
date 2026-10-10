import { describe, expect, it } from 'vitest';
import type { AchievementsView } from '../api';
import { PROGRESS_HINT, showsProgressHint, summaryLine, unlockDate } from './achievements';

function view(partial: Partial<AchievementsView['summary']>, progressKnown = true): AchievementsView {
  return {
    progress_known: progressKnown,
    summary: { total: 4, earned: 2, hardcore: 0, points_total: 90, points_earned: 55, ...partial },
    rows: [],
  };
}

describe('summaryLine', () => {
  it('states earned of total and points earned of total', () => {
    expect(summaryLine(view({}))).toBe('2 of 4 unlocked · 55 of 90 points');
  });

  it('adds the hardcore count only when there is one', () => {
    expect(summaryLine(view({ hardcore: 1 }))).toBe('2 of 4 unlocked · 55 of 90 points · 1 hardcore');
  });

  it('states only the totals when progress is unknown', () => {
    expect(summaryLine(view({ earned: 0, points_earned: 0 }, false))).toBe('4 achievements · 90 points');
  });

  it('uses the singular for one', () => {
    expect(summaryLine(view({ total: 1, earned: 0, points_total: 1, points_earned: 0 }, false))).toBe(
      '1 achievement · 1 point',
    );
  });
});

describe('the progress hint', () => {
  it('shows only when the RomM account has no RA username', () => {
    expect(showsProgressHint(view({}, false))).toBe(true);
    expect(showsProgressHint(view({}, true))).toBe(false);
    expect(PROGRESS_HINT).toBe(
      'Set your RetroAchievements username in Settings to see your progress.',
    );
  });
});

describe('unlockDate', () => {
  it('keeps the calendar date of a RomM timestamp', () => {
    expect(unlockDate('2024-03-02 10:00:00')).toBe('2024-03-02');
    expect(unlockDate('2024-03-02T10:00:00Z')).toBe('2024-03-02');
  });

  it('passes an unknown shape through and blanks null', () => {
    expect(unlockDate('March 2, 2024')).toBe('March 2, 2024');
    expect(unlockDate(null)).toBe('');
  });
});
