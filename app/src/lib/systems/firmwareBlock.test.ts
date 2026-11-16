import { describe, expect, it } from 'vitest';
import type { FirmwareExpectedFile } from '../api';
import { firmwareBlock, locateLede, locateTitle, type FirmwareBlockInput } from './firmwareBlock';

const file = (name: string, over: Partial<FirmwareExpectedFile> = {}): FirmwareExpectedFile => ({
  name,
  size: 524288,
  md5: 'a'.repeat(32),
  required: true,
  installed: false,
  verified_by: 'grid',
  region: '',
  ...over,
});

const input = (over: Partial<FirmwareBlockInput> = {}): FirmwareBlockInput => ({
  system: 'PlayStation',
  slug: 'psx',
  emulator: 'DuckStation (Playstation 1)',
  files: [file('scph5500.bin'), file('scph5501.bin', { installed: true }), file('scph5502.bin', { required: false })],
  connection: 'connected',
  onServer: true,
  serverFirmware: true,
  installing: false,
  ...over,
});

describe('the firmware block', () => {
  it('counts the files and what is installed', () => {
    expect(firmwareBlock(input()).count).toBe('3 files · 1 installed');
    expect(firmwareBlock(input({ files: [file('a', { installed: true })] })).count).toBe('1 file · all installed');
  });

  it('Install from RomM is primary while a required file is missing and RomM can install it', () => {
    const v = firmwareBlock(input());
    expect(v.romm).toMatchObject({ label: 'Install from RomM', disabled: false, primary: true, why: null });
    expect(v.locate).toMatchObject({ label: 'Locate files…', disabled: false, primary: false });
    expect(v.reason).toBeNull();
    expect(v.needed).toBe(true);
  });

  it('normal when nothing required is missing', () => {
    const v = firmwareBlock(input({ files: [file('a', { installed: true }), file('b', { required: false })] }));
    expect(v.romm).toMatchObject({ disabled: false, primary: false });
    expect(v.needed).toBe(false);
  });

  it('Installing… while a pass runs', () => {
    const v = firmwareBlock(input({ installing: true }));
    expect(v.romm).toMatchObject({ label: 'Installing…', disabled: true });
    expect(v.locate.disabled).toBe(true);
  });

  it('needs RomM while it is off: Locate files… becomes primary', () => {
    for (const connection of ['not_connected', 'unreachable', 'expired'] as const) {
      const v = firmwareBlock(input({ connection, onServer: false, serverFirmware: null }));
      expect(v.romm).toMatchObject({ disabled: true, primary: false, why: 'needs-romm', title: 'Install from RomM needs RomM' });
      expect(v.locate.primary).toBe(true);
      expect(v.reason).toBe('needs-romm');
    }
  });

  it('not on the server, or the server has no firmware for it', () => {
    expect(firmwareBlock(input({ onServer: false })).romm).toMatchObject({ disabled: true, why: 'not-on-server' });
    expect(firmwareBlock(input({ onServer: false })).reason).toBe('not-on-server');
    expect(firmwareBlock(input({ serverFirmware: false })).romm.why).toBe('no-server-firmware');
  });

  it('no emulator: both disabled, and the reason', () => {
    const v = firmwareBlock(input({ emulator: '' }));
    expect(v.romm.disabled).toBe(true);
    expect(v.locate.disabled).toBe(true);
    expect(v.reason).toBe('no-emulator');
    expect(v.needed).toBe(false);
  });

  it('file lines: verified, not verified, missing, missing · optional, and key files', () => {
    const v = firmwareBlock(
      input({
        files: [
          file('a.bin', { installed: true }),
          file('prod.keys', { md5: '', size: 0, installed: true, verified_by: 'none' }),
          file('b.bin'),
          file('c.bin', { required: false }),
          file('scph5500.bin', { region: 'Japan' }),
          file('scph5502.bin', { required: false, region: 'Europe' }),
        ],
      }),
    );
    expect(v.files.map((f) => [f.name, f.meta, f.status])).toEqual([
      ['a.bin', 'Required', 'Installed · verified'],
      ['prod.keys', 'Required · no fixed checksum', 'Installed · not verified'],
      ['b.bin', 'Required', 'Missing'],
      ['scph5500.bin', 'Required · Japan', 'Missing'],
      ['c.bin', 'Optional', 'Missing · optional'],
      ['scph5502.bin', 'Optional · Europe', 'Missing · optional'],
    ]);
  });

  it('folds a long list of optional files', () => {
    const many = Array.from({ length: 10 }, (_, i) => file(`bios${i}.bin`, { required: false }));
    const v = firmwareBlock(input({ files: [file('main.bin'), ...many] }));
    expect(v.files).toHaveLength(5);
    expect(v.moreOptional).toHaveLength(6);
  });

  it('PlayStation 3: the PUP picker', () => {
    const v = firmwareBlock(input({ slug: 'ps3', files: [file('PS3UPDAT.PUP', { md5: '', size: 0, verified_by: 'none' })] }));
    expect(v.pup).toBe(true);
    expect(v.locate.label).toBe('Locate PS3UPDAT.PUP…');
  });

  it('the Locate dialog words', () => {
    expect(locateTitle('PlayStation', false)).toBe('Locate PlayStation firmware');
    expect(locateTitle('PlayStation 3', true)).toBe('Locate PS3UPDAT.PUP');
    expect(locateLede('DuckStation (Playstation 1)', false)).toBe(
      'Pick firmware files you have, or a folder. GRID checks each file’s size and MD5, then puts the ones it knows into DuckStation’s folders.',
    );
    expect(locateLede('RPCS3 (Playstation 3)', true)).toContain('then RPCS3 installs it.');
  });
});
