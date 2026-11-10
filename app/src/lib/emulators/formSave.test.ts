import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { EmulatorEntry } from '../api';
import type { FormFields } from './formSave';

type Fn = ReturnType<typeof vi.fn>;

const fields = (over: Partial<FormFields> = {}): FormFields => ({
  name: 'Dolphin',
  path: '/apps/dolphin',
  args: '-e {rom}',
  saveStrategy: 'single_file',
  ignoreFiles: '',
  ignoreExtensions: '',
  savePaths: '',
  statePaths: '',
  ...over,
});

describe('formSave', () => {
  let api: Record<string, Fn>;
  let toasts: string[];

  beforeEach(() => {
    vi.resetModules();
    toasts = [];
    api = {
      matchProfile: vi.fn(async () => ({ name: 'Dolphin', args: '-b -e {rom}' })),
      saveEmulator: vi.fn(async () => {}),
    };
    vi.doMock('../api', () => ({ api }));
    vi.doMock('../stores/toasts.svelte', () => ({ pushToast: (t: string) => toasts.push(t) }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('../stores/toasts.svelte');
  });

  it('fills the name and options from a profile only on an empty form', async () => {
    const { profileFill } = await import('./formSave');
    expect(await profileFill(' /apps/dolphin ', { name: '', args: '' })).toEqual({ name: 'Dolphin', args: '-b -e {rom}' });
    expect(api.matchProfile).toHaveBeenCalledWith('/apps/dolphin');
    expect(await profileFill('/apps/dolphin', { name: 'Mine', args: '' })).toBeNull();
    expect(await profileFill('  ', { name: '', args: '' })).toBeNull();
    api.matchProfile.mockRejectedValueOnce(new Error('x'));
    expect(await profileFill('/apps/dolphin', { name: '', args: '' })).toBeNull();
  });

  it('adds a new entry with a card', async () => {
    const { saveEmulatorForm } = await import('./formSave');
    await saveEmulatorForm('add', null, fields());
    expect(api.saveEmulator).toHaveBeenCalledWith('', expect.objectContaining({ name: 'Dolphin', path: '/apps/dolphin' }));
    expect(toasts).toEqual(["Added emulator 'Dolphin'."]);
  });

  it('saves an edit under its old name, keeping the fields the form does not show', async () => {
    const { saveEmulatorForm } = await import('./formSave');
    const entry = { name: 'Old', path: '/old', source_id: 'dolphin/dolphin' } as unknown as EmulatorEntry;
    await saveEmulatorForm('edit', entry, fields());
    expect(api.saveEmulator).toHaveBeenCalledWith(
      'Old',
      expect.objectContaining({ name: 'Dolphin', source_id: 'dolphin/dolphin' }),
    );
    expect(toasts).toEqual([]);
  });

  it('throws the refusal', async () => {
    api.saveEmulator.mockRejectedValueOnce(new Error('name taken'));
    const { saveEmulatorForm } = await import('./formSave');
    await expect(saveEmulatorForm('add', null, fields())).rejects.toThrow('name taken');
    expect(toasts).toEqual([]);
  });
});
