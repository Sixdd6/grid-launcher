import { describe, expect, it } from 'vitest';
import type { BackupPlan, DriveInfo, PackSummary } from '../api';
import {
  anyLocal,
  backupRequest,
  chooseAll,
  backupTitle,
  chooseDrive,
  dialogLede,
  driveRow,
  emulatorLine,
  firmwareLine,
  footerModel,
  localLine,
  noDrivesText,
  outdatedRows,
  planNotes,
  runtimeLine,
  sizeText,
  sourcePicks,
  sourceRows,
  wineNote,
} from './dialog';

const GB = 1024 ** 3;
const MB = 1024 ** 2;

function pack(overrides: Partial<PackSummary> = {}): PackSummary {
  return {
    name: 'Couch kit',
    os: ['linux', 'windows'],
    game_count: 6,
    gridpack_bytes: 100 * GB,
    emulators: [],
    firmware: [],
    updated: '2026-11-02T10:00:00Z',
    rom_ids: [],
    ...overrides,
  };
}

function drive(overrides: Partial<DriveInfo> = {}): DriveInfo {
  return {
    key: '/run/media/p/SAMSUNG T7',
    mount: '/run/media/p/SAMSUNG T7',
    label: 'SAMSUNG T7',
    fs: 'exFAT',
    fat32: false,
    total_bytes: 1000 * GB,
    free_bytes: 61.4 * GB,
    read_only: false,
    status: 'ready',
    problem: '',
    pack: pack(),
    ...overrides,
  };
}

function plan(overrides: Partial<BackupPlan> = {}): BackupPlan {
  return {
    drive: '/run/media/p/SAMSUNG T7',
    games_to_write: [1, 2, 3],
    skipped: [],
    too_big: [],
    emulators: [],
    firmware: [],
    firmware_reason: '',
    runtime: null,
    bytes_to_write: 11.1 * GB,
    free_bytes: 61.4 * GB,
    fits: true,
    local_available: false,
    use_local: true,
    sources: [],
    outdated: [],
    needs_choice: 0,
    local_bytes: 0,
    download_bytes: 11.1 * GB,
    ...overrides,
  };
}

describe('sizeText', () => {
  it('keeps one decimal and drops a whole number’s ".0"', () => {
    expect(sizeText(11.1 * GB)).toBe('11.1 GB');
    expect(sizeText(76 * GB)).toBe('76 GB');
    expect(sizeText(120 * MB)).toBe('120 MB');
  });
});

describe('titles and requests', () => {
  it('titles each way of starting a backup', () => {
    expect(backupTitle({ kind: 'games', count: 3 })).toBe('Backup 3 games');
    expect(backupTitle({ kind: 'games', count: 1 })).toBe('Backup 1 game');
    expect(backupTitle({ kind: 'game', name: 'Cinder Peaks' })).toBe('Backup Cinder Peaks');
    expect(backupTitle({ kind: 'platform', platform: 'PlayStation', count: 4 })).toBe('Backup PlayStation · 4 games');
  });

  it('builds the request, with platform ids only when given', () => {
    expect(backupRequest({ kind: 'game', name: 'Cinder Peaks' }, [7])).toEqual({
      rom_ids: [7],
      title: 'Backup Cinder Peaks',
    });
    expect(backupRequest({ kind: 'platform', platform: 'PlayStation', count: 2 }, [1, 2], [5])).toEqual({
      rom_ids: [1, 2],
      platform_ids: [5],
      title: 'Backup PlayStation · 2 games',
    });
  });

  it('names the server in the lede', () => {
    expect(dialogLede('romm.home.lan')).toContain('from romm.home.lan and writes it');
    expect(dialogLede('')).toContain('from the server');
  });

  it('says it copies installed games when some game comes from this computer', () => {
    expect(dialogLede('romm.home.lan', true)).toBe(
      'GRID copies games installed on this computer and downloads the rest from romm.home.lan, then writes each one, with its art and details, to the drive’s gridpack folder.',
    );
    expect(dialogLede('romm.home.lan', false)).toContain('GRID downloads each game from romm.home.lan');
  });
});

describe('driveRow', () => {
  it('shows the pack, label, file system and free space', () => {
    const row = driveRow(drive(), plan(), true);
    expect(row.name).toBe('Couch kit');
    expect(row.sub).toBe('SAMSUNG T7 · exFAT');
    expect(row.freeText).toBe('61.4 GB free');
    expect(row.over).toBe(false);
  });

  it('reads "Needs 76 GB · 61.4 GB free" when the backup does not fit', () => {
    const row = driveRow(drive(), plan({ bytes_to_write: 76 * GB, fits: false }), false);
    expect(row.freeText).toBe('Needs 76 GB · 61.4 GB free');
    expect(row.over).toBe(true);
  });

  it('hatches this backup’s share into the chosen row only, taken from free space', () => {
    const chosen = driveRow(drive(), plan({ bytes_to_write: 10 * GB }), true);
    const adding = chosen.segments.find((s) => s.kind === 'adding')!;
    expect(adding.pct).toBeCloseTo(1, 5);
    const free = chosen.segments.find((s) => s.kind === 'free')!;
    expect(free.pct).toBeCloseTo(5.14, 1);
    const other = driveRow(drive(), plan({ bytes_to_write: 10 * GB }), false);
    expect(other.segments.some((s) => s.kind === 'adding')).toBe(false);
    const total = chosen.segments.reduce((s, x) => s + x.pct, 0);
    expect(total).toBeCloseTo(100, 5);
  });

  it('shows only the free space while the plan is being made', () => {
    const row = driveRow(drive(), null, true);
    expect(row.freeText).toBe('61.4 GB free');
    expect(row.segments.some((s) => s.kind === 'adding')).toBe(false);
  });

  it('chooses the current drive while it is still ready, else the first', () => {
    const a = drive({ key: 'a' });
    const b = drive({ key: 'b' });
    expect(chooseDrive([a, b], 'b')).toBe('b');
    expect(chooseDrive([a, b], 'gone')).toBe('a');
    expect(chooseDrive([], 'a')).toBeNull();
  });
});

describe('no drives', () => {
  it('names connected drives that are not set up', () => {
    const fresh = drive({ label: 'KINGSTON', status: 'not_set_up', pack: null });
    const text = noDrivesText([fresh]);
    expect(text.title).toBe('No backup drive connected');
    expect(text.body).toBe(
      'Plug in a drive you set up for GRID, or set up KINGSTON (connected now). Set up a drive once in Settings › Backup drives.',
    );
  });

  it('skips read-only drives and says only how to set one up when none is connected', () => {
    const ro = drive({ label: 'CDROM', status: 'not_set_up', read_only: true, pack: null });
    expect(noDrivesText([ro]).body).toBe(
      'Plug in a drive you set up for GRID. Set up a drive once in Settings › Backup drives.',
    );
  });
});

describe('emulatorLine', () => {
  it('names the emulators, the computers and the size', () => {
    const line = emulatorLine(
      plan({ emulators: [{ name: 'PCSX2', os: ['linux', 'windows'], bytes: 120 * MB, already: false, note: '' }] }),
    );
    expect(line.text).toBe('PCSX2 for Linux and Windows · 120 MB');
  });

  it('groups emulators by computers and leaves out an unknown size', () => {
    const line = emulatorLine(
      plan({
        emulators: [
          { name: 'DuckStation', os: ['linux', 'windows'], bytes: 50 * MB, already: false, note: '' },
          { name: 'GE-Proton10-17', os: ['linux'], bytes: null, already: false, note: '' },
        ],
      }),
    );
    expect(line.text).toBe('DuckStation for Linux and Windows; GE-Proton10-17 for Linux');
  });

  it('reads "Already on the drive" and passes the notes on', () => {
    const line = emulatorLine(
      plan({
        emulators: [
          { name: 'Dolphin', os: ['linux'], bytes: 0, already: true, note: '' },
          { name: 'Cemu', os: [], bytes: null, already: false, note: 'Cemu is only for Linux' },
        ],
      }),
    );
    expect(line.text).toBe('Already on the drive');
    expect(line.notes).toEqual(['Cemu is only for Linux']);
  });

  it('reads "None needed" with no emulators', () => {
    expect(emulatorLine(plan()).text).toBe('None needed');
  });
});

describe('firmwareLine', () => {
  it('lists the systems and file counts', () => {
    const line = firmwareLine(
      plan({
        firmware: [
          { platform_name: 'PlayStation', files: 3, bytes: 1500000, already: false },
          { platform_name: 'Saturn', files: 1, bytes: 500000, already: true },
        ],
      }),
    );
    expect(line).toEqual({ text: 'PlayStation (3 files)', disabled: false });
  });

  it('is disabled with the backend’s reason', () => {
    expect(firmwareLine(plan({ firmware_reason: 'Your server has no firmware for these systems' }))).toEqual({
      text: 'Your server has no firmware for these systems',
      disabled: true,
    });
  });
});

describe('runtimeLine', () => {
  it('is absent when the plan offers no runtime', () => {
    expect(runtimeLine(plan())).toBeNull();
  });

  it('offers the runtime with its size', () => {
    const line = runtimeLine(
      plan({ runtime: { name: 'Steam Linux Runtime', bytes: 300 * MB, already: false, included: false } }),
    )!;
    expect(line.label).toBe('Make Windows games work offline');
    expect(line.text).toBe('Adds the Steam Linux Runtime (300 MB) that umu-run otherwise downloads on its first launch');
    expect(line.disabled).toBe(false);
  });

  it('is disabled when the drive has it already', () => {
    const line = runtimeLine(
      plan({ runtime: { name: 'Steam Linux Runtime', bytes: 0, already: true, included: false } }),
    )!;
    expect(line.disabled).toBe(true);
    expect(line.text).toBe('The Steam Linux Runtime is already on the drive');
  });
});

describe('planNotes', () => {
  it('names skipped games, up to three', () => {
    const notes = planNotes(plan({ skipped: [{ rom_id: 4, name: 'Moss & Marrow' }] }), drive());
    expect(notes).toEqual([
      { text: '1 game is already on Couch kit and will be skipped: Moss & Marrow', warn: false },
    ]);
    const many = planNotes(
      plan({ skipped: [1, 2, 3, 4].map((id) => ({ rom_id: id, name: `G${id}` })) }),
      drive(),
    );
    expect(many[0].text).toBe('4 games are already on Couch kit and will be skipped');
  });

  it('lists games too big for a FAT32 drive as a warning', () => {
    const fat = drive({ label: 'KINGSTON', fs: 'FAT32', fat32: true });
    const notes = planNotes(plan({ too_big: [{ rom_id: 9, name: 'Big Game' }] }), fat);
    expect(notes).toEqual([
      { text: '1 game has a file over 4 GB, which KINGSTON (FAT32) can’t hold: Big Game', warn: true },
    ]);
  });
});

describe('footerModel', () => {
  it('shows the bytes to write and the game count', () => {
    expect(footerModel(plan())).toEqual({ note: '11.1 GB to write', button: 'Backup 3 games', disabled: false });
  });

  it('reads "Nothing to backup" when every game is on the drive', () => {
    expect(footerModel(plan({ games_to_write: [], bytes_to_write: 0 }))).toEqual({
      note: 'Nothing new to write',
      button: 'Nothing to backup',
      disabled: true,
    });
  });

  it('reads "Not enough space" when it does not fit', () => {
    const f = footerModel(plan({ fits: false, bytes_to_write: 76 * GB }));
    expect(f).toEqual({ note: '76 GB to write', button: 'Not enough space', disabled: true });
  });

  it('waits while the plan is being made', () => {
    expect(footerModel(null).disabled).toBe(true);
  });
});

// ── Source ───────────────────────────────────────────────────────────────

const LOCAL_PLAN: Partial<BackupPlan> = {
  games_to_write: [1, 2, 3, 4, 5, 6],
  local_available: true,
  use_local: true,
  sources: [
    { rom_id: 1, name: 'Cinder Peaks', source: 'local', note: '' },
    { rom_id: 2, name: 'Harbor Lights', source: 'local', note: '' },
    { rom_id: 3, name: 'Neon Orchard', source: 'local', note: '' },
    { rom_id: 4, name: 'Moss & Marrow', source: 'download', note: 'Its files weren’t found on this computer' },
    { rom_id: 5, name: 'Velvet Static', source: 'download', note: '' },
    { rom_id: 6, name: 'Lantern Vale', source: 'download', note: '' },
  ],
  outdated: [{ rom_id: 6, name: 'Lantern Vale', installed_version: 'v1.1', server_version: 'v1.2', choice: null }],
  needs_choice: 1,
  bytes_to_write: 14.3 * GB,
  local_bytes: 11.2 * GB,
  download_bytes: 3.1 * GB,
};

const lookup = (id: number) =>
  ({
    1: { platform: 'PlayStation', windows: false },
    2: { platform: 'Windows', windows: true },
    3: { platform: 'PlayStation 2', windows: false },
    4: { platform: 'Game Boy', windows: false },
    6: { platform: 'Nintendo 64', windows: false },
  })[id];

describe('localLine', () => {
  it('counts the games copied, downloaded and still to choose', () => {
    expect(localLine(plan(LOCAL_PLAN), 'romm.home.lan')).toEqual({
      label: 'Copy installed games from this computer',
      note: '3 games from this computer (11.2 GB) · 2 downloaded · 1 to choose',
    });
  });

  it('says every game downloads when unticked', () => {
    expect(localLine(plan({ ...LOCAL_PLAN, use_local: false }), 'romm.home.lan').note).toBe(
      'Every game downloads from romm.home.lan',
    );
  });

  it('says so when no game here can be copied', () => {
    const p = plan({ ...LOCAL_PLAN, sources: [LOCAL_PLAN.sources![3]], outdated: [], needs_choice: 0 });
    expect(localLine(p, 'h').note).toBe('No game here can be copied · 1 downloaded');
  });
});

describe('sourceRows', () => {
  it('tags each game and says why an installed one downloads', () => {
    const m = sourceRows(plan(LOCAL_PLAN), {}, lookup);
    expect(m.collapsed).toBe(false);
    expect(m.rows[0]).toEqual({
      romId: 1,
      name: 'Cinder Peaks',
      sub: 'PlayStation',
      tag: 'local',
      tagText: 'From this computer',
    });
    expect(m.rows[3].sub).toBe('Game Boy · Its files weren’t found on this computer');
    expect(m.rows[3].tagText).toBe('Download');
    expect(m.rows[4]).toMatchObject({ sub: '', tag: 'download' });
    expect(m.rows[5]).toMatchObject({ sub: 'Nintendo 64 · the server has newer files', tag: 'ask', tagText: 'Choose below' });
  });

  it('says which copy goes once an out-of-date game is picked', () => {
    expect(sourceRows(plan(LOCAL_PLAN), { 6: 'local' }).rows[5]).toMatchObject({
      sub: 'an older copy than the server’s',
      tag: 'download',
    });
    expect(sourceRows(plan(LOCAL_PLAN), { 6: 'download' }).rows[5].sub).toBe('the newer version');
  });

  it('collapses more than 6 games into "Show each game"', () => {
    const many = plan({
      sources: [1, 2, 3, 4, 5, 6, 7].map((id) => ({ rom_id: id, name: `G${id}`, source: 'download' as const, note: '' })),
    });
    expect(sourceRows(many)).toMatchObject({ collapsed: true, summary: 'Show each game', count: 7 });
  });
});

describe('outdatedRows', () => {
  it('asks per game which copy to backup', () => {
    const m = outdatedRows(plan(LOCAL_PLAN))!;
    expect(m.title).toBe('The server has newer files');
    expect(m.bulk).toBeNull();
    expect(m.items).toEqual([
      {
        romId: 6,
        name: 'Lantern Vale',
        detail: ': the server has newer files (v1.2; this computer has v1.1)',
        localLabel: 'Use this computer’s copy',
        downloadLabel: 'Download v1.2 from the server',
        choice: null,
      },
    ]);
  });

  it('counts several games and offers the bulk buttons', () => {
    const p = plan({
      ...LOCAL_PLAN,
      outdated: [
        { rom_id: 6, name: 'Lantern Vale', installed_version: 'v1.1', server_version: 'v1.2', choice: null },
        { rom_id: 5, name: 'Velvet Static', installed_version: '', server_version: 'v1.3', choice: 'download' },
      ],
      needs_choice: 1,
    });
    const m = outdatedRows(p, { 6: 'local' })!;
    expect(m.title).toBe('The server has newer files for 2 games');
    expect(m.bulk).toEqual({ local: 'Use this computer’s copies', download: 'Download all' });
    expect(m.items[0].choice).toBe('local');
    expect(m.items[1].detail).toBe(': the server has newer files (v1.3; this computer has an older copy)');
    expect(m.items[1].choice).toBe('download');
    expect(chooseAll(p, { 6: 'local' }, 'download')).toEqual({ 5: 'download', 6: 'download' });
  });

  it('is hidden when unticked or nothing is out of date', () => {
    expect(outdatedRows(plan({ ...LOCAL_PLAN, use_local: false }))).toBeNull();
    expect(outdatedRows(plan())).toBeNull();
  });
});

describe('sourcePicks', () => {
  it('turns the record into the IPC list', () => {
    expect(sourcePicks({ 6: 'local', 9: 'download' })).toEqual([
      { rom_id: 6, source: 'local' },
      { rom_id: 9, source: 'download' },
    ]);
  });
});

describe('wineNote', () => {
  it('appears when a Windows game is copied from this computer', () => {
    expect(wineNote(plan(LOCAL_PLAN), lookup)).toBe(
      'Windows games are copied without their Wine prefix, so saves kept in the prefix stay on this computer.',
    );
    const downloads = plan({ ...LOCAL_PLAN, sources: LOCAL_PLAN.sources!.filter((s) => s.rom_id !== 2) });
    expect(wineNote(downloads, lookup)).toBe('');
    expect(wineNote(plan({ ...LOCAL_PLAN, use_local: false }), lookup)).toBe('');
  });
});

describe('local copies in the notes and footer', () => {
  it('names only games that download in the FAT32 note', () => {
    const fat = drive({ label: 'KINGSTON', fs: 'FAT32', fat32: true });
    const p = plan({
      ...LOCAL_PLAN,
      too_big: [
        { rom_id: 1, name: 'Cinder Peaks' },
        { rom_id: 9, name: 'Big Game' },
      ],
    });
    expect(planNotes(p, fat)).toEqual([
      { text: '1 game has a file over 4 GB, which KINGSTON (FAT32) can’t hold: Big Game', warn: true },
    ]);
  });

  it('holds the button until every out-of-date game is picked', () => {
    expect(footerModel(plan({ ...LOCAL_PLAN, needs_choice: 2 }))).toEqual({
      note: '14.3 GB to write · 3.1 GB to download',
      button: 'Choose for 2 games',
      disabled: true,
    });
    expect(footerModel(plan({ ...LOCAL_PLAN, needs_choice: 1 })).button).toBe('Choose for 1 game');
    expect(footerModel(plan({ ...LOCAL_PLAN, needs_choice: 0 }))).toEqual({
      note: '14.3 GB to write · 3.1 GB to download',
      button: 'Backup 6 games',
      disabled: false,
    });
  });

  it('leaves out the download half when nothing comes from this computer', () => {
    expect(anyLocal(plan({ ...LOCAL_PLAN, use_local: false }))).toBe(false);
    expect(footerModel(plan({ ...LOCAL_PLAN, use_local: false, needs_choice: 0 })).note).toBe('14.3 GB to write');
  });
});
