import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Fn = ReturnType<typeof vi.fn>;

describe('nativeSettingsStore', () => {
  let api: Record<string, Fn>;

  beforeEach(() => {
    vi.resetModules();
    api = {
      nativeGameSettings: vi.fn(async () => ({
        executable: '',
        candidates: ['/g/game.exe', '/g/tools/setup.exe'],
        parameters: '-fullscreen',
        compat_tool: '',
      })),
      listCompatTools: vi.fn(async () => ({ tools: [{ name: 'GE-Proton' }], default_tool: 'GE-Proton' })),
      setNativeGameSettings: vi.fn(async () => {}),
      reloadNativeLaunch: vi.fn(async () => ({ message: 'Launch info reloaded', outcome: 'applied' })),
    };
    vi.doMock('../api', () => ({ api }));
  });

  afterEach(() => vi.doUnmock('../api'));

  it('loads the settings with the first candidate and the default compat tool', async () => {
    const { loadNativeSettings } = await import('./nativeSettingsStore');
    const loaded = await loadNativeSettings(4, true);
    expect(loaded.executable).toBe('/g/game.exe');
    expect(loaded.parameters).toBe('-fullscreen');
    expect(loaded.compat).toEqual({ tools: [{ name: 'GE-Proton' }], selected: 'GE-Proton' });
  });

  it('skips the compat tools when none apply, and reports a compat failure apart', async () => {
    const { loadNativeSettings } = await import('./nativeSettingsStore');
    expect((await loadNativeSettings(4, false)).compat).toBeNull();
    expect(api.listCompatTools).not.toHaveBeenCalled();
    api.listCompatTools.mockRejectedValueOnce(new Error('no tools'));
    expect((await loadNativeSettings(4, true)).compat).toEqual({ error: 'no tools' });
  });

  it('throws when the settings cannot be read', async () => {
    api.nativeGameSettings.mockRejectedValueOnce(new Error('gone'));
    const { loadNativeSettings } = await import('./nativeSettingsStore');
    await expect(loadNativeSettings(4, true)).rejects.toThrow('gone');
  });

  it('saves the form', async () => {
    const { saveNativeSettings } = await import('./nativeSettingsStore');
    await saveNativeSettings(4, '/g/game.exe', '', 'GE-Proton');
    expect(api.setNativeGameSettings).toHaveBeenCalledWith(4, '/g/game.exe', '', 'GE-Proton');
  });

  it('reads a reload answer, a refusal and a failed call', async () => {
    const { reloadFromServer } = await import('./nativeSettingsStore');
    expect(await reloadFromServer(4)).toEqual({ message: 'Launch info reloaded', failed: false, answered: true });
    api.reloadNativeLaunch.mockResolvedValueOnce({ message: 'game.json is invalid', outcome: 'invalid' });
    expect(await reloadFromServer(4)).toEqual({ message: 'game.json is invalid', failed: true, answered: true });
    api.reloadNativeLaunch.mockRejectedValueOnce(new Error('offline'));
    expect(await reloadFromServer(4)).toEqual({ message: 'offline', failed: true, answered: false });
  });
});
