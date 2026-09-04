// The details popup's four tabs (design §7) and the session's remembered
// choice. Module scoped rather than stored in config: §7 says "last tab
// remembered per session", so it must survive closing and reopening the
// popup but not survive a restart.

export type DetailsTab = 'overview' | 'media' | 'saves' | 'files';

export const DETAILS_TABS: readonly DetailsTab[] = ['overview', 'media', 'saves', 'files'] as const;

export const DETAILS_TAB_LABELS: Record<DetailsTab, string> = {
  overview: 'Overview',
  media: 'Media',
  saves: 'Saves',
  files: 'Files',
};

/** Design §11's new id for a tab button. */
export function tabTestId(tab: DetailsTab): string {
  return `details-tab-${tab}`;
}

export function isDetailsTab(value: string): value is DetailsTab {
  return (DETAILS_TABS as readonly string[]).includes(value);
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
