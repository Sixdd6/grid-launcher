import { describe, expect, it } from 'vitest';
import { baseName, pickerHints, pickerSub, pickRowAction, pickRows, pickSize, picksLine, tvReport } from './filePicker';

describe('the TV file browser in file mode', () => {
  const listing = {
    path: '/run/media/USB-1TB/bios',
    parent: '/run/media/USB-1TB',
    entries: [
      { name: 'old', is_dir: true, size: 0 },
      { name: 'scph5501 (1).bin', is_dir: false, size: 524288 },
    ],
  };

  it('rows: up, folders, files', () => {
    expect(pickRows(listing)).toEqual([
      { kind: 'up', key: 'up', path: '/run/media/USB-1TB' },
      { kind: 'dir', key: 'd-old', name: 'old', path: '/run/media/USB-1TB/bios/old' },
      { kind: 'file', key: 'f-scph5501 (1).bin', name: 'scph5501 (1).bin', path: '/run/media/USB-1TB/bios/scph5501 (1).bin', size: '512 KB' },
    ]);
  });

  it('file sizes: KB, MB, GB, nothing for an empty file', () => {
    expect(pickSize(524288)).toBe('512 KB');
    expect(pickSize(197 * 1024 * 1024)).toBe('197 MB');
    expect(pickSize(4.4 * 1024 * 1024 * 1024)).toBe('4.4 GB');
    expect(pickSize(0)).toBe('');
  });

  it('A says Open, Pick, Unpick or Choose', () => {
    const [up, dir, f] = pickRows(listing);
    expect(pickRowAction(up, false, false)).toBe('Up');
    expect(pickRowAction(dir, false, false)).toBe('Open');
    expect(pickRowAction(f, false, false)).toBe('Pick');
    expect(pickRowAction(f, true, false)).toBe('Unpick');
    expect(pickRowAction(f, false, true)).toBe('Choose');
  });

  it('the picks line and the hints', () => {
    expect(picksLine([])).toBe('No files picked. Pick files with (A), or check this whole folder with (Y).');
    expect(picksLine(['/a/scph5501 (1).bin', '/b/readme.txt'])).toBe('2 files picked: scph5501 (1).bin, readme.txt');
    expect(pickerHints(0, false)).toEqual([{ button: 'Y', label: 'Check this folder' }]);
    expect(pickerHints(4, false)).toEqual([
      { button: 'Y', label: 'Check this folder' },
      { button: 'MENU', label: 'Check 4 files' },
    ]);
    expect(pickerHints(3, true)).toEqual([]);
    expect(pickerSub('DuckStation (Playstation 1)', false)).toBe(
      'For DuckStation. GRID checks each file and puts the ones it knows in DuckStation’s folders.',
    );
    expect(baseName('/a/b/PS3UPDAT.PUP')).toBe('PS3UPDAT.PUP');
  });
});

describe('the TV Locate report', () => {
  it('groups, counts and the card', () => {
    const r = tvReport({
      system: 'PlayStation',
      emulator: 'DuckStation (Playstation 1)',
      folder: false,
      where: '/run/media/USB-1TB/bios/scph5501 (1).bin',
      picked: 5,
      answer: {
        installed: [
          { file: 'scph5501 (1).bin', as_name: 'scph5501.bin' },
          { file: 'BIOS pack.zip', as_name: 'scph5502.bin' },
        ],
        unverified: [{ file: 'prod.keys', as_name: 'prod.keys' }],
        refused: [
          { file: 'scph1001.bin', reason: "This isn't the scph1001.bin GRID knows (size or MD5 differs)" },
          { file: 'readme.txt', reason: "GRID doesn't know this file for PlayStation" },
        ],
        unmatched: 0,
      },
    });
    expect(r.title).toBe('Firmware check');
    expect(r.sub).toBe('Checked 5 files you picked for PlayStation · DuckStation');
    expect(r.groups.map((g) => g.title)).toEqual(['Installed · 2', 'Installed · not verified · 1', 'Refused · 2']);
    expect(r.groups[0].items[0]).toMatchObject({ label: 'scph5501.bin', sub: 'From scph5501 (1).bin · size and MD5 match', pill: 'Installed' });
    expect(r.groups[0].items[1].sub).toBe('From BIOS pack.zip · size and MD5 match');
    expect(r.groups[1].items[0]).toMatchObject({ sub: 'GRID can’t check this file: it’s different on every console', pill: 'Installed · not verified' });
    expect(r.groups[2].items[0].pill).toBe('Refused');
    expect(r.card).toEqual({ title: '3 firmware files installed for DuckStation', text: '1 not verified · 2 refused', error: false });
  });

  it('a folder with nothing installed', () => {
    const r = tvReport({
      system: 'Nintendo Switch',
      emulator: 'Eden (Nintendo Switch)',
      folder: true,
      where: '/run/media/USB-1TB/keys',
      picked: 1,
      answer: { installed: [], unverified: [], refused: [], unmatched: 10 },
    });
    expect(r.sub).toBe('Checked /run/media/USB-1TB/keys for Nintendo Switch · Eden');
    expect(r.unmatched).toBe('10 other files matched nothing GRID knows for Nintendo Switch.');
    expect(r.nothing).toBe('Nothing was installed. The Nintendo Switch page lists the files Eden needs.');
    expect(r.card).toEqual({ title: 'No firmware installed', text: 'Nothing matched Nintendo Switch firmware', error: true });
  });
});
