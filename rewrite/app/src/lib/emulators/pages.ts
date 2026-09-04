// The Emulators view's category rail (design §9, D-UI-5): four pages, the
// rail entries with their §11 ids and counts, where the manual form renders,
// and where a save lands. Pure — Emulators.svelte owns the state and the
// markup, this module owns the rules.

export const EMULATOR_PAGES = ['installed', 'catalog', 'defaults', 'compat'] as const;
export type EmulatorPage = (typeof EMULATOR_PAGES)[number];

const LABELS: Record<EmulatorPage, string> = {
  installed: 'Installed',
  catalog: 'Add from catalog',
  defaults: 'Platform defaults',
  compat: 'Compat tools',
};

export function emulatorPageLabel(page: EmulatorPage): string {
  return LABELS[page];
}

/** Design §9: "Compat tools (hidden on Windows)". */
export function visibleEmulatorPages(windowsHost: boolean): EmulatorPage[] {
  return EMULATOR_PAGES.filter((p) => p !== 'compat' || !windowsHost);
}

/** A page that is not on this host's rail falls back to the first one. */
export function safeEmulatorPage(page: EmulatorPage, windowsHost: boolean): EmulatorPage {
  return visibleEmulatorPages(windowsHost).includes(page) ? page : 'installed';
}

export type EmulatorPageCounts = Record<EmulatorPage, number>;

/** One rail row. The shape `RailPane.svelte` renders, minus its generics. */
export type EmulatorRailEntry = {
  key: EmulatorPage;
  testId: string;
  countTestId: string;
  label: string;
  count: number;
  selected: boolean;
  heading?: string;
};

export function emulatorRailEntries(
  counts: EmulatorPageCounts,
  selected: EmulatorPage,
  windowsHost: boolean,
): EmulatorRailEntry[] {
  return visibleEmulatorPages(windowsHost).map((page, i) => ({
    key: page,
    testId: `emu-nav-${page}`,
    countTestId: `emu-nav-count-${page}`,
    label: LABELS[page],
    count: counts[page],
    selected: page === selected,
    ...(i === 0 ? { heading: 'EMULATORS' } : {}),
  }));
}

/** The catalog pane's two tabs: the catalog rows, or the manual form. */
export type AddTab = 'install' | 'manual';

/**
 * Where the one `EmulatorForm` renders: as the Installed pane's edit sheet
 * (design §9: "Edit opens the manual form inline as a sheet on the right of
 * the pane"), as the catalog pane's Manual tab, or nowhere.
 *
 * The current page is deliberately not an input: each host pane is already
 * hidden when it is not selected, so keying the placement on the page would
 * unmount a half-filled form on any rail click and re-seed it on the way
 * back (final review P5-2). An open edit sheet wins over the Manual tab.
 */
export type FormPlacement = 'sheet' | 'manual' | null;

export function formPlacement(editing: boolean, addTab: AddTab): FormPlacement {
  if (editing) return 'sheet';
  if (addTab === 'manual') return 'manual';
  return null;
}

/** A successful save shows the row it produced: both modes land on Installed. */
export function pageAfterSave(mode: 'add' | 'edit'): EmulatorPage {
  void mode;
  return 'installed';
}

/** Design §3: Ctrl+F focuses the view's search; only this page has one. */
export const SEARCH_PAGE: EmulatorPage = 'catalog';
