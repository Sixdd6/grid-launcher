import { describe, expect, it } from 'vitest';
import type { EmulatorEntry, LaunchDefaults } from '../api';
import { NO_DEFAULT_VALUE, resolveDefaultEmulatorValue } from './defaults';

function launchDefaults(entries: Record<string, string>): LaunchDefaults {
  return { default_emulators: entries, retroarch_cores: {}, launch_args: '' };
}

function emulator(name: string): EmulatorEntry {
  return { name, path: '/bin/x', args: '%rom%' };
}

describe('resolveDefaultEmulatorValue', () => {
  it('no defaults loaded yet resolves to the none value', () => {
    expect(resolveDefaultEmulatorValue(null, 'SNES', [])).toBe(NO_DEFAULT_VALUE);
  });

  it('no saved default for the platform resolves to the none value', () => {
    const defaults = launchDefaults({ NES: 'FCEUX' });
    expect(resolveDefaultEmulatorValue(defaults, 'SNES', [emulator('FCEUX')])).toBe(
      NO_DEFAULT_VALUE
    );
  });

  it('platform key lookup is case-insensitive', () => {
    const defaults = launchDefaults({ snes: 'Snes9x' });
    expect(resolveDefaultEmulatorValue(defaults, 'SNES', [emulator('Snes9x')])).toBe('Snes9x');
  });

  it('a saved default matching a configured emulator name verbatim resolves to it', () => {
    const defaults = launchDefaults({ SNES: 'Snes9x' });
    const emulators = [emulator('Snes9x'), emulator('RetroArch')];
    expect(resolveDefaultEmulatorValue(defaults, 'SNES', emulators)).toBe('Snes9x');
  });

  it('a saved default that differs only in case from every emulator name is not a match', () => {
    const defaults = launchDefaults({ SNES: 'snes9x' });
    expect(resolveDefaultEmulatorValue(defaults, 'SNES', [emulator('Snes9x')])).toBe(
      NO_DEFAULT_VALUE
    );
  });

  it('a saved default naming a deleted or renamed emulator resolves to the none value', () => {
    const defaults = launchDefaults({ SNES: 'Ghost Emulator' });
    expect(resolveDefaultEmulatorValue(defaults, 'SNES', [emulator('Snes9x')])).toBe(
      NO_DEFAULT_VALUE
    );
  });

  it('an emulator literally named "(none)" is only selected when it is the saved default', () => {
    const emulators = [emulator('(none)'), emulator('Snes9x')];
    expect(resolveDefaultEmulatorValue(launchDefaults({ SNES: '(none)' }), 'SNES', emulators)).toBe(
      '(none)'
    );
    expect(resolveDefaultEmulatorValue(launchDefaults({}), 'SNES', emulators)).toBe(
      NO_DEFAULT_VALUE
    );
  });
});
