import { describe, expect, it, vi } from 'vitest';
import type { Row } from '../rows';
import {
  ADD_SUB,
  emulatorFormRows,
  fileName,
  NAME_NEEDED,
  programStart,
  savedNotice,
  STRATEGY_OPTIONS,
  typedArgs,
  whichLabel,
  type EmuFormActions,
  type EmuFormState,
} from './emulatorForm';

const form = (extra: Partial<EmuFormState> = {}): EmuFormState => ({
  name: '',
  path: '',
  args: '%rom%',
  argsDefault: true,
  saveStrategy: 'auto',
  savePaths: '',
  statePaths: '',
  ignoreFiles: '',
  ignoreExtensions: '',
  which: '',
  whichFilled: null,
  ...extra,
});

const acts = (): EmuFormActions => ({ which: vi.fn(), name: vi.fn(), program: vi.fn(), args: vi.fn(), strategy: vi.fn(), text: vi.fn(), save: vi.fn() });

const shape = (rows: Row[]) => rows.map((r) => `${r.label}${'value' in r ? ` = ${r.value}` : ''}${r.sub ? ` (${r.sub})` : ''}`);

describe('the emulator form', () => {
  it('adding: which emulator, Name, Program, Launch options "%rom%", Cloud saves, Add emulator (items 337–341)', () => {
    expect(ADD_SUB).toBe("For an emulator GRID doesn't install");
    const rows = emulatorFormRows('add', form(), acts());
    expect(shape(rows)).toEqual([
      'Which emulator is it? = Something else',
      'Name = Not set',
      "Program = Not set (Browse for the emulator's program)",
      'Launch options = %rom% (%rom% is replaced by the game file)',
      'Cloud save options = Save strategy: Automatic (Save and state folders, files not to upload)',
      'Save folders =  (Separate folders with ;)',
      'Save state folders = ',
      "Don't upload these files = ",
      "Don't upload these file types = ",
      'Add emulator',
    ]);
    // No name yet: Add says why.
    expect(rows[rows.length - 1]).toMatchObject({ disabled: true, why: NAME_NEEDED });
  });

  it('editing: no which row; the program shows its file name over its path; Save (items 339, 341)', () => {
    const rows = emulatorFormRows('edit', form({ name: 'PCSX2', path: '/home/sam/Emu/pcsx2.AppImage', args: '-batch %rom%', argsDefault: false, saveStrategy: 'folder' }), acts());
    expect(shape(rows).slice(0, 4)).toEqual([
      'Name = PCSX2',
      'Program = pcsx2.AppImage (/home/sam/Emu/pcsx2.AppImage)',
      'Launch options = -batch %rom% (%rom% is replaced by the game file)',
      'Cloud save options = Save strategy: A folder per game (Save and state folders, files not to upload)',
    ]);
    expect(rows[rows.length - 1]).toMatchObject({ label: 'Save' });
    expect(rows[rows.length - 1].disabled).toBeUndefined();
    expect(savedNotice('PCSX2')).toBe('PCSX2 settings saved');
  });

  it('the strategy list, file names and where the browser opens (items 339–340)', () => {
    expect(STRATEGY_OPTIONS).toEqual([
      ['auto', 'Automatic'],
      ['single_file', 'One file per game'],
      ['folder', 'A folder per game'],
    ]);
    expect(fileName('/a/b/duckstation')).toBe('duckstation');
    expect(programStart('/a/b/duckstation')).toBe('/a/b');
    expect(programStart('')).toBe('');
    expect(whichLabel('')).toBe('Something else');
    expect(whichLabel('DuckStation')).toBe('DuckStation');
  });

  it('the untouched "%rom%" counts as nothing typed for the autofill', () => {
    expect(typedArgs({ args: '%rom%', argsDefault: true })).toBe('');
    expect(typedArgs({ args: '%rom%', argsDefault: false })).toBe('%rom%');
    expect(typedArgs({ args: '-x', argsDefault: true })).toBe('-x');
  });
});

describe('Save with no name (consultant decision)', () => {
  it('asks for a name first', () => {
    expect(NAME_NEEDED).toBe('Give the emulator a name first.');
  });
});
