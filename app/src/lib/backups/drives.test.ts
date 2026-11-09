import { describe, expect, it } from 'vitest';
import type { DriveInfo, DrivesSnapshot, PackSummary, RememberedDrive } from '../api';
import {
  addedOses,
  defaultPackName,
  driveByKey,
  driveCardModel,
  drivesHoldingRom,
  EMPTY_SNAPSHOT,
  emulatorNames,
  FAT32_WARNING,
  firstLibraryDriveKey,
  formatDay,
  gridpackPath,
  hostPackOs,
  lastSeenText,
  osChanged,
  osChangeError,
  osChangeHelp,
  osChangeToasts,
  osNames,
  packDrives,
  packFacts,
  readyDrives,
  rememberedCardModel,
  renameError,
  serverKey,
  setupError,
  usageLabel,
  usageSegments,
  wipeConfirmText,
} from './drives';

const GB = 1024 ** 3;

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

describe('osNames / hostPackOs', () => {
  it('names the computers in Linux, Windows order', () => {
    expect(osNames(['windows', 'linux'])).toBe('Linux and Windows');
    expect(osNames(['windows'])).toBe('Windows');
    expect(osNames([])).toBe('');
  });
  it('maps navigator.platform to a pack OS, none for macOS', () => {
    expect(hostPackOs('Win32')).toBe('windows');
    expect(hostPackOs('Linux x86_64')).toBe('linux');
    expect(hostPackOs('MacIntel')).toBeNull();
  });
});

describe('setup and rename validation', () => {
  it('defaults the pack name to the volume label, else the mount folder', () => {
    expect(defaultPackName({ label: ' KINGSTON ', mount: '/run/media/p/KINGSTON' })).toBe('KINGSTON');
    expect(defaultPackName({ label: '', mount: '/run/media/p/USB1/' })).toBe('USB1');
    expect(defaultPackName({ label: '', mount: 'E:\\' })).toBe('E:');
  });
  it('needs a name and at least one computer', () => {
    expect(setupError('  ', ['linux'])).toBe('Give the drive a name');
    expect(setupError('Couch kit', [])).toBe('Choose at least one computer');
    expect(setupError('Couch kit', ['linux'])).toBeNull();
    expect(renameError('')).toBe('Give the drive a name');
    expect(renameError('New')).toBeNull();
  });
});

describe('OS change', () => {
  it('keeps at least one computer ticked', () => {
    expect(osChangeError([])).toBe('Choose at least one computer');
    expect(osChangeError(['windows'])).toBeNull();
  });
  it('tells a real change from the same set in another order', () => {
    expect(osChanged(['linux', 'windows'], ['windows', 'linux'])).toBe(false);
    expect(osChanged(['linux'], ['linux', 'windows'])).toBe(true);
    expect(osChanged(['linux', 'windows'], ['windows'])).toBe(true);
  });
  it('explains what adding and removing do', () => {
    expect(osChangeHelp(2)).toBe(
      'Adding one writes its build of the 2 emulators on this drive. Removing one deletes those builds; games and firmware stay.',
    );
    expect(osChangeHelp(1)).toContain('the 1 emulator on');
    expect(osChangeHelp(0)).toMatch(/^No emulators are on this drive yet/);
  });
  it('leaves the queued builds of an added computer to the batch notifications', () => {
    expect(osChangeToasts('Couch kit', ['linux'], ['linux', 'windows'], { batch: 4, queued: 2, removed: 0 })).toEqual([]);
  });
  it('names the OSes a change adds', () => {
    expect(addedOses(['linux'], ['windows', 'linux'])).toEqual(['windows']);
    expect(addedOses(['linux', 'windows'], ['linux'])).toEqual([]);
  });
  it('toasts the removed builds, and only those on a swap', () => {
    expect(osChangeToasts('Couch kit', ['linux', 'windows'], ['linux'], { batch: 0, queued: 0, removed: 1 })).toEqual([
      'Removed the Windows builds of 1 emulator from Couch kit',
    ]);
    expect(osChangeToasts('Kit', ['linux'], ['windows'], { batch: 2, queued: 3, removed: 3 })).toEqual([
      'Removed the Linux builds of 3 emulators from Kit',
    ]);
  });
  it('confirms the change when no emulator builds moved', () => {
    expect(osChangeToasts('Kit', ['linux'], ['linux', 'windows'], { batch: 0, queued: 0, removed: 0 })).toEqual([
      'Kit is now for Linux and Windows',
    ]);
  });
});

describe('FAT32 warning and the wipe confirmation', () => {
  it('warns about 4 GB files and suggests exFAT', () => {
    expect(FAT32_WARNING.title).toBe("FAT32 can't hold files over 4 GB.");
    expect(FAT32_WARNING.body).toContain('exFAT');
  });
  it('states the games and the bytes Remove GRID files frees', () => {
    const t = wipeConfirmText('SAMSUNG T7', 6, 12.5 * GB);
    expect(t.title).toBe('Delete the gridpack folder from SAMSUNG T7?');
    expect(t.body).toBe(
      'This removes 6 games and 12.5 GB of GRID files from the drive. Nothing else on the drive is touched, and games installed on this computer stay.',
    );
    expect(wipeConfirmText('X', 1, 0).body).toContain('1 game and 0 B');
  });
  it('leaves the games out when the pack cannot be read', () => {
    expect(wipeConfirmText('U', null, 2048).body).toMatch(/^This removes 2\.0 KB of GRID files/);
  });
});

describe('paths and dates', () => {
  it('joins gridpack with the mount separator', () => {
    expect(gridpackPath('/run/media/p/T7')).toBe('/run/media/p/T7/gridpack');
    expect(gridpackPath('/run/media/p/T7/')).toBe('/run/media/p/T7/gridpack');
    expect(gridpackPath('E:')).toBe('E:\\gridpack');
    expect(gridpackPath('E:\\')).toBe('E:\\gridpack');
  });
  it('formats a day, blank for a bad timestamp', () => {
    expect(formatDay('2026-11-02T10:00:00Z', 'en-GB')).toBe('2 Nov 2026');
    expect(formatDay('', 'en-GB')).toBe('');
    expect(formatDay('nope', 'en-GB')).toBe('');
  });
  it('says when a drive was last seen in calendar days', () => {
    const now = new Date(2026, 10, 9, 12, 0);
    expect(lastSeenText(new Date(2026, 10, 9, 1, 0).toISOString(), now)).toBe('today');
    expect(lastSeenText(new Date(2026, 10, 8, 23, 50).toISOString(), now)).toBe('yesterday');
    expect(lastSeenText(new Date(2026, 10, 6, 9, 0).toISOString(), now)).toBe('3 days ago');
    expect(lastSeenText(new Date(2026, 8, 1, 9, 0).toISOString(), now, 'en-GB')).toBe(formatDay(new Date(2026, 8, 1, 9, 0).toISOString(), 'en-GB'));
    expect(lastSeenText('', now)).toBe('');
  });
});

describe('usageSegments', () => {
  it('splits the drive into other files, GRID and free space', () => {
    const [other, grid, free] = usageSegments(100 * GB, 40 * GB, 10 * GB);
    expect([other.kind, grid.kind, free.kind]).toEqual(['other', 'grid', 'free']);
    expect([other.bytes, grid.bytes, free.bytes]).toEqual([50 * GB, 10 * GB, 40 * GB]);
    expect([other.pct, grid.pct, free.pct]).toEqual([50, 10, 40]);
    expect(other.legend).toBe('Other files 50.0 GB');
    expect(grid.legend).toBe('GRID 10.0 GB');
    expect(free.legend).toBe('Free 40.0 GB');
  });
  it('never adds up past the drive size', () => {
    const segs = usageSegments(100, 90, 50);
    expect(segs.map((s) => s.bytes)).toEqual([0, 10, 90]);
    expect(segs.reduce((n, s) => n + s.pct, 0)).toBe(100);
  });
  it('is all zeros for a drive of unknown size', () => {
    expect(usageSegments(0, 0, 0).map((s) => s.pct)).toEqual([0, 0, 0]);
  });
  it('names the bar by its free space', () => {
    expect(usageLabel(100 * GB, 61.4 * GB)).toBe('61.4 GB free of 100.0 GB');
  });
});

describe('driveCardModel', () => {
  const t7 = drive('/run/media/p/T7', {
    label: 'SAMSUNG T7',
    total_bytes: 1000 * GB,
    pack: pack({ name: 'Couch kit', os: ['windows', 'linux'] }),
  });

  it('shows a set-up drive by its pack name, with the label, file system and size', () => {
    const m = driveCardModel(t7);
    expect(m.title).toBe('Couch kit');
    expect(m.subtitle).toBe('SAMSUNG T7 · exFAT · 1000.0 GB');
    expect(m.status).toBe('Ready for backups');
    expect(m.tone).toBe('ok');
    expect(m.forLine).toBe('Linux and Windows');
    expect(m.note).toBe('');
    expect([m.showsPack, m.canEdit, m.canSetUp, m.canRemove]).toEqual([true, true, false, true]);
  });

  it('leaves the label out when the pack is named after it', () => {
    const m = driveCardModel(drive('/m/K', { label: 'KIT', pack: pack({ name: 'KIT' }), total_bytes: GB }));
    expect(m.subtitle).toBe('exFAT · 1.0 GB');
  });

  it('offers setup on a writable drive without a pack', () => {
    const m = driveCardModel(
      drive('/m/K', { label: 'KINGSTON', fs: 'FAT32', fat32: true, status: 'not_set_up', pack: null, free_bytes: 31 * GB }),
    );
    expect(m.title).toBe('KINGSTON');
    expect(m.status).toBe('Not set up');
    expect(m.note).toBe('31.0 GB free. Set it up to back up games to it.');
    expect([m.showsPack, m.canEdit, m.canSetUp, m.canRemove]).toEqual([false, false, true, false]);
  });

  it('shows a read-only pack without the write actions', () => {
    const m = driveCardModel(drive('/m/R', { label: 'R', status: 'read_only', read_only: true }));
    expect(m.status).toBe('Read-only');
    expect(m.note).toBe("R is read-only. Games install from it, but backups can't go to it.");
    expect([m.showsPack, m.canEdit, m.canSetUp, m.canRemove]).toEqual([true, false, false, false]);
  });

  it('cannot set up a read-only drive', () => {
    const m = driveCardModel(drive('/m/R', { label: 'R', status: 'read_only', read_only: true, pack: null }));
    expect(m.note).toBe("R is read-only, so GRID can't set it up.");
    expect(m.canSetUp).toBe(false);
  });

  it('names the problem of an unreadable pack and still lets its files go', () => {
    const m = driveCardModel(drive('/m/U', { label: 'U', status: 'unreadable', problem: 'bad json', pack: null }));
    expect(m.status).toBe("Can't read");
    expect(m.tone).toBe('warn');
    expect(m.note).toBe("GRID can't read the gridpack folder on U: bad json");
    expect([m.showsPack, m.canEdit, m.canSetUp, m.canRemove]).toEqual([false, false, false, true]);
  });

  it('asks for a newer GRID on a newer-format pack and changes nothing', () => {
    const m = driveCardModel(drive('/m/N', { label: 'N', status: 'newer_format', pack: null }));
    expect(m.status).toBe('Newer GRID needed');
    expect(m.note).toBe('N was set up by a newer version of GRID. Update GRID to use it.');
    expect([m.showsPack, m.canEdit, m.canSetUp, m.canRemove]).toEqual([false, false, false, false]);
  });
});

describe('packFacts', () => {
  it('lists games, distinct emulators, firmware, last backup and the folder', () => {
    const d = drive('/run/media/p/T7', {
      pack: pack({
        game_count: 6,
        emulators: [
          { name: 'DuckStation', version: '1', os: 'linux' },
          { name: 'DuckStation', version: '1', os: 'windows' },
          { name: 'Dolphin', version: '2', os: 'linux' },
        ],
        firmware: ['PlayStation'],
        updated: '2026-11-02T10:00:00Z',
      }),
    });
    expect(packFacts(d, 'en-GB')).toEqual([
      { label: 'Games', value: '6', mono: false },
      { label: 'Emulators', value: 'DuckStation, Dolphin', mono: false },
      { label: 'Firmware', value: 'PlayStation', mono: false },
      { label: 'Last backup', value: '2 Nov 2026', mono: false },
      { label: 'Folder', value: '/run/media/p/T7/gridpack', mono: true },
    ]);
  });
  it('says None for an empty pack and nothing for a drive without one', () => {
    const facts = packFacts(drive('/m/a'));
    expect(facts.map((f) => f.value).slice(0, 4)).toEqual(['0', 'None', 'None', 'None yet']);
    expect(packFacts(drive('/m/b', { pack: null }))).toEqual([]);
  });
  it('dedupes emulator names', () => {
    expect(
      emulatorNames([
        { name: 'A', version: '1', os: 'linux' },
        { name: 'A', version: '1', os: 'windows' },
      ]),
    ).toEqual(['A']);
  });
});

describe('rememberedCardModel', () => {
  const now = new Date(2026, 10, 9, 12, 0);
  const wd: RememberedDrive = {
    mount: '/run/media/p/WD',
    name: 'Everything',
    label: 'WD Elements',
    fs: 'NTFS',
    total_bytes: 2000 * GB,
    game_count: 16,
    last_seen: new Date(2026, 10, 6, 9, 0).toISOString(),
  };
  it('shows when it was last seen, its games and its size', () => {
    expect(rememberedCardModel(wd, now)).toEqual({
      title: 'Everything',
      subtitle: 'WD Elements · NTFS · 2.0 TB',
      note: 'Last seen 3 days ago · 16 games. Plug it in to back up to it or install from it.',
    });
  });
  it('drops an unknown last-seen time', () => {
    expect(rememberedCardModel({ ...wd, last_seen: '' }, now).note).toBe(
      '16 games. Plug it in to back up to it or install from it.',
    );
  });
});
