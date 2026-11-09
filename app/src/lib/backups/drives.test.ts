import { describe, expect, it } from 'vitest';
import type { DriveInfo, DrivesSnapshot, PackSummary } from '../api';
import {
  driveByKey,
  drivesHoldingRom,
  EMPTY_SNAPSHOT,
  firstLibraryDriveKey,
  packDrives,
  readyDrives,
  serverKey,
} from './drives';

function pack(overrides: Partial<PackSummary> = {}): PackSummary {
  return {
    name: 'Couch kit',
    os: ['linux'],
    game_count: 0,
    gridpack_bytes: 0,
    emulators: [],
    firmware: [],
    updated: '2026-11-09T00:00:00Z',
    rom_ids: [],
    ...overrides,
  };
}

function drive(key: string, overrides: Partial<DriveInfo> = {}): DriveInfo {
  return {
    key,
    mount: key,
    label: key.toUpperCase(),
    fs: 'exFAT',
    fat32: false,
    total_bytes: 1000,
    free_bytes: 500,
    read_only: false,
    status: 'ready',
    problem: '',
    pack: pack(),
    ...overrides,
  };
}

function snap(connected: DriveInfo[]): DrivesSnapshot {
  return { connected, remembered: [] };
}

describe('serverKey', () => {
  it('mirrors grid-core server_key: host lowercased, port explicit, no trailing slash, no userinfo', () => {
    expect(serverKey('https://RomM.Example.test/')).toBe('romm.example.test:443');
    expect(serverKey('http://romm.example.test')).toBe('romm.example.test:80');
    expect(serverKey('https://user:pw@romm.example.test:8443/base/')).toBe('romm.example.test:8443/base');
  });
  it('treats an explicit default port as the same server', () => {
    expect(serverKey('https://h.test:443')).toBe(serverKey('https://h.test'));
  });
  it('falls back to the trimmed, lowercased text for a non-URL', () => {
    expect(serverKey(' Not A URL/ ')).toBe('not a url');
  });
});

describe('readyDrives / packDrives / driveByKey / firstLibraryDriveKey', () => {
  const ready = drive('/media/a');
  const readOnly = drive('/media/b', { status: 'read_only', read_only: true });
  const bare = drive('/media/c', { status: 'not_set_up', pack: null });
  const unreadable = drive('/media/d', { status: 'unreadable', problem: 'bad json', pack: null });
  const s = snap([readOnly, bare, ready, unreadable]);

  it('keeps only set-up, writable drives as backup targets', () => {
    expect(readyDrives(s).map((d) => d.key)).toEqual(['/media/a']);
  });
  it('lists every drive with a readable pack, read-only included, in scan order', () => {
    expect(packDrives(s).map((d) => d.key)).toEqual(['/media/b', '/media/a']);
  });
  it('finds a connected drive by key', () => {
    expect(driveByKey(s, '/media/c')?.label).toBe('/MEDIA/C');
    expect(driveByKey(s, '/media/z')).toBeNull();
  });
  it('opens the first ready drive, else the first read-only pack, else none', () => {
    expect(firstLibraryDriveKey(s)).toBe('/media/a');
    expect(firstLibraryDriveKey(snap([readOnly, bare]))).toBe('/media/b');
    expect(firstLibraryDriveKey(snap([bare, unreadable]))).toBeNull();
    expect(firstLibraryDriveKey(EMPTY_SNAPSHOT)).toBeNull();
  });
});

describe('drivesHoldingRom', () => {
  const kit = drive('/media/kit', {
    pack: pack({ rom_ids: [{ server: 'https://romm.example.test', rom_id: 7 }] }),
  });
  const other = drive('/media/other', {
    pack: pack({ name: 'Other', rom_ids: [{ server: 'https://elsewhere.test', rom_id: 7 }] }),
  });
  const ro = drive('/media/ro', {
    status: 'read_only',
    read_only: true,
    pack: pack({ rom_ids: [{ server: 'https://romm.example.test/', rom_id: 7 }] }),
  });
  const s = snap([kit, other, ro, drive('/media/blank', { status: 'not_set_up', pack: null })]);

  it('matches the rom id on the same server, comparing normalised servers', () => {
    expect(drivesHoldingRom(s, 'https://ROMM.example.test:443/', 7).map((d) => d.key)).toEqual(['/media/kit', '/media/ro']);
  });
  it('ignores the same rom id from another server', () => {
    expect(drivesHoldingRom(s, 'https://elsewhere.test', 7).map((d) => d.key)).toEqual(['/media/other']);
  });
  it('matches nothing for another rom id, or without a server', () => {
    expect(drivesHoldingRom(s, 'https://romm.example.test', 8)).toEqual([]);
    expect(drivesHoldingRom(s, '', 7)).toEqual([]);
  });
});
