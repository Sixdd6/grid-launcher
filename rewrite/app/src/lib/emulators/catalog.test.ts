import { describe, expect, it } from 'vitest';
import type { ProfileSummary } from '../api';
import { matchProfileByName } from './catalog';

function profile(name: string, args = ''): ProfileSummary {
  return { name, args };
}

describe('matchProfileByName', () => {
  it('exact match beats a substring match on another profile', () => {
    const profiles = [profile('PCSX2', '%rom%'), profile('PCSX2 Turbo', '--turbo %rom%')];
    expect(matchProfileByName('PCSX2', profiles)).toEqual(profile('PCSX2', '%rom%'));
  });

  it('matches case-insensitively and trims surrounding whitespace', () => {
    const profiles = [profile('Dolphin', '%rom%')];
    expect(matchProfileByName('  dolphin  ', profiles)).toEqual(profile('Dolphin', '%rom%'));
  });

  it('falls back to a unique substring match when there is no exact match', () => {
    const profiles = [profile('Dolphin', '%rom%'), profile('RetroArch', '-L core %rom%')];
    expect(matchProfileByName('dolph', profiles)).toEqual(profile('Dolphin', '%rom%'));
  });

  it('an ambiguous substring match (more than one profile) returns null', () => {
    const profiles = [profile('PCSX2', '%rom%'), profile('PCSX2 Turbo', '--turbo %rom%')];
    expect(matchProfileByName('pcsx', profiles)).toBeNull();
  });

  it('an empty or whitespace-only name returns null', () => {
    const profiles = [profile('Dolphin', '%rom%')];
    expect(matchProfileByName('', profiles)).toBeNull();
    expect(matchProfileByName('   ', profiles)).toBeNull();
  });

  it('no match at all returns null', () => {
    const profiles = [profile('Dolphin', '%rom%')];
    expect(matchProfileByName('nonexistent', profiles)).toBeNull();
  });

  it('an empty profile list always returns null', () => {
    expect(matchProfileByName('Dolphin', [])).toBeNull();
  });
});
