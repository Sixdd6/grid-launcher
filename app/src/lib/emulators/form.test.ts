import { describe, expect, it } from 'vitest';
import type { EmulatorEntry } from '../api';
import {
  addedEmulatorToast,
  ARGS_LABEL,
  emulatorFormValues,
  entryPatch,
  normalizeSaveStrategy,
  SAVE_STRATEGIES,
} from './form';

describe('SAVE_STRATEGIES', () => {
  it('is the reference dialog list, in order', () => {
    expect([...SAVE_STRATEGIES]).toEqual(['auto', 'single_file', 'folder']);
  });
});

describe('ARGS_LABEL', () => {
  it('is the reference label verbatim', () => {
    expect(ARGS_LABEL).toBe('Arguments (%rom%, %core%, %ps3_launch_target%)');
  });
});

describe('normalizeSaveStrategy', () => {
  it('maps blank, unknown and undefined to auto', () => {
    expect(normalizeSaveStrategy('')).toBe('auto');
    expect(normalizeSaveStrategy('   ')).toBe('auto');
    expect(normalizeSaveStrategy('not-a-strategy')).toBe('auto');
    expect(normalizeSaveStrategy(undefined)).toBe('auto');
    expect(normalizeSaveStrategy(null)).toBe('auto');
  });

  it('maps every single_file alias', () => {
    for (const alias of ['singlefile', 'single_file', 'single-file', 'single file', 'file']) {
      expect(normalizeSaveStrategy(alias)).toBe('single_file');
      expect(normalizeSaveStrategy(`  ${alias.toUpperCase()}  `)).toBe('single_file');
    }
  });

  it('maps every folder alias', () => {
    for (const alias of ['folder', 'directory', 'folder_per_game', 'folder-per-game']) {
      expect(normalizeSaveStrategy(alias)).toBe('folder');
    }
  });
});

describe('emulatorFormValues', () => {
  it('seeds every field empty for an add', () => {
    expect(emulatorFormValues(null)).toEqual({
      name: '',
      path: '',
      args: '',
      saveStrategy: 'auto',
      ignoreFiles: '',
      ignoreExtensions: '',
      savePaths: '',
      statePaths: '',
    });
  });

  it('seeds from an entry and normalizes the stored strategy alias', () => {
    const entry: EmulatorEntry = {
      name: 'DuckStation',
      path: '/opt/duckstation',
      args: '%rom%',
      save_strategy: 'single-file',
      ignore_files: 'a.bin;b.bin',
      ignore_extensions: '.tmp;.log',
      save_paths: 'memcards',
      state_paths: 'savestates',
    };
    expect(emulatorFormValues(entry)).toEqual({
      name: 'DuckStation',
      path: '/opt/duckstation',
      args: '%rom%',
      saveStrategy: 'single_file',
      ignoreFiles: 'a.bin;b.bin',
      ignoreExtensions: '.tmp;.log',
      savePaths: 'memcards',
      statePaths: 'savestates',
    });
  });

  it('seeds a missing optional field as blank', () => {
    const entry: EmulatorEntry = { name: 'Bare', path: '/x', args: '' };
    const values = emulatorFormValues(entry);
    expect(values.saveStrategy).toBe('auto');
    expect(values.ignoreFiles).toBe('');
    expect(values.statePaths).toBe('');
  });
});

describe('entryPatch', () => {
  const values = {
    name: '  RetroArch  ',
    path: '/opt/retroarch',
    args: '-L "%core%" "%rom%"',
    saveStrategy: 'folder' as const,
    ignoreFiles: '  a.bin;b.bin  ',
    ignoreExtensions: '  .tmp  ',
    savePaths: '  saves  ',
    statePaths: '  states  ',
  };

  it('trims the name and the four semicolon lists', () => {
    expect(entryPatch(values)).toEqual({
      name: 'RetroArch',
      path: '/opt/retroarch',
      args: '-L "%core%" "%rom%"',
      save_strategy: 'folder',
      ignore_files: 'a.bin;b.bin',
      ignore_extensions: '.tmp',
      save_paths: 'saves',
      state_paths: 'states',
    });
  });

  it('leaves the path and args exactly as typed', () => {
    const patch = entryPatch({ ...values, path: '  /spaced/path  ', args: '  %rom%  ' });
    expect(patch.path).toBe('  /spaced/path  ');
    expect(patch.args).toBe('  %rom%  ');
  });

  it('always writes a strategy, never a blank', () => {
    expect(entryPatch({ ...values, saveStrategy: 'auto' }).save_strategy).toBe('auto');
  });
});

describe('addedEmulatorToast', () => {
  it('is the reference toast verbatim', () => {
    expect(addedEmulatorToast('RetroArch (Multi-System)')).toBe(
      "Added emulator 'RetroArch (Multi-System)'.",
    );
  });

  it('uses the trimmed name, as the backend stores it', () => {
    expect(addedEmulatorToast('  Dolphin  ')).toBe("Added emulator 'Dolphin'.");
  });
});
