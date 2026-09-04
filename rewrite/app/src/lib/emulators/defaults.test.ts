import { describe, expect, it } from 'vitest';
import type { LaunchDefaults } from '../api';
import {
  isRetroarchName,
  NO_CORE_VALUE,
  NO_DEFAULT_VALUE,
  platformCoreSelect,
  platformDefaultSelect,
} from './defaults';

function launchDefaults(
  entries: Record<string, string>,
  cores: Record<string, string> = {}
): LaunchDefaults {
  return { default_emulators: entries, retroarch_cores: cores, launch_args: '' };
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

describe('isRetroarchName', () => {
  it('matches any casing and any surrounding text', () => {
    expect(isRetroarchName('RetroArch')).toBe(true);
    expect(isRetroarchName('retroarch (multi-system)')).toBe(true);
    expect(isRetroarchName('My RETROARCH Build')).toBe(true);
  });

  it('does not match a different emulator', () => {
    expect(isRetroarchName('PCSX2')).toBe(false);
    expect(isRetroarchName('')).toBe(false);
  });
});

describe('platformCoreSelect', () => {
  it('is hidden when the row’s selected emulator is not RetroArch', () => {
    const result = platformCoreSelect(launchDefaults({}), 'SNES', 'Snes9x', ['snes9x']);
    expect(result.visible).toBe(false);
  });

  it('is visible for a RetroArch selection', () => {
    const result = platformCoreSelect(launchDefaults({}), 'SNES', 'RetroArch', ['snes9x']);
    expect(result.visible).toBe(true);
    expect(result.options).toEqual(['snes9x']);
    expect(result.disabled).toBe(false);
  });

  it('a saved core that is still installed stays selected', () => {
    const defaults = launchDefaults({}, { SNES: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x', 'bsnes']).selected).toBe(
      'bsnes'
    );
  });

  it('a saved core that is no longer installed falls back to the first option', () => {
    // D-RC-5: display-only fallback; nothing is rewritten.
    const defaults = launchDefaults({}, { SNES: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x']).selected).toBe('snes9x');
  });

  it('the platform key lookup is case-insensitive', () => {
    const defaults = launchDefaults({}, { snes: 'bsnes' });
    expect(platformCoreSelect(defaults, 'SNES', 'RetroArch', ['snes9x', 'bsnes']).selected).toBe(
      'bsnes'
    );
  });

  it('no installed core yields an empty, disabled select', () => {
    const result = platformCoreSelect(launchDefaults({}, { SNES: 'bsnes' }), 'SNES', 'RetroArch', []);
    expect(result.visible).toBe(true);
    expect(result.options).toEqual([]);
    expect(result.selected).toBe(NO_CORE_VALUE);
    expect(result.disabled).toBe(true);
  });
});
