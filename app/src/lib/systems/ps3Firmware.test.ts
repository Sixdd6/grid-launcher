import { describe, expect, it } from 'vitest';
import type { Rpcs3FirmwareStatus } from '../api';
import { statusOf } from './rows';
import { isRpcs3Name, pollUntilInstalled, ps3FirmwareInstalled, ps3FirmwareView, withPs3Firmware } from './ps3Firmware';

const status = (s: Partial<Rpcs3FirmwareStatus>): Rpcs3FirmwareStatus => ({
  pup_path: null,
  installed: false,
  version: null,
  ...s,
});

describe('ps3FirmwareView', () => {
  it('shows one installed line with the version and no Install prompt', () => {
    expect(ps3FirmwareView(status({ installed: true, version: '4.93', pup_path: '/rpcs3/PS3UPDAT.PUP' }))).toEqual({
      kind: 'installed',
      text: 'PS3 firmware 4.93 installed',
    });
  });

  it('says installed without a version when RPCS3 has none to report', () => {
    expect(ps3FirmwareView(status({ installed: true }))).toEqual({ kind: 'installed', text: 'PS3 firmware installed' });
    expect(ps3FirmwareView(status({ installed: true, version: ' ' }))?.text).toBe('PS3 firmware installed');
  });

  it('prompts Install only for a downloaded PUP RPCS3 has not installed', () => {
    expect(ps3FirmwareView(status({ pup_path: '/rpcs3/PS3UPDAT.PUP' }))?.kind).toBe('install');
  });

  it('shows nothing with neither a PUP nor installed firmware, or no answer yet', () => {
    expect(ps3FirmwareView(status({}))).toBeNull();
    expect(ps3FirmwareView(undefined)).toBeNull();
  });
});

describe('ps3FirmwareInstalled / isRpcs3Name', () => {
  it('reads the installed flag', () => {
    expect(ps3FirmwareInstalled(status({ installed: true }))).toBe(true);
    expect(ps3FirmwareInstalled(status({ pup_path: '/x' }))).toBe(false);
    expect(ps3FirmwareInstalled(undefined)).toBe(false);
  });

  it('matches RPCS3 entries by name', () => {
    expect(isRpcs3Name('RPCS3 (Playstation 3)')).toBe(true);
    expect(isRpcs3Name('PCSX2')).toBe(false);
  });
});

describe('withPs3Firmware', () => {
  it('turns a stale Install into installed, so the pill is not "Firmware needed"', () => {
    const action = withPs3Firmware('install', true);
    expect(action).toBe('installed');
    expect(statusOf({ emulator: 'RPCS3', installing: false, firmware: action, updateAvailable: false })).toBe('ready');
    expect(withPs3Firmware('unavailable', true)).toBe('installed');
  });

  it('leaves the action alone when RPCS3 has no firmware, nothing is offered, or a request is running', () => {
    expect(withPs3Firmware('install', false)).toBe('install');
    expect(withPs3Firmware(null, true)).toBeNull();
    expect(withPs3Firmware('requested', true)).toBe('requested');
  });
});

describe('pollUntilInstalled', () => {
  const sleep = () => Promise.resolve();

  it('stops as soon as the install lands', async () => {
    const answers = [false, false, true, true];
    let calls = 0;
    const seen = await pollUntilInstalled(async () => answers[calls++], { intervalMs: 1, attempts: 10, sleep });
    expect(seen).toBe(true);
    expect(calls).toBe(3);
  });

  it('gives up after its attempts, ignoring failed reads', async () => {
    let calls = 0;
    const seen = await pollUntilInstalled(
      async () => {
        calls++;
        throw new Error('offline');
      },
      { intervalMs: 1, attempts: 4, sleep },
    );
    expect(seen).toBe(false);
    expect(calls).toBe(4);
  });

  it('stops when the page goes away', async () => {
    let calls = 0;
    const seen = await pollUntilInstalled(async () => (calls++, false), {
      intervalMs: 1,
      attempts: 10,
      sleep,
      stopped: () => calls >= 2,
    });
    expect(seen).toBe(false);
    expect(calls).toBe(2);
  });
});
