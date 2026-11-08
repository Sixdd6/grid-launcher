import { describe, expect, it } from 'vitest';
import type { CatalogEntry, CompatTool, DownloadEntry } from '../api';
import {
  NO_PROTON_VALUE,
  protonDownloadLabel,
  protonInstalling,
  protonInstallSource,
  protonPicker,
  protonStatus,
  RECOMMENDED_PROTON_SOURCE,
} from './proton';

const tool = (name: string, kind: string, path: string, source: string): CompatTool => ({ name, kind, path, source });
const entry = (name: string, source_id: string, installed = false): CatalogEntry => ({
  name,
  source_id,
  provider: 'github-release',
  owner: source_id.split('/')[0],
  repo: source_id.split('/')[1],
  tag: 'latest',
  installed,
});

const wine = tool('Wine (system)', 'wine', 'wine', 'system');
const steam = tool('Proton 9.0', 'proton', '/steam/Proton 9.0', 'steam');
const managed = tool('GE-Proton10-17', 'proton', '/lib/GE-Proton10-17', 'managed');
const ge = entry('GE-Proton', RECOMMENDED_PROTON_SOURCE);
const cachy = entry('Proton-CachyOS', 'CachyOS/proton-cachyos');

describe('protonPicker', () => {
  it('lists system Wine, Steam, then GRID downloads; the catalog with GE-Proton first', () => {
    const picker = protonPicker([managed, steam, wine], [cachy, ge], '/steam/Proton 9.0');
    expect(picker.installed.map((o) => o.label)).toEqual([
      'Wine (system)',
      'Proton 9.0 (Steam)',
      'GE-Proton10-17 (downloaded by GRID)',
    ]);
    expect(picker.download.map((o) => o.label)).toEqual([
      'Install GE-Proton (latest, recommended)',
      'Install Proton-CachyOS (latest)',
    ]);
    expect(picker.download[0]).toMatchObject({ value: `install:${RECOMMENDED_PROTON_SOURCE}`, recommended: true });
    expect(picker.selected).toBe('/steam/Proton 9.0');
    expect(picker.unset).toBe(false);
  });

  it('leaves out catalog tools already installed and shows a placeholder while unset', () => {
    const picker = protonPicker([wine], [entry('GE-Proton', RECOMMENDED_PROTON_SOURCE, true), cachy], '');
    expect(picker.download.map((o) => o.value)).toEqual(['install:CachyOS/proton-cachyos']);
    expect(picker.selected).toBe(NO_PROTON_VALUE);
    expect(picker.unset).toBe(true);
  });

  it('treats a default that is no longer on the computer as unset', () => {
    expect(protonPicker([wine], [], '/gone/proton').unset).toBe(true);
  });
});

describe('the rest', () => {
  it('reads the install source of a value', () => {
    expect(protonInstallSource(`install:${RECOMMENDED_PROTON_SOURCE}`)).toBe(RECOMMENDED_PROTON_SOURCE);
    expect(protonInstallSource('wine')).toBeNull();
  });

  it('labels a catalog entry with no tag as latest', () => {
    expect(protonDownloadLabel({ ...cachy, tag: '' }, false)).toBe('Install Proton-CachyOS (latest)');
  });

  it('is installing only while a catalog compat-tool download runs', () => {
    const row = (source_id: string, status: DownloadEntry['status']) =>
      ({ id: 1, job: 'emulator', kind: 'compat_tool', source_id, status }) as unknown as DownloadEntry;
    expect(protonInstalling([row(RECOMMENDED_PROTON_SOURCE, 'downloading')], [ge])).toBe(true);
    expect(protonInstalling([row(RECOMMENDED_PROTON_SOURCE, 'completed')], [ge])).toBe(false);
    // umu-launcher shares the Downloads kind but is not a Proton version.
    expect(protonInstalling([row('Open-Wine-Components/umu-launcher', 'downloading')], [ge])).toBe(false);
  });

  it('says Ready once a default is set', () => {
    expect(protonStatus(false, false)).toBe('ready');
    expect(protonStatus(true, false)).toBe('none');
    expect(protonStatus(false, true)).toBe('installing');
  });
});
