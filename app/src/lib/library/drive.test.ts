import { describe, expect, it } from 'vitest';
import type { DriveGame, DriveInfo, PackSummary } from '../api';
import {
  driveChips,
  driveCountLine,
  driveEmptyText,
  driveGameKey,
  driveGrid,
  installSelectionLabel,
  normalizeDriveSort,
  removedFromDriveText,
} from './drive';

const GB = 1024 ** 3;

const pack = (overrides: Partial<PackSummary> = {}): PackSummary => ({
  name: 'Couch kit', os: ['linux', 'windows'], game_count: 8, gridpack_bytes: 13.3 * GB, emulators: [], firmware: [],
  updated: '', rom_ids: [], ...overrides,
});

const drive = (overrides: Partial<DriveInfo> = {}): DriveInfo => ({
  key: '/m/T7', mount: '/m/T7', label: 'SAMSUNG T7', fs: 'exFAT', fat32: false, total_bytes: 1, free_bytes: 1,
  read_only: false, status: 'ready', problem: '', pack: pack(), ...overrides,
});

const game = (name: string, overrides: Partial<DriveGame> = {}): DriveGame => ({
  rom_id: 1, server: 's', name, platform: 'psx', platform_name: 'PlayStation', size: 1, files_max: 1, cover: '',
  installed: false, summary: '', region: '', languages: '', tags: '', revision: '', rating: '', companies: [],
  release_date: '', genres: [], game_modes: [], franchises: [], players: '', version: '', fanart: '', logo: '',
  screenshots: [], added: '2026-11-01T00:00:00Z', ...overrides,
});

describe('the drive header', () => {
  it('reads "8 games · 13.3 GB · on SAMSUNG T7"', () => {
    expect(driveCountLine(drive())).toBe('8 games · 13.3 GB · on SAMSUNG T7');
    expect(driveCountLine(drive({ label: '', pack: pack({ game_count: 1, gridpack_bytes: 0 }) }))).toBe('1 game · 0 B · on /m/T7');
  });

  it('lists each emulator once and the firmware, or none', () => {
    const d = drive({
      pack: pack({
        emulators: [
          { name: 'DuckStation', version: '1', os: 'linux' },
          { name: 'DuckStation', version: '1', os: 'windows' },
          { name: 'PCSX2', version: '2', os: 'linux' },
        ],
        firmware: ['PlayStation', 'PlayStation 2'],
      }),
    });
    expect(driveChips(d)).toEqual({ emulators: 'Emulators: DuckStation, PCSX2', firmware: 'Firmware: PlayStation, PlayStation 2' });
    expect(driveChips(drive())).toEqual({ emulators: 'Emulators: none', firmware: 'Firmware: none' });
  });
});

describe('the drive grid', () => {
  const games = [
    game('Moss & Marrow', { platform_name: 'SNES', size: 5, added: '2026-11-03T00:00:00Z' }),
    game('Cinder Peaks', { size: 9, added: '2026-11-01T00:00:00Z' }),
    game('Velvet Static', { size: 1, added: '2026-11-05T00:00:00Z' }),
  ];
  const names = (list: DriveGame[]) => list.map((g) => g.name);

  it('sorts by title, platform, recently backed up and size', () => {
    expect(names(driveGrid(games, '', 'title'))).toEqual(['Cinder Peaks', 'Moss & Marrow', 'Velvet Static']);
    expect(names(driveGrid(games, '', 'platform'))).toEqual(['Cinder Peaks', 'Velvet Static', 'Moss & Marrow']);
    expect(names(driveGrid(games, '', 'added'))).toEqual(['Velvet Static', 'Moss & Marrow', 'Cinder Peaks']);
    expect(names(driveGrid(games, '', 'size'))).toEqual(['Cinder Peaks', 'Moss & Marrow', 'Velvet Static']);
  });

  it('searches by title and never reorders its input', () => {
    expect(names(driveGrid(games, 'PEAK', 'title'))).toEqual(['Cinder Peaks']);
    expect(names(games)).toEqual(['Moss & Marrow', 'Cinder Peaks', 'Velvet Static']);
  });

  it('says why it is empty', () => {
    expect(driveEmptyText('')).toBe('Nothing on this drive yet. Back up games from Server or Library.');
    expect(driveEmptyText(' zz ')).toBe('No games on this drive match “zz”');
  });

  it('keys a game by its server and rom id', () => {
    expect(driveGameKey({ server: 'a', rom_id: 1 })).not.toBe(driveGameKey({ server: 'b', rom_id: 1 }));
  });

  it('falls back to Title for an unknown sort', () => {
    expect(normalizeDriveSort('played')).toBe('title');
    expect(normalizeDriveSort('size')).toBe('size');
  });
});

describe('select mode on a drive', () => {
  it('labels the install button with the count', () => {
    expect(installSelectionLabel(0)).toBe('Install');
    expect(installSelectionLabel(1)).toBe('Install 1 game');
    expect(installSelectionLabel(3)).toBe('Install 3 games');
  });

  it('words the removal card', () => {
    expect(removedFromDriveText(['Cinder Peaks'], 'Couch kit')).toBe('Removed Cinder Peaks from Couch kit');
    expect(removedFromDriveText(['a', 'b'], 'Couch kit')).toBe('Removed 2 games from Couch kit');
  });
});
