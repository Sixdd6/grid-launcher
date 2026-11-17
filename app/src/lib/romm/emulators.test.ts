import { describe, expect, it } from 'vitest';
import type { DownloadEntry, RommEmulator, RommEmulators, RommEmulatorsView, RommOtherFile } from '../api';
import {
  catalogCopyLine,
  downloadingLine,
  emulatorInstalledCard,
  emulatorMatches,
  emulatorRowState,
  emulatorUninstallConfirm,
  emulatorsCount,
  emulatorsCountsLine,
  emulatorsMatchLine,
  emulatorsRailRow,
  installedCount,
  latestEntry,
  nothingHereText,
  otherBuildsSummary,
  otherFileInstalledCard,
  otherFileLine,
  otherFileMatches,
  otherFileRowState,
  otherFileSourceId,
  otherReasonText,
  otherWhyText,
  tvEmulatorLine,
  tvOtherBuildWhy,
  tvWrongMachineLine,
  wrongMachineLine,
  type EmulatorsLoad,
} from './emulators';
import { rommRailGroup, rommSelectionFallback } from './rail';
import { platform, rommStatus } from './fixtures.test-util';

const MB = 1024 * 1024;

function emu(o: Partial<RommEmulator> = {}): RommEmulator {
  return {
    source_id: 'romm:ps2/pcsx2 (playstation 2)',
    name: 'PCSX2',
    profile_name: 'PCSX2 (Playstation 2)',
    systems: 'PlayStation 2',
    many_systems: false,
    system_slug: 'ps2',
    version: '2.0.2',
    size_bytes: 38 * MB,
    file_name: 'PCSX2 (v2.0.2) (arch-linux-x64).AppImage',
    folder: 'ps2',
    rom_id: 1,
    file_id: 11,
    entry: null,
    installed_version: null,
    update: false,
    catalog_copy: false,
    install_as: 'PCSX2 (Playstation 2)',
    ...o,
  };
}

function file(o: Partial<RommOtherFile> = {}): RommOtherFile {
  return {
    rom_id: 3,
    file_id: 31,
    file_name: 'xemu-v0.8.5-x86_64.AppImage',
    folder: 'xbox',
    size_bytes: 18 * MB,
    why: { kind: 'not_named' },
    installed: false,
    ...o,
  };
}

function view(o: Partial<RommEmulatorsView> = {}): RommEmulatorsView {
  return {
    host: 'Linux x64',
    emulators: [emu(), emu({ source_id: 'romm:pico/pico-8', name: 'Pico-8', entry: 'Pico-8' })],
    other_files: [file(), file({ file_id: 32, installed: true })],
    other_builds: [{ file_name: 'PCSX2 (v2.0.2) (arch-windows-x64).zip', folder: 'ps2', size_bytes: 31 * MB, reason: { kind: 'for', value: 'Windows x64' } }],
    ...o,
  };
}

function ready(v: RommEmulatorsView | null = view(), o: Partial<RommEmulators> = {}): EmulatorsLoad {
  return { phase: 'ready', data: { present: true, platform_id: 4, file_count: 5, view: v, ...o } };
}

function entry(o: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1, job: 'emulator', kind: 'emulator', rom_id: 1, source_id: 'romm:ps2/pcsx2 (playstation 2)', title: 'PCSX2 (Playstation 2)',
    platform: 'Emulator', status: 'queued', downloaded_bytes: 0, total_bytes: 0, speed_bps: 0, install_processed_bytes: 0,
    install_total_bytes: 0, error: '', detail: '', drive_id: '', drive_name: '', drive_label: '', batch: 0, phase: 'none', failure: null,
    ...o,
  };
}

const platforms = [
  platform({ id: 1, slug: 'ngc', name: 'GameCube', display_name: 'GameCube', rom_count: 2 }),
  platform({ id: 4, slug: 'emulators', name: 'Emulators', display_name: 'Emulators', rom_count: 3 }),
  platform({ id: 2, slug: 'snes', display_name: 'Super Nintendo Entertainment System', rom_count: 4 }),
];

describe('the Emulators rail row', () => {
  it('shows sorted by label among the platform rows, with what this computer can install as its count', () => {
    const g = rommRailGroup(rommStatus(), platforms, [], ready());
    expect(g?.rows.map((r) => r.key)).toEqual(['romm:installed', 'platform:emulators', 'platform:ngc', 'platform:snes']);
    expect(g?.rows[1]).toMatchObject({
      kind: 'emulators',
      label: 'Emulators',
      count: 4,
      testId: 'library-rail-platform-emulators',
      countTestId: 'library-rail-count-platform-emulators',
    });
  });

  it('shows without a count while its list loads or failed', () => {
    expect(emulatorsRailRow(platforms, { phase: 'loading' })).toMatchObject({ count: null, countTestId: null });
    expect(emulatorsRailRow(platforms, { phase: 'error', message: 'x' })).toMatchObject({ count: null });
  });

  it('counts 0 when nothing is for this computer', () => {
    expect(emulatorsRailRow(platforms, ready(view({ emulators: [], other_files: [] })))?.count).toBe(0);
    expect(emulatorsRailRow(platforms, ready(null))?.count).toBe(0);
  });

  it('is hidden with no Emulators platform, an empty one, no files, or before a load', () => {
    expect(emulatorsRailRow(platforms.filter((p) => p.id !== 4), ready())).toBeNull();
    expect(emulatorsRailRow([platform({ id: 4, slug: 'emulators', name: 'Emulators', display_name: 'Emulators', rom_count: 0 })], ready())).toBeNull();
    expect(emulatorsRailRow(platforms, ready(view(), { file_count: 0 }))).toBeNull();
    expect(emulatorsRailRow(platforms, ready(null, { present: false }))).toBeNull();
    expect(emulatorsRailRow(platforms, null)).toBeNull();
  });

  it.each(['unreachable', 'expired', 'not_connected'] as const)('is hidden while RomM is %s', (connection) => {
    const g = rommRailGroup(rommStatus({ connection }), platforms, [], ready());
    expect(g?.rows.some((r) => r.kind === 'emulators') ?? false).toBe(false);
  });

  it('is hidden while connecting at startup', () => {
    const g = rommRailGroup(rommStatus({ connection: 'not_connected', busy: true }), null, [], ready());
    expect(g?.rows.some((r) => r.kind === 'emulators') ?? false).toBe(false);
  });

  it('a selected row that went away falls back to RomM › Installed', () => {
    const gone = rommRailGroup(rommStatus(), platforms, [], ready(null, { present: false }));
    expect(rommSelectionFallback('platform:emulators', gone)).toBe('romm:installed');
    const there = rommRailGroup(rommStatus(), platforms, [], ready());
    expect(rommSelectionFallback('platform:emulators', there)).toBe('platform:emulators');
  });
});

describe('counts', () => {
  it('counts emulators plus other files; installed counts any version and installed files', () => {
    expect(emulatorsCount(view())).toBe(4);
    expect(emulatorsCount(null)).toBe(0);
    expect(installedCount(view())).toBe(2);
  });

  it('words the counts line for each state', () => {
    expect(emulatorsCountsLine(ready(), 'romm.home.lan')).toBe('2 emulators · 2 other files · 2 installed');
    expect(emulatorsCountsLine({ phase: 'loading' }, 'romm.home.lan')).toBe('Loading…');
    expect(emulatorsCountsLine({ phase: 'error', message: '' }, 'romm.home.lan')).toBe('On romm.home.lan');
    expect(emulatorsCountsLine(ready(view({ emulators: [], other_files: [] })), 'h')).toBe('No builds for this computer');
    expect(emulatorsCountsLine(ready(view({ emulators: [emu()], other_files: [file()] })), 'h')).toBe('1 emulator · 1 other file · 0 installed');
    expect(emulatorsMatchLine(2, 8)).toBe('2 of 8 items match');
  });
});

describe('row states', () => {
  it('an emulator is Install, Installed or Update by its entry', () => {
    expect(emulatorRowState(emu(), [])).toEqual({ kind: 'none' });
    expect(emulatorRowState(emu({ entry: 'PCSX2 (Playstation 2)' }), [])).toEqual({ kind: 'installed' });
    expect(emulatorRowState(emu({ entry: 'PCSX2 (Playstation 2)', update: true }), [])).toEqual({ kind: 'update' });
  });

  it('its Downloads row decides while live or failed, the newest row winning', () => {
    const e = emu();
    expect(emulatorRowState(e, [entry({ status: 'queued' })])).toEqual({ kind: 'queued' });
    expect(emulatorRowState(e, [entry({ status: 'downloading', downloaded_bytes: 17 * MB, total_bytes: 38 * MB })])).toEqual({
      kind: 'downloading',
      pct: 45,
      done: 17 * MB,
      total: 38 * MB,
    });
    expect(emulatorRowState(e, [entry({ status: 'installing' })])).toEqual({ kind: 'installing' });
    expect(
      emulatorRowState(e, [
        entry({ status: 'failed', error: 'boom', failure: { code: 'wrong_machine', reason: 'Linux ARM64', base_title: '', base_gog_id: '', kept: false } }),
      ]),
    ).toEqual({ kind: 'failed', builtFor: 'Linux ARM64', message: 'boom' });
    expect(emulatorRowState(e, [entry({ status: 'failed', error: 'No space' })])).toEqual({ kind: 'failed', builtFor: null, message: 'No space' });
    // A finished or cancelled row leaves the view's own answer.
    expect(emulatorRowState(e, [entry({ status: 'completed' })])).toEqual({ kind: 'none' });
    expect(emulatorRowState(e, [entry({ status: 'cancelled' })])).toEqual({ kind: 'none' });
    // A Retry adds a newer row.
    expect(emulatorRowState(e, [entry({ id: 2, status: 'downloading' }), entry({ id: 1, status: 'failed' })]).kind).toBe('downloading');
    // Another emulator's row and a game row are not this one's.
    expect(emulatorRowState(e, [entry({ source_id: 'romm:pico/pico-8' }), entry({ job: 'game' })])).toEqual({ kind: 'none' });
  });

  it('an other file is keyed by its rom and file id', () => {
    const f = file();
    expect(otherFileSourceId(f)).toBe('romm-file:3/31');
    expect(otherFileRowState(f, [])).toEqual({ kind: 'none' });
    expect(otherFileRowState(file({ installed: true }), [])).toEqual({ kind: 'installed' });
    expect(otherFileRowState(f, [entry({ source_id: 'romm-file:3/31', status: 'installing' })])).toEqual({ kind: 'installing' });
    expect(latestEntry([entry({ source_id: 'romm-file:3/31' })], 'romm-file:3/32')).toBeNull();
  });
});

describe('search', () => {
  it('matches name, system, file name and folder', () => {
    expect(emulatorMatches(emu(), 'playstation')).toBe(true);
    expect(emulatorMatches(emu(), 'arch-linux')).toBe(true);
    expect(emulatorMatches(emu(), 'PS2')).toBe(true);
    expect(emulatorMatches(emu(), 'dolphin')).toBe(false);
    expect(otherFileMatches(file(), 'xbox')).toBe(true);
    expect(otherFileMatches(file(), '  ')).toBe(true);
    expect(otherFileMatches(file(), 'cemu')).toBe(false);
  });
});

describe('words', () => {
  it('words why a file is in Other files and why a build is not offered', () => {
    expect(otherWhyText({ kind: 'not_named' })).toBe('Not named for GRID');
    expect(otherWhyText({ kind: 'no_arch_tag' })).toBe('It has no arch tag');
    expect(otherWhyText({ kind: 'no_processor' })).toBe('Its arch tag has no processor');
    expect(otherWhyText({ kind: 'unknown_emulator', value: 'Ares Nightly' })).toBe('GRID doesn’t know an emulator called “Ares Nightly”');
    expect(otherWhyText({ kind: 'extension', value: '.rar' })).toBe('GRID doesn’t unpack .rar files');
    expect(otherFileLine(file())).toBe('xbox folder · 18 MB · Not named for GRID');
    expect(otherReasonText({ kind: 'for', value: 'macOS (universal)' })).toBe('For macOS (universal)');
    expect(otherReasonText({ kind: 'older', value: '2.0.2' })).toBe('Older than v2.0.2');
    expect(otherBuildsSummary(6)).toBe('6 other builds GRID doesn’t install on this computer');
    expect(nothingHereText(6, 'Linux x64')).toBe(
      'RomM’s Emulators platform has 6 builds, all for other systems or processors. GRID installs builds for Linux x64.',
    );
    expect(tvOtherBuildWhy({ kind: 'for', value: 'Windows x64' }, 'Linux x64')).toBe('For Windows x64. GRID installs builds for Linux x64 on this computer.');
  });

  it('words the row lines', () => {
    expect(downloadingLine(46, 17 * MB, 38 * MB)).toBe('Downloading · 46% · 17 of 38 MB');
    expect(catalogCopyLine(emu({ name: 'DuckStation', install_as: 'DuckStation (server)' }))).toBe(
      'GRID’s catalog copy of DuckStation is installed. This build installs beside it as “DuckStation (server)”.',
    );
    expect(wrongMachineLine('Linux ARM64', 'Linux x64')).toBe(
      'Couldn’t install. The program in this file is built for Linux ARM64, not Linux x64. Check the file’s arch tag on RomM. Nothing was set up.',
    );
    expect(tvWrongMachineLine('Linux ARM64', 'Linux x64')).toBe(
      'Couldn’t install: its program is built for Linux ARM64, not Linux x64. Check the file’s arch tag on RomM.',
    );
    expect(tvEmulatorLine(emu(), { kind: 'none' })).toBe('PlayStation 2 · v2.0.2 · 38 MB');
    expect(tvEmulatorLine(emu({ installed_version: '1.6.0' }), { kind: 'update' })).toBe('PlayStation 2 · installed v1.6.0 · v2.0.2 is on RomM');
  });

  it('words the confirms and the cards', () => {
    expect(emulatorUninstallConfirm(emu({ installed_version: '1.6.0' }))).toEqual({
      title: 'Uninstall PCSX2 v1.6.0?',
      body: 'GRID deletes it from the emulators folder in your library. Games and saves stay. PlayStation 2 games set to it can’t start until another emulator is picked.',
    });
    expect(emulatorUninstallConfirm(emu({ name: 'RetroArch', many_systems: true, installed_version: '1.19.1' })).body).toContain('Games set to RetroArch can’t start');
    expect(emulatorInstalledCard(emu({ name: 'Pico-8', version: '0.2.7a6', systems: 'Pico-8' }), false)).toEqual({
      text: 'Pico-8 v0.2.7a6 installed from RomM · set up for Pico-8',
      tvText: 'Pico-8 v0.2.7a6 installed from RomM',
      sub: 'Set up for Pico-8 in Settings › Systems',
    });
    expect(emulatorInstalledCard(emu({ name: 'RetroArch', many_systems: true }), false).text).toContain('set up for its systems');
    expect(emulatorInstalledCard(emu(), true).text).toBe('PCSX2 updated to v2.0.2');
    expect(otherFileInstalledCard(file()).text).toBe('xemu-v0.8.5-x86_64.AppImage installed in your library’s Emulators folder');
  });
});
