import { describe, expect, it } from 'vitest';
import type { SystemRow } from '../systems/rows';
import { installsRecommended, noEmulatorFor, noEmulatorSub, noEmulatorText } from './noEmulator';

const sys = (over: Partial<SystemRow>): SystemRow => ({
  key: 'PlayStation 2',
  label: 'PlayStation 2',
  name: 'PlayStation 2',
  slug: 'ps2',
  romCount: 1,
  platform: null,
  emulator: '',
  explicitNone: false,
  installed: [],
  downloads: [],
  recommended: { source_id: 'pcsx2', name: 'PCSX2 (Playstation 2)', kind: 'standalone', installed: false } as SystemRow['recommended'],
  core: null,
  ...over,
});

describe('the no-emulator line both Details show (user decision 2026-11-10)', () => {
  it('names the recommendation Install brings', () => {
    const n = noEmulatorFor(sys({}), false)!;
    expect(noEmulatorText(n)).toBe('No emulator for PlayStation 2. Install installs PCSX2 too.');
    expect(installsRecommended(n)).toBe(true);
    expect(noEmulatorSub(n)).toBe('PCSX2 installs with it');
  });

  it('says the recommendation is installing, and Install then brings only the game', () => {
    const n = noEmulatorFor(sys({}), true)!;
    expect(noEmulatorText(n)).toBe('No emulator for PlayStation 2. PCSX2 is installing.');
    expect(installsRecommended(n)).toBe(false);
  });

  it('sends the player to Settings › Systems with no recommendation', () => {
    const n = noEmulatorFor(sys({ recommended: null }), false)!;
    expect(noEmulatorText(n)).toBe('No emulator for PlayStation 2. Add one in Settings › Systems.');
    expect(installsRecommended(n)).toBe(false);
    expect(noEmulatorSub(n)).toBeNull();
  });

  it('is silent for a system with an emulator or set to None', () => {
    expect(installsRecommended(noEmulatorFor(sys({ emulator: 'PCSX2' }), false))).toBe(false);
    expect(noEmulatorFor(sys({ explicitNone: true }), false)).toBeNull();
  });
});
