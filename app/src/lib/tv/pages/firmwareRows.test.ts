import { describe, expect, it } from 'vitest';
import type { FirmwareExpectedFile } from '../../api';
import { firmwareBlock } from '../../systems/firmwareBlock';
import type { ActionRow } from '../rows';
import { fileSub, fileValue, firmwareNote, firmwareRows, rommDimmed, sizeText, type FirmwareRowsInput } from './firmwareRows';

const file = (name: string, over: Partial<FirmwareExpectedFile> = {}): FirmwareExpectedFile => ({
  name,
  size: 524288,
  md5: 'a'.repeat(32),
  required: true,
  installed: false,
  verified_by: 'grid',
  ...over,
});

function input(over: Partial<FirmwareRowsInput> & { onServer?: boolean } = {}): FirmwareRowsInput {
  const files = over.files ?? [file('scph5501.bin'), file('scph5502.bin', { required: false })];
  const connection = over.connection ?? 'not_connected';
  const emulator = over.emulator ?? 'DuckStation (Playstation 1)';
  return {
    system: 'PlayStation',
    emulator,
    host: 'romm.home.lan',
    connection,
    files,
    view: firmwareBlock({
      system: 'PlayStation',
      slug: 'psx',
      emulator,
      files,
      connection,
      onServer: over.onServer ?? false,
      serverFirmware: null,
      installing: false,
    }),
    ...over,
  };
}

const actions = { romm: () => {}, locate: () => {} };

describe('the TV firmware block', () => {
  it('one row per file, then the note, then the two actions', () => {
    const rows = firmwareRows(input(), actions);
    expect(rows.map((r) => r.key)).toEqual(['fw-file-scph5501.bin', 'fw-file-scph5502.bin', 'fw-note', 'fw-romm', 'fw-locate']);
    expect(rows[0]).toMatchObject({ label: 'scph5501.bin', sub: '512 KB · required', value: 'Missing · required' });
    expect(rows[1]).toMatchObject({ sub: '512 KB · optional', value: 'Missing' });
  });

  it('file lines once installed, and key files', () => {
    expect(fileSub(file('a', { installed: true, verified_by: 'romm' }))).toBe('512 KB · required · checked against RomM');
    expect(fileSub(file('a', { installed: true }))).toBe('512 KB · required · checked by GRID');
    expect(fileSub(file('prod.keys', { md5: '', size: 0 }))).toBe("Different on every console, so GRID can't check it");
    expect(fileValue(file('a', { installed: true }))).toBe('Installed');
    expect(fileValue(file('prod.keys', { md5: '', installed: true, verified_by: 'none' }))).toBe('Installed · not verified');
    expect(sizeText(4 * 1024 * 1024)).toBe('4 MB');
    expect(sizeText(0)).toBe('');
  });

  it('notes', () => {
    expect(firmwareNote(input())).toBe('GRID checks each file’s size and MD5 before it goes into DuckStation’s folders.');
    expect(firmwareNote(input({ emulator: '' }))).toBe('Install an emulator for PlayStation first. GRID puts firmware in its folders.');
    expect(firmwareNote(input({ files: [file('prod.keys', { md5: '' })] }))).toContain('Key files are different on every console');
  });

  it('Install from RomM is dimmed with why, by RomM state', () => {
    expect(rommDimmed(input())).toEqual({
      value: 'Needs RomM',
      why: 'Install from RomM needs RomM. Connect RomM in Settings › Game stores.',
    });
    expect(rommDimmed(input({ connection: 'expired' }))?.why).toBe('Your RomM sign-in expired. Sign in again in Settings › Game stores.');
    expect(rommDimmed(input({ connection: 'connected', onServer: false }))).toEqual({
      value: 'Not on your RomM',
      why: 'PlayStation isn’t on your RomM server.',
    });
    expect(rommDimmed(input({ connection: 'connected', onServer: true }))).toBeNull();
    expect(rommDimmed(input({ connection: 'connected', onServer: true, files: [file('a', { installed: true })] }))?.value).toBe(
      'Every file is installed',
    );
    const romm = firmwareRows(input(), actions).find((r) => r.key === 'fw-romm') as ActionRow;
    expect(romm).toMatchObject({ disabled: true, value: 'Needs RomM' });
  });

  it('no emulator: Locate files… is dimmed too', () => {
    const locate = firmwareRows(input({ emulator: '' }), actions).find((r) => r.key === 'fw-locate') as ActionRow;
    expect(locate).toMatchObject({ disabled: true, value: 'Needs an emulator first' });
  });
});
