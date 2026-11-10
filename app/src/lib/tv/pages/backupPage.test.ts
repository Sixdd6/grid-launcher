import { describe, expect, it, vi } from 'vitest';
import type { BackupPlan, DriveInfo, PackSummary } from '../../api';
import {
  backupLede,
  backupMain,
  backupPageRows,
  CHOOSE_WHY,
  EACH_INLINE_MAX,
  firmwareText,
  noDrivesNote,
  outdatedWords,
  type BackupPageActions,
  type BackupPageModel,
} from './backupPage';
import type { PageRow } from './pageRows';

const GB = 1024 ** 3;
const KEY = '/run/media/p/SAMSUNG T7';

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
    key: KEY,
    mount: KEY,
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
    drive: KEY,
    games_to_write: [1, 2, 3],
    skipped: [],
    too_big: [],
    emulators: [],
    firmware: [],
    firmware_reason: '',
    runtime: null,
    bytes_to_write: 4.2 * GB,
    free_bytes: 61.4 * GB,
    reserved_bytes: 0,
    fits: true,
    local_available: false,
    use_local: true,
    sources: [],
    outdated: [],
    needs_choice: 0,
    local_bytes: 0,
    download_bytes: 4.2 * GB,
    ...overrides,
  };
}

function model(p: BackupPlan | null, overrides: Partial<BackupPageModel> = {}): BackupPageModel {
  const d = drive();
  return {
    title: 'Backup 3 games',
    host: 'romm.home.lan',
    ready: [d],
    connected: [d],
    plans: p ? { [KEY]: p } : {},
    chosen: KEY,
    busy: false,
    useLocal: true,
    choices: {},
    includeFirmware: true,
    includeRuntime: false,
    each: false,
    error: null,
    starting: false,
    ...overrides,
  };
}

function acts(): BackupPageActions {
  return {
    choose: vi.fn(),
    setUp: vi.fn(),
    toggleLocal: vi.fn(),
    choiceFor: vi.fn(),
    pickAll: vi.fn(),
    toggleEach: vi.fn(),
    toggleFirmware: vi.fn(),
    toggleRuntime: vi.fn(),
    start: vi.fn(),
  };
}

const keys = (rows: PageRow[]) => rows.map((r) => r.key);
const find = (rows: PageRow[], key: string) => rows.find((r) => r.key === key);

const localPlan = (n: number, extra: Partial<BackupPlan> = {}) =>
  plan({
    local_available: true,
    games_to_write: Array.from({ length: n }, (_, i) => i + 1),
    sources: Array.from({ length: n }, (_, i) => ({ rom_id: i + 1, name: `Game ${i + 1}`, source: 'local' as const, note: '' })),
    local_bytes: 3 * GB,
    download_bytes: 1.2 * GB,
    ...extra,
  });

describe('the lede (item 458)', () => {
  it('names the host, and copying from this computer when a game is', () => {
    expect(backupLede('romm.home.lan', localPlan(2))).toMatch(/^GRID copies games installed on this computer and downloads the rest from romm\.home\.lan/);
    expect(backupLede('romm.home.lan', plan())).toMatch(/^GRID downloads each game from romm\.home\.lan/);
  });
});

describe('Drive (items 460–464)', () => {
  it('lists one radio row per drive with the space left after this backup', () => {
    const rows = backupPageRows(model(plan()), acts());
    const r = find(rows, `bk-drive-${KEY}`);
    expect(r).toMatchObject({ kind: 'row', radio: true, al: 'Choose', label: 'Couch kit', muted: ' · SAMSUNG T7 · exFAT', sub: '57.2 GB left after this backup', subErr: false });
    if (r?.kind !== 'row') throw new Error('row');
    expect(r.bar?.segs.some((s) => s.kind === 'adding')).toBe(true);
  });

  it('says what queued backups set aside (ruling 12)', () => {
    const rows = backupPageRows(model(plan({ reserved_bytes: 1.2 * GB, free_bytes: 60.2 * GB })), acts());
    const r = find(rows, `bk-drive-${KEY}`);
    if (r?.kind !== 'row') throw new Error('row');
    expect(r.sub).toBe('60.2 GB available');
    expect(r.bar?.after).toBe('1.2 GB set aside for queued backups');
  });

  it('says when the backup doesn’t fit, in the danger colour (item 462)', () => {
    const rows = backupPageRows(model(plan({ fits: false, bytes_to_write: 76 * GB })), acts());
    expect(find(rows, `bk-drive-${KEY}`)).toMatchObject({ sub: 'Needs 76 GB · 61.4 GB free', subErr: true });
  });

  it('with no set-up drive, says so, names one not set up, and offers Set up a drive… (item 464)', () => {
    const raw = drive({ key: '/m/USB', mount: '/m/USB', label: 'USB-1TB', status: 'not_set_up', pack: null });
    const rows = backupPageRows(model(null, { ready: [], connected: [raw], chosen: null }), acts());
    expect(keys(rows)).toEqual(['bk-h-drive', 'bk-no-drives', 'bk-setup']);
    expect(find(rows, 'bk-no-drives')).toMatchObject({ text: 'No Backup drive is connected. USB-1TB is connected but not set up for GRID.' });
    expect(noDrivesNote([])).toBe('No Backup drive is connected.');
    expect(backupMain(model(null, { ready: [], chosen: null }), vi.fn())).toBeNull();
  });
});

describe('Source (items 465–472, 476; ruling 11)', () => {
  it('shows only when a game can be copied from this computer (465)', () => {
    expect(keys(backupPageRows(model(plan()), acts()))).not.toContain('bk-h-source');
    expect(keys(backupPageRows(model(localPlan(2)), acts()))).toContain('bk-h-source');
  });

  it('starts with the switch and its count note (466)', () => {
    const rows = backupPageRows(model(localPlan(2)), acts());
    expect(find(rows, 'bk-local')).toMatchObject({ toggle: true, label: 'Copy installed games from this computer', sub: '2 games from this computer (3 GB)' });
  });

  it('switched off, every game downloads and the rows hide (466)', () => {
    const rows = backupPageRows(model(localPlan(2, { use_local: false }), { useLocal: false }), acts());
    expect(find(rows, 'bk-local')).toMatchObject({ toggle: false, sub: 'Every game downloads from romm.home.lan' });
    expect(keys(rows).some((k) => k.startsWith('bk-src-'))).toBe(false);
  });

  it('lists each game inline up to 6, behind "Show each game (n)" above that (ruling 11)', () => {
    const six = backupPageRows(model(localPlan(EACH_INLINE_MAX)), acts());
    expect(keys(six).filter((k) => k.startsWith('bk-src-'))).toHaveLength(6);
    expect(keys(six)).not.toContain('bk-each');
    const seven = backupPageRows(model(localPlan(7)), acts());
    expect(find(seven, 'bk-each')).toMatchObject({ label: 'Show each game', value: '7', al: 'Show' });
    expect(keys(seven).filter((k) => k.startsWith('bk-src-'))).toHaveLength(0);
    const open = backupPageRows(model(localPlan(7), { each: true }), acts());
    expect(find(open, 'bk-each')).toMatchObject({ label: 'Hide each game', al: 'Hide' });
    expect(keys(open).filter((k) => k.startsWith('bk-src-'))).toHaveLength(7);
  });

  it('tags each game and says why an installed game downloads (471–472)', () => {
    const p = localPlan(2, {
      sources: [
        { rom_id: 1, name: 'Lantern Vale', source: 'local', note: '' },
        { rom_id: 2, name: 'Brass Comet', source: 'download', note: 'Its files weren’t found on this computer' },
      ],
    });
    const rows = backupPageRows(model(p, { lookup: () => ({ platform: 'PlayStation', windows: false }) }), acts());
    expect(find(rows, 'bk-src-1')).toMatchObject({ kind: 'info', label: 'Lantern Vale', sub: 'PlayStation', tag: 'From this computer' });
    expect(find(rows, 'bk-src-2')).toMatchObject({ sub: 'PlayStation · Its files weren’t found on this computer', tag: 'Download' });
  });

  it('asks for each out-of-date game, with the bulk rows when there are several (467–470)', () => {
    const p = localPlan(3, {
      outdated: [
        { rom_id: 1, name: 'Lantern Vale', installed_version: 'v1.1', server_version: 'v1.2', choice: null },
        { rom_id: 2, name: 'Neon Orchard', installed_version: '', server_version: '', choice: null },
      ],
      needs_choice: 2,
    });
    const rows = backupPageRows(model(p, { choices: { 2: 'download' } }), acts());
    expect(find(rows, 'bk-ood-note')).toMatchObject({ warn: true, text: '2 games have newer files on the server. Choose a copy for each.' });
    expect(find(rows, 'bk-ood-1')).toMatchObject({
      warn: true,
      label: 'Lantern Vale: the server has newer files',
      sub: 'v1.2 on the server; this computer has v1.1',
      value: 'Choose',
      valueWarn: true,
    });
    expect(find(rows, 'bk-ood-2')).toMatchObject({ warn: false, value: 'Download the newer version', valueWarn: false });
    expect(find(rows, 'bk-all-local')).toMatchObject({ label: 'Use this computer’s copies' });
    expect(find(rows, 'bk-all-download')).toMatchObject({ label: 'Download all' });
    expect(find(rows, 'bk-src-1')).toMatchObject({ tag: 'Choose below' });
  });

  it('words the version choice like the Game backups card (468–469)', () => {
    const w = outdatedWords({ name: 'Lantern Vale', installed_version: 'v1.1', server_version: 'v1.2' });
    expect(w.listSub).toBe('The server has newer files (v1.2; this computer has v1.1)');
    expect(w.download).toBe('Download v1.2 from the server');
    expect(w.chosenDownload).toBe('Download v1.2');
    const bare = outdatedWords({ name: 'X', installed_version: '', server_version: '' });
    expect(bare.listSub).toBe('The server has newer files (this computer has an older copy)');
    expect(bare.download).toBe('Download the newer version from the server');
  });

  it('notes the Wine prefix when a Windows game is copied (476)', () => {
    const rows = backupPageRows(model(localPlan(1), { lookup: () => ({ platform: 'Windows', windows: true }) }), acts());
    expect(find(rows, 'bk-wine')).toMatchObject({
      text: 'Windows games are copied without their Wine prefix. The save folders GRID knows about are copied from it.',
    });
  });
});

describe('Also copy and the notes (items 473–478)', () => {
  it('always copies the emulators, with a fixed check', () => {
    const rows = backupPageRows(model(plan({ emulators: [{ name: 'PCSX2', os: ['linux', 'windows'], bytes: 120 * 1024 ** 2, already: false, note: '' }] })), acts());
    expect(find(rows, 'bk-emus')).toMatchObject({ check: true, sub: 'PCSX2 for Linux and Windows · 120 MB' });
    const already = backupPageRows(model(plan({ emulators: [{ name: 'PCSX2', os: ['linux'], bytes: 1, already: true, note: '' }] })), acts());
    expect(find(already, 'bk-emus')).toMatchObject({ sub: 'Already on the drive' });
  });

  it('counts the firmware, or says why it is off (474; ruling 12)', () => {
    expect(firmwareText(plan({ firmware: [{ platform_name: 'PlayStation', files: 3, bytes: 1, already: false }, { platform_name: 'PlayStation 2', files: 2, bytes: 1, already: false }] }))).toEqual({ text: '2 systems · 5 files', disabled: false });
    expect(firmwareText(plan({ firmware_reason: 'The server has no firmware for these systems' }))).toEqual({ text: 'The server has no firmware for these systems', disabled: true });
    const rows = backupPageRows(model(plan({ firmware_reason: 'Already on Couch kit' })), acts());
    expect(find(rows, 'bk-fw')).toMatchObject({ disabled: true, why: 'Already on Couch kit', toggle: false });
  });

  it('offers to make Windows games work offline when the plan does (475)', () => {
    const rows = backupPageRows(model(plan({ runtime: { name: 'Steam Linux Runtime', bytes: 0.3 * GB, already: false, included: false } })), acts());
    expect(find(rows, 'bk-runtime')).toMatchObject({ toggle: false, label: 'Make Windows games work offline' });
    expect(keys(backupPageRows(model(plan()), acts()))).not.toContain('bk-runtime');
  });

  it('notes games already on the drive and FAT32’s limit (477–478)', () => {
    const p = plan({ skipped: [{ rom_id: 9, name: 'Moss & Marrow' }], too_big: [{ rom_id: 3, name: 'Big One' }] });
    const fat = drive({ fs: 'FAT32', fat32: true });
    const rows = backupPageRows(model(p, { ready: [fat], connected: [fat] }), acts());
    expect(find(rows, 'bk-note-0')).toMatchObject({ text: '1 game is already on Couch kit and will be skipped: Moss & Marrow.', warn: false });
    expect(find(rows, 'bk-note-1')).toMatchObject({ warn: true });
  });
});

describe('the footer (items 479–481)', () => {
  it('reads Backup N games with what it writes and downloads', () => {
    const m = backupMain(model(localPlan(3)), vi.fn());
    expect(m).toMatchObject({ label: 'Backup 3 games', disabled: false, summary: '4.2 GB to write · 1.2 GB to download', hint: 'Backup' });
    expect(backupMain(model(plan({ games_to_write: [1] })), vi.fn())?.label).toBe('Backup 1 game');
    expect(backupMain(model(plan()), vi.fn())?.summary).toBe('4.2 GB to write');
  });

  it('says why it is disabled', () => {
    expect(backupMain(model(plan({ games_to_write: [] })), vi.fn())).toMatchObject({ label: 'Nothing to backup', disabled: true, why: 'Every game is already on Couch kit.' });
    expect(backupMain(model(localPlan(2, { needs_choice: 2 })), vi.fn())).toMatchObject({ label: 'Choose for 2 games', disabled: true, why: CHOOSE_WHY });
    expect(backupMain(model(plan({ fits: false })), vi.fn())).toMatchObject({ label: 'Not enough space', disabled: true, why: 'Couch kit doesn’t have room for this backup.' });
  });
});
