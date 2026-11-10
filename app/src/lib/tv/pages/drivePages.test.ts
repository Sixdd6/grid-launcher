import { describe, expect, it, vi } from 'vitest';
import type { DriveInfo, PackSummary } from '../../api';
import { drivePageRows, drivePageSub, flipOs, osOffConfirm, removeFilesWords, setupMain, setupRows, setupStart, spaceLine } from './drivePages';

const GB = 1024 ** 3;

function pack(overrides: Partial<PackSummary> = {}): PackSummary {
  return { name: 'Couch kit', os: ['linux', 'windows'], game_count: 8, gridpack_bytes: 100 * GB, emulators: [{ name: 'PCSX2', version: 'v2', os: 'linux' }, { name: 'PCSX2', version: 'v2', os: 'windows' }], firmware: ['PlayStation 2'], updated: '', rom_ids: [], ...overrides };
}

function drive(overrides: Partial<DriveInfo> = {}): DriveInfo {
  return { key: '/m/T7', mount: '/m/T7', label: 'SAMSUNG T7', fs: 'exFAT', fat32: false, total_bytes: 1000 * GB, free_bytes: 60 * GB, read_only: false, status: 'ready', problem: '', pack: pack(), ...overrides };
}

const acts = () => ({ changeOs: vi.fn(), showInLibrary: vi.fn(), openFolder: vi.fn(), rename: vi.fn(), removeFiles: vi.fn() });

describe('the drive page (items 499–509)', () => {
  it('heads with the label, file system, size and status', () => {
    expect(drivePageSub(drive())).toBe('SAMSUNG T7 · exFAT · 1000.0 GB · Ready for backups');
  });

  it('lists its rows in order', () => {
    const rows = drivePageRows(drive(), acts());
    expect(rows.map((r) => r.key)).toEqual(['dp-os', 'dp-space', 'dp-games', 'dp-emus', 'dp-fw', 'dp-last', 'dp-folder', 'dp-show', 'dp-open', 'dp-rename', 'dp-remove']);
    expect(rows[0]).toMatchObject({ kind: 'row', label: 'For Linux and Windows', value: 'Change…', al: 'Change' });
    expect(rows.find((r) => r.key === 'dp-emus')).toMatchObject({ value: 'PCSX2' });
    expect(rows.find((r) => r.key === 'dp-fw')).toMatchObject({ value: '1 system' });
    expect(rows.find((r) => r.key === 'dp-folder')).toMatchObject({ value: '/m/T7/gridpack' });
    expect(rows.find((r) => r.key === 'dp-remove')).toMatchObject({ danger: true });
  });

  it('splits the space into other files, GRID and free (501)', () => {
    expect(spaceLine(drive())).toBe('Other files 840 GB · GRID 100 GB · 60 GB free');
  });

  it('a read-only drive can’t be changed', () => {
    const keys = drivePageRows(drive({ status: 'read_only', read_only: true }), acts()).map((r) => r.key);
    expect(keys).not.toContain('dp-rename');
    expect(keys).not.toContain('dp-remove');
  });

  it('Remove GRID files asks twice and says what it freed (508–509)', () => {
    const w = removeFilesWords('Couch kit', 'SAMSUNG T7', 100 * GB);
    expect(w.first.title).toBe('Remove GRID files from Couch kit?');
    expect(w.first.ok).toBe('Continue');
    expect(w.second).toEqual({ title: 'Delete 100 GB from SAMSUNG T7?', body: 'This can’t be undone.', ok: 'Remove GRID files' });
    expect(w.freed).toBe('100 GB freed');
  });
});

describe('Computers this pack is for (items 502–504; ruling 9)', () => {
  it('refuses to turn off the last computer', () => {
    expect(flipOs(['linux'], 'linux')).toEqual({ refused: 'At least one computer stays ticked' });
    expect(flipOs(['linux'], 'windows')).toEqual({ next: ['linux', 'windows'] });
    expect(flipOs(['windows', 'linux'], 'windows')).toEqual({ next: ['linux'] });
  });

  it('asks before deleting a computer’s builds', () => {
    expect(osOffConfirm('windows', 'Couch kit')).toEqual({
      title: 'Remove the Windows builds from Couch kit?',
      body: 'The Windows builds of the emulators on Couch kit are deleted from the drive. Games and firmware stay.',
      ok: 'Remove Windows builds',
    });
  });
});

describe('Set up for GRID (items 510–513; ruling 14)', () => {
  it('starts with the label and this computer’s OS', () => {
    expect(setupStart(drive(), 'linux', 'SAMSUNG T7')).toEqual({ name: 'SAMSUNG T7', os: ['linux'] });
    expect(setupStart(drive(), 'windows', 'X').os).toEqual(['windows']);
  });

  it('warns on FAT32 and notes the gridpack folder', () => {
    const fat = setupRows(drive({ fs: 'FAT32', fat32: true, pack: null, status: 'not_set_up' }), { name: 'X', os: ['linux'] }, { editName: vi.fn(), flip: vi.fn() });
    expect(fat.map((r) => r.key)).toEqual(['su-name', 'su-h-os', 'su-linux', 'su-windows', 'su-fat32', 'su-note']);
    const ex = setupRows(drive({ pack: null, status: 'not_set_up' }), { name: 'X', os: ['linux'] }, { editName: vi.fn(), flip: vi.fn() });
    expect(ex.map((r) => r.key)).not.toContain('su-fat32');
  });

  it('Set up for GRID is the main button, disabled with the reason', () => {
    expect(setupMain({ name: 'Kit', os: ['linux'] }, false, vi.fn())).toMatchObject({ label: 'Set up for GRID', disabled: false, hint: 'Set up' });
    expect(setupMain({ name: ' ', os: ['linux'] }, false, vi.fn())).toMatchObject({ disabled: true, why: 'Give the drive a name' });
  });
});
