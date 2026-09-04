import { describe, expect, it } from 'vitest';
import {
  EMULATOR_PAGES,
  emulatorPageLabel,
  emulatorRailEntries,
  formPlacement,
  pageAfterSave,
  safeEmulatorPage,
  SEARCH_PAGE,
  visibleEmulatorPages,
  type EmulatorPageCounts,
} from './pages';

const counts: EmulatorPageCounts = { installed: 2, catalog: 7, defaults: 3, compat: 1 };

describe('emulator pages', () => {
  it('lists the four categories of design §9, in order', () => {
    expect([...EMULATOR_PAGES]).toEqual(['installed', 'catalog', 'defaults', 'compat']);
  });

  it('labels every page', () => {
    expect(emulatorPageLabel('installed')).toBe('Installed');
    expect(emulatorPageLabel('catalog')).toBe('Add from catalog');
    expect(emulatorPageLabel('defaults')).toBe('Platform defaults');
    expect(emulatorPageLabel('compat')).toBe('Compat tools');
  });

  it('hides Compat tools on a Windows host (design §9)', () => {
    expect(visibleEmulatorPages(false)).toEqual(['installed', 'catalog', 'defaults', 'compat']);
    expect(visibleEmulatorPages(true)).toEqual(['installed', 'catalog', 'defaults']);
  });

  it('falls back to Installed when a hidden page is asked for', () => {
    expect(safeEmulatorPage('compat', true)).toBe('installed');
    expect(safeEmulatorPage('compat', false)).toBe('compat');
    expect(safeEmulatorPage('defaults', true)).toBe('defaults');
  });
});

describe('emulatorRailEntries', () => {
  it('builds one entry per visible page with the §11 ids and the counts', () => {
    const entries = emulatorRailEntries(counts, 'defaults', false);
    expect(entries.map((e) => e.key)).toEqual(['installed', 'catalog', 'defaults', 'compat']);
    expect(entries.map((e) => e.testId)).toEqual([
      'emu-nav-installed',
      'emu-nav-catalog',
      'emu-nav-defaults',
      'emu-nav-compat',
    ]);
    expect(entries.map((e) => e.countTestId)).toEqual([
      'emu-nav-count-installed',
      'emu-nav-count-catalog',
      'emu-nav-count-defaults',
      'emu-nav-count-compat',
    ]);
    expect(entries.map((e) => e.count)).toEqual([2, 7, 3, 1]);
    expect(entries.map((e) => e.selected)).toEqual([false, false, true, false]);
  });

  it('puts the section heading on the first entry only', () => {
    const entries = emulatorRailEntries(counts, 'installed', false);
    expect(entries[0].heading).toBe('EMULATORS');
    expect(entries.slice(1).every((e) => e.heading === undefined)).toBe(true);
  });

  it('omits the compat entry on Windows', () => {
    expect(emulatorRailEntries(counts, 'installed', true).map((e) => e.key)).toEqual([
      'installed',
      'catalog',
      'defaults',
    ]);
  });
});

describe('formPlacement', () => {
  it('renders the edit sheet only on Installed while an entry is being edited', () => {
    expect(formPlacement('installed', true, 'install')).toBe('sheet');
    expect(formPlacement('installed', false, 'install')).toBeNull();
    expect(formPlacement('defaults', true, 'install')).toBeNull();
  });

  it('renders the manual add form only on the catalog page under the Manual tab', () => {
    expect(formPlacement('catalog', false, 'manual')).toBe('manual');
    expect(formPlacement('catalog', false, 'install')).toBeNull();
    expect(formPlacement('installed', false, 'manual')).toBeNull();
  });

  it('an open edit never leaks onto the catalog page', () => {
    expect(formPlacement('catalog', true, 'install')).toBeNull();
    expect(formPlacement('catalog', true, 'manual')).toBe('manual');
  });
});

describe('pageAfterSave / SEARCH_PAGE', () => {
  it('lands every successful save on Installed', () => {
    expect(pageAfterSave('add')).toBe('installed');
    expect(pageAfterSave('edit')).toBe('installed');
  });

  it('Ctrl+F targets the catalog page, the only one with a search box', () => {
    expect(SEARCH_PAGE).toBe('catalog');
  });
});
