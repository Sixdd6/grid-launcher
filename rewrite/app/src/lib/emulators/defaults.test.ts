import { describe, expect, it } from 'vitest';
import type { LaunchDefaults } from '../api';
import { NO_DEFAULT_VALUE, platformDefaultSelect } from './defaults';

function launchDefaults(entries: Record<string, string>): LaunchDefaults {
  return { default_emulators: entries, retroarch_cores: {}, launch_args: '' };
}

describe('platformDefaultSelect', () => {
  it('offers only the compatible names, in the order given', () => {
    const result = platformDefaultSelect(launchDefaults({}), 'SNES', ['Snes9x', 'RetroArch']);
    expect(result.options).toEqual(['Snes9x', 'RetroArch']);
    expect(result.disabled).toBe(false);
  });

  it('a saved default that is still compatible stays selected', () => {
    const defaults = launchDefaults({ SNES: 'RetroArch' });
    expect(platformDefaultSelect(defaults, 'SNES', ['Snes9x', 'RetroArch']).selected).toBe(
      'RetroArch'
    );
  });

  it('a saved default that is not compatible falls back to the first compatible name', () => {
    const defaults = launchDefaults({ SNES: 'PCSX2' });
    expect(platformDefaultSelect(defaults, 'SNES', ['Snes9x', 'RetroArch']).selected).toBe('Snes9x');
  });

  it('no saved default selects the first compatible name', () => {
    expect(platformDefaultSelect(launchDefaults({}), 'SNES', ['Snes9x']).selected).toBe('Snes9x');
  });

  it('no compatible emulator yields an empty, disabled select', () => {
    const result = platformDefaultSelect(launchDefaults({ SNES: 'PCSX2' }), 'SNES', []);
    expect(result.options).toEqual([]);
    expect(result.selected).toBe(NO_DEFAULT_VALUE);
    expect(result.disabled).toBe(true);
  });

  it('no defaults loaded yet still selects the first compatible name', () => {
    expect(platformDefaultSelect(null, 'SNES', ['Snes9x']).selected).toBe('Snes9x');
  });

  it('the platform key lookup is case-insensitive', () => {
    const defaults = launchDefaults({ snes: 'RetroArch' });
    expect(platformDefaultSelect(defaults, 'SNES', ['Snes9x', 'RetroArch']).selected).toBe(
      'RetroArch'
    );
  });

  it('a saved default differing only in case is not a match and falls back', () => {
    const defaults = launchDefaults({ SNES: 'retroarch' });
    expect(platformDefaultSelect(defaults, 'SNES', ['Snes9x', 'RetroArch']).selected).toBe('Snes9x');
  });
});
