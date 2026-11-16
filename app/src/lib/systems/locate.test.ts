import { describe, expect, it } from 'vitest';
import { checkingLine, checkingTitle, folderOf, isArchiveName, reportView, type LocateAnswer } from './locate';

const answer: LocateAnswer = {
  installed: [
    { file: 'scph5501 (1).bin', as_name: 'scph5501.bin' },
    { file: 'bios-pack.zip', as_name: 'scph5502.bin' },
    { file: 'scph5500.bin', as_name: 'scph5500.bin' },
  ],
  unverified: [{ file: 'prod.keys', as_name: 'prod.keys' }],
  refused: [{ file: 'scph1001.bin', reason: "GRID doesn't know this file for PlayStation" }],
  unmatched: 1,
};

const base = { system: 'PlayStation', emulator: 'DuckStation (Playstation 1)', folder: '/home/player/Downloads/bios', picked: 5 };

describe('the Locate report', () => {
  it('lists installed (renamed, archive member, same name), not verified and refused', () => {
    const v = reportView({ ...base, mode: 'files', answer });
    expect(v.title).toBe('PlayStation firmware');
    expect(v.lede).toBe('5 files from /home/player/Downloads/bios. Installed into DuckStation’s folders.');
    expect(v.sections.map((s) => s.title)).toEqual(['Installed · 3', 'Installed · not verified · 1', 'Not installed · 1']);
    expect(v.sections[0].items).toEqual([
      {
        file: 'scph5501 (1).bin',
        as: 'scph5501.bin',
        from: null,
        note: 'Size and MD5 match; installed under the name DuckStation looks for',
      },
      { file: 'scph5502.bin', as: null, from: 'bios-pack.zip', note: 'Size and MD5 match' },
      { file: 'scph5500.bin', as: null, from: null, note: 'Size and MD5 match' },
    ]);
    expect(v.sections[1].items[0].note).toBe(
      'GRID has no checksum for this file, so it can’t check it. If games don’t start, dump it from your console again.',
    );
    expect(v.unmatched).toBe('1 other file in the folder matched nothing.');
    expect(v.nothing).toBe(false);
    expect(v.card).toEqual({ title: '4 firmware files installed for DuckStation', text: '1 not verified · 1 refused', error: false });
  });

  it('a folder search, and nothing installed', () => {
    const none = reportView({ ...base, mode: 'folder', answer: { installed: [], unverified: [], refused: [] } });
    expect(none.lede).toBe('Searched /home/player/Downloads/bios and the folders in it. Nothing was installed.');
    expect(none.nothing).toBe(true);
    expect(none.sections).toEqual([]);
    expect(none.card).toEqual({ title: 'No firmware installed', text: 'Nothing matched PlayStation firmware', error: true });
  });

  it('the PS3 PUP', () => {
    const pup = reportView({
      system: 'PlayStation 3',
      emulator: 'RPCS3 (Playstation 3)',
      mode: 'pup',
      folder: '/home/player/Downloads',
      picked: 1,
      answer: { installed: [{ file: 'PS3UPDAT.PUP', as_name: 'PS3UPDAT.PUP' }], unverified: [], refused: [] },
      pupVersion: '4.92',
    });
    expect(pup.title).toBe('PS3 system software');
    expect(pup.sections[0].items[0].note).toBe('RPCS3 installed PS3 system software 4.92');
    const bad = reportView({
      system: 'PlayStation 3',
      emulator: 'RPCS3',
      mode: 'pup',
      folder: '/x',
      picked: 1,
      answer: { installed: [], unverified: [], refused: [{ file: 'PS3UPDAT.PUP', reason: 'This PS3UPDAT.PUP isn’t the one GRID knows (size or MD5 differs)' }] },
    });
    expect(bad.sections[0].title).toBe('Not installed · 1');
  });

  it('the checking words and helpers', () => {
    expect(checkingTitle(5)).toBe('Checking 5 files…');
    expect(checkingTitle(1)).toBe('Checking 1 file…');
    expect(checkingLine('DuckStation (Playstation 1)')).toBe('Comparing sizes and MD5s with the files DuckStation needs');
    expect(isArchiveName('a.ZIP')).toBe(true);
    expect(isArchiveName('a.bin')).toBe(false);
    expect(folderOf('/a/b/c.bin')).toBe('/a/b');
  });
});
