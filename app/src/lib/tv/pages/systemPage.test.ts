import { describe, expect, it, vi } from 'vitest';
import type { Row } from '../rows';
import {
  BUSY_WHY,
  emulatorListItems,
  openingNotice,
  pageSub,
  protonListItems,
  removeConfirm,
  systemPageRows,
  umuValue,
  type EmuLine,
  type SystemPageActions,
  type SystemPageModel,
} from './systemPage';

function actions(): SystemPageActions {
  return {
    pickEmulator: vi.fn(),
    installRecommended: vi.fn(),
    pickCore: vi.fn(),
    update: vi.fn(),
    installFirmware: vi.fn(),
    installPs3Firmware: vi.fn(),
    open: vi.fn(),
    settings: vi.fn(),
    remove: vi.fn(),
    installCompanion: vi.fn(),
    pickProton: vi.fn(),
    installUmu: vi.fn(),
  };
}

const line = (name: string, extra: Partial<EmuLine> = {}): EmuLine => ({ name, version: '2.3.1', updateTo: null, updating: false, alsoPlays: '', tips: [], ...extra });

const emulator = (extra: Partial<Extract<SystemPageModel, { kind: 'emulator' }>> = {}): SystemPageModel => ({
  kind: 'emulator',
  system: 'PlayStation 2',
  status: 'Ready',
  emu: line('PCSX2'),
  core: null,
  firmware: null,
  ps3: null,
  companions: [],
  removing: false,
  ...extra,
});

/** "<key> <label> = <value>" for each row, the way the page reads. */
const shape = (rows: Row[]) => rows.map((r) => `${r.key} ${r.label}${'value' in r && r.value ? ` = ${r.value}` : ''}`);

describe('the System page', () => {
  it('an emulator system: Emulator, version, Open, Emulator settings, Remove, Setup tips (items 326–329, 332–334)', () => {
    const rows = systemPageRows(emulator({ emu: line('PCSX2', { version: '2.3.1 · up to date', tips: ['Map your controller once in PCSX2.'] }) }), actions());
    expect(shape(rows)).toEqual([
      'emu Emulator = PCSX2',
      'ver Version = 2.3.1 · up to date',
      'open Open PCSX2',
      'settings Emulator settings',
      'remove Remove PCSX2',
      'tips Setup tips',
    ]);
    expect(rows.find((r) => r.key === 'open')?.sub).toBe('Opens the emulator on its own to change its settings. Its window needs a mouse.');
    expect(rows.find((r) => r.key === 'settings')?.sub).toBe('Program, launch options, cloud save folders');
    expect(rows.find((r) => r.key === 'remove')?.danger).toBe(true);
    expect(rows.find((r) => r.key === 'tips')).toMatchObject({ type: 'info', sub: 'Map your controller once in PCSX2.' });
    expect(pageSub(emulator())).toBe('PCSX2 · Ready');
  });

  it('a newer version adds "Update to <version>" (item 329)', () => {
    const a = actions();
    const rows = systemPageRows(emulator({ emu: line('PCSX2', { updateTo: '2.3.4' }) }), a);
    const up = rows.find((r) => r.key === 'update');
    expect(up?.label).toBe('Update to 2.3.4');
    if (up?.type === 'action') up.run();
    expect(a.update).toHaveBeenCalledWith('PCSX2');
    const busy = systemPageRows(emulator({ emu: line('PCSX2', { updateTo: '2.3.4', updating: true }) }), a).find((r) => r.key === 'update');
    expect(busy?.disabled).toBe(true);
  });

  it('a RetroArch system has its core row (item 330)', () => {
    const rows = systemPageRows(emulator({ emu: line('RetroArch'), core: { options: ['snes9x', 'bsnes'], selected: 'snes9x', disabled: false } }), actions());
    expect(shape(rows)).toContain('core RetroArch core = snes9x');
    const none = systemPageRows(emulator({ emu: line('RetroArch'), core: { options: [], selected: '', disabled: true } }), actions());
    expect(none.find((r) => r.key === 'core')).toMatchObject({ value: 'No installed core', disabled: true });
  });

  it('firmware from the server: Install, Installing…, then Installed (item 331)', () => {
    const fw = (action: 'install' | 'requested' | 'installed' | 'unavailable') =>
      systemPageRows(emulator({ firmware: { label: 'PlayStation 2 firmware · 2 files', platformName: 'PlayStation 2', action } }), actions()).find((r) => r.key === 'fw');
    expect(fw('install')).toMatchObject({
      label: 'PlayStation 2 firmware · 2 files · from your server',
      sub: 'Each file is checked against the size and MD5 the server lists',
      value: 'Install',
    });
    expect(fw('requested')).toMatchObject({ value: 'Installing…', disabled: true });
    expect(fw('installed')).toMatchObject({ type: 'info', label: 'PlayStation 2 firmware', value: 'Installed' });
    expect(fw('unavailable')).toBeUndefined();
  });

  it("RPCS3's PS3 firmware line (SPEC.md)", () => {
    const installed = systemPageRows(emulator({ emu: line('RPCS3'), ps3: { kind: 'installed', text: 'PS3 firmware 4.93 installed' } }), actions());
    expect(installed.find((r) => r.key === 'ps3fw')).toMatchObject({ type: 'info', label: 'PS3 firmware 4.93 installed' });
    const a = actions();
    const install = systemPageRows(emulator({ emu: line('RPCS3'), ps3: { kind: 'install', text: 'PS3 firmware downloaded — click Install to activate it.' } }), a).find((r) => r.key === 'ps3fw');
    expect(install).toMatchObject({ value: 'Install' });
    if (install?.type === 'action') install.run();
    expect(a.installPs3Firmware).toHaveBeenCalledWith('RPCS3');
  });

  it('companions show under the emulator; a missing one offers Install (SPEC.md › Companions)', () => {
    const rows = systemPageRows(
      emulator({
        emu: line('ShadPS4'),
        companions: [
          { kind: 'installed', key: 'qt', line: line('ShadPS4 Qt Launcher', { version: '1.0' }) },
          { kind: 'missing', key: 'other', profile: 'Other Tool', text: 'Other Tool is not installed. Installing or updating ShadPS4 adds it too.', installing: false, canInstall: true },
        ],
      }),
      actions(),
    );
    expect(shape(rows)).toEqual(expect.arrayContaining(['comp-qt-ver ShadPS4 Qt Launcher = 1.0', 'comp-qt-open Open ShadPS4 Qt Launcher', 'comp-other-install Install Other Tool']));
  });

  it('Remove asks with the system named (item 333)', () => {
    expect(removeConfirm('PCSX2', 'PlayStation 2')).toEqual({
      title: 'Remove PCSX2?',
      body: "Games and saves stay. PlayStation 2 games can't start until another emulator is picked.",
      ok: 'Remove',
    });
    expect(removeConfirm('Mednafen', null).body).toBe('Games and saves stay.');
    expect(openingNotice('PCSX2')).toBe('Opening PCSX2…');
  });

  it('a system with no emulator: Install <recommended> (recommended) and Choose another emulator (item 335)', () => {
    const a = actions();
    const m: SystemPageModel = { kind: 'none', system: 'Saturn', status: 'No emulator', recommended: 'Ymir', missing: '', installing: false };
    const rows = systemPageRows(m, a);
    expect(shape(rows)).toEqual(['install Install Ymir (recommended)', 'choose Choose another emulator']);
    if (rows[0].type === 'action') rows[0].run();
    expect(a.installRecommended).toHaveBeenCalled();
    expect(pageSub(m)).toBe('No emulator');
    const none = systemPageRows({ ...m, recommended: null, missing: 'No emulator GRID installs plays this system.' }, a);
    expect(none[0]).toMatchObject({ type: 'info', label: 'No emulator GRID installs plays this system.' });
    expect(systemPageRows({ ...m, installing: true }, a)[0]).toMatchObject({ disabled: true, sub: 'Installing Ymir…' });
  });

  it('Windows: Proton version, the umu-run line, Reinstall and the first-game note (item 336)', () => {
    const m: SystemPageModel = {
      kind: 'windows',
      status: 'Ready',
      tool: 'GE-Proton9-20 (downloaded by GRID)',
      umu: 'GRID-managed · 1.2.6',
      umuAction: 'Reinstall',
      umuBusy: false,
      pickerBusy: false,
    };
    const rows = systemPageRows(m, actions());
    expect(shape(rows)).toEqual([
      'proton Default Proton version = GE-Proton9-20 (downloaded by GRID)',
      'umu Proton launcher (umu-run) = GRID-managed · 1.2.6',
      'umu-install Reinstall umu-launcher',
      'first First Windows game = Downloads the Steam Linux Runtime (about 200 MB)',
    ]);
    expect(rows[0].sub).toBe('Runs Windows games on Linux');
    expect(pageSub(m)).toBe('GE-Proton9-20 (downloaded by GRID) · Ready');
    expect(umuValue({ source: 'managed', release_tag: '1.2.6' })).toBe('GRID-managed · 1.2.6');
    expect(umuValue({ source: 'system', path: '/usr/bin/umu-run' })).toBe('System · /usr/bin/umu-run');
    expect(umuValue({ source: 'missing' })).toBe('Not installed');
  });

  it('an emulator no system uses gets its own lines (Other emulators)', () => {
    const m: SystemPageModel = { kind: 'other', system: 'Mednafen', note: 'Not used by any system on your server.', emu: line('Mednafen'), ps3: null, removing: false };
    expect(shape(systemPageRows(m, actions()))).toEqual(['ver Version = 2.3.1', 'open Open Mednafen', 'settings Emulator settings', 'remove Remove Mednafen']);
    expect(pageSub(m)).toBe('Not used by any system on your server.');
  });

  it('the emulator list: Installed and Download groups, (recommended), None last (items 327–328)', () => {
    const pick = vi.fn();
    const items = emulatorListItems(
      {
        selected: 'use:PCSX2',
        installed: [{ value: 'use:PCSX2', label: 'PCSX2 (recommended)', recommended: true }],
        download: [{ value: 'install:x', label: 'Install Play!', recommended: false }],
      },
      '',
      pick,
    );
    expect(items.map((i) => ('group' in i ? `[${i.group}]` : `${i.label}${i.checked ? ' ✓' : ''}`))).toEqual([
      '[Installed]',
      'PCSX2 (recommended) ✓',
      '[Download]',
      'Play!',
      '[ ]',
      'None',
    ]);
    const play = items[3];
    if (!('group' in play)) play.onPick?.();
    expect(pick).toHaveBeenCalledWith('install:x');
    const proton = protonListItems(
      { selected: '/w', installed: [{ value: '/w', label: 'Wine', recommended: false }], download: [{ value: 'install:ge', label: 'Install GE-Proton (latest, recommended)', recommended: true }] },
      pick,
    );
    expect(proton.map((i) => ('group' in i ? `[${i.group}]` : i.label))).toEqual(['[Installed]', 'Wine', '[Download]', 'GE-Proton (latest, recommended)']);
  });
});

describe('busy rows (consultant decision)', () => {
  it('says to try again when the action finishes', () => {
    expect(BUSY_WHY).toBe('Still in progress. Try again when it finishes.');
  });
});
