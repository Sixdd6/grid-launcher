// The details popup's tabs (design §7, plus Achievements) and the session's
// remembered choice. Module scoped rather than stored in config: §7 says
// "last tab remembered per session", so it must survive closing and
// reopening the popup but not survive a restart.

export type DetailsTab = 'overview' | 'media' | 'saves' | 'files' | 'achievements';

export const DETAILS_TABS: readonly DetailsTab[] = [
  'overview',
  'media',
  'saves',
  'files',
  'achievements',
] as const;

export const DETAILS_TAB_LABELS: Record<DetailsTab, string> = {
  overview: 'Overview',
  media: 'Media',
  saves: 'Saves',
  files: 'Files',
  achievements: 'Achievements',
};

/** Design §11's new id for a tab button. */
export function tabTestId(tab: DetailsTab): string {
  return `details-tab-${tab}`;
}

export function isDetailsTab(value: string): value is DetailsTab {
  return (DETAILS_TABS as readonly string[]).includes(value);
}

/**
 * The tabs this game shows. Achievements appears only when the ROM detail
 * lists at least one achievement (`RomDetail.achievement_count`): many
 * RA-linked ROMs have an empty list. `null`/`undefined` = detail not loaded.
 */
export function visibleTabs(achievementCount: number | null | undefined): DetailsTab[] {
  return DETAILS_TABS.filter((tab) => tab !== 'achievements' || (achievementCount ?? 0) > 0);
}

/** The tab to show: the chosen one when this game has it, else Overview. */
export function shownTab(chosen: DetailsTab, visible: readonly DetailsTab[]): DetailsTab {
  return visible.includes(chosen) ? chosen : 'overview';
}

let remembered: DetailsTab = 'overview';

export function rememberedTab(): DetailsTab {
  return remembered;
}

export function rememberTab(tab: DetailsTab): void {
  remembered = tab;
}

/** Test-only reset, so one spec's choice cannot leak into the next. */
export function resetRememberedTab(): void {
  remembered = 'overview';
}
