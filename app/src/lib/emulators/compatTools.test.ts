import { describe, expect, it } from 'vitest';
import type { CompatTool, DownloadEntry } from '../api';
import {
  compatToolLabel,
  compatToolTerminalSignature,
  groupCompatTools,
  isWindowsHost,
  liveCompatToolSourceIds,
} from './compatTools';

function tool(overrides: Partial<CompatTool>): CompatTool {
  return { name: 'Tool', kind: 'wine', path: '/opt/tool', source: 'system', ...overrides };
}

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'emulator',
    kind: 'compat_tool',
    rom_id: 0,
    source_id: 'ge-proton/GE-Proton8-25',
    title: 'GE-Proton8-25',
    platform: '',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

describe('groupCompatTools', () => {
  it('puts wine-kind tools in a Wine group', () => {
    const wine = tool({ name: 'Wine (system)', kind: 'wine', source: 'system', path: '/usr/bin/wine' });
    expect(groupCompatTools([wine])).toEqual([{ title: 'Wine', tools: [wine] }]);
  });

  it('puts proton-kind + steam-source tools in a Proton (system) group', () => {
    const proton = tool({ name: 'Proton 8.0', kind: 'proton', source: 'steam', path: '/steam/proton8' });
    expect(groupCompatTools([proton])).toEqual([{ title: 'Proton (system)', tools: [proton] }]);
  });

  it('puts managed-source proton tools in a Managed group', () => {
    const managed = tool({ name: 'GE-Proton8-25', kind: 'proton', source: 'managed', path: '/managed/ge-proton' });
    expect(groupCompatTools([managed])).toEqual([{ title: 'Managed', tools: [managed] }]);
  });

  it('orders non-empty groups as Wine, Proton (system), Managed and omits empty ones', () => {
    const wine = tool({ name: 'Wine (system)', kind: 'wine', source: 'system', path: '/usr/bin/wine' });
    const proton = tool({ name: 'Proton 8.0', kind: 'proton', source: 'steam', path: '/steam/proton8' });
    const managed = tool({ name: 'GE-Proton8-25', kind: 'proton', source: 'managed', path: '/managed/ge-proton' });
    expect(groupCompatTools([managed, wine, proton])).toEqual([
      { title: 'Wine', tools: [wine] },
      { title: 'Proton (system)', tools: [proton] },
      { title: 'Managed', tools: [managed] },
    ]);
  });

  it('omits the Proton (system) group entirely when there are no steam-source proton tools', () => {
    const wine = tool({ name: 'Wine (system)', kind: 'wine', source: 'system', path: '/usr/bin/wine' });
    const managed = tool({ name: 'GE-Proton8-25', kind: 'proton', source: 'managed', path: '/managed/ge-proton' });
    expect(groupCompatTools([wine, managed])).toEqual([
      { title: 'Wine', tools: [wine] },
      { title: 'Managed', tools: [managed] },
    ]);
  });

  it('returns an empty array for an empty input', () => {
    expect(groupCompatTools([])).toEqual([]);
  });

  it('groups multiple tools of the same kind together in list order', () => {
    const managedA = tool({ name: 'GE-Proton8-25', kind: 'proton', source: 'managed', path: '/managed/a' });
    const managedB = tool({ name: 'GE-Proton9-1', kind: 'proton', source: 'managed', path: '/managed/b' });
    expect(groupCompatTools([managedA, managedB])).toEqual([
      { title: 'Managed', tools: [managedA, managedB] },
    ]);
  });
});

describe('compatToolLabel', () => {
  it('formats a wine tool as "<name> — <path>" (backend already names it "Wine (system)")', () => {
    const wine = tool({ name: 'Wine (system)', kind: 'wine', source: 'system', path: '/usr/bin/wine' });
    expect(compatToolLabel(wine)).toBe('Wine (system) — /usr/bin/wine');
  });

  it('formats a steam-source tool as "<name> (system) — <path>"', () => {
    const proton = tool({ name: 'Proton 8.0', kind: 'proton', source: 'steam', path: '/steam/proton8' });
    expect(compatToolLabel(proton)).toBe('Proton 8.0 (system) — /steam/proton8');
  });

  it('formats a managed tool as "<name> — <path>"', () => {
    const managed = tool({ name: 'GE-Proton8-25', kind: 'proton', source: 'managed', path: '/managed/ge-proton' });
    expect(compatToolLabel(managed)).toBe('GE-Proton8-25 — /managed/ge-proton');
  });
});

describe('isWindowsHost (re-exported from details/actions, not redefined)', () => {
  it('is true for Win32', () => {
    expect(isWindowsHost('Win32')).toBe(true);
  });

  it('is false for a non-Windows platform', () => {
    expect(isWindowsHost('Linux x86_64')).toBe(false);
  });
});

describe('compatToolTerminalSignature', () => {
  it('is empty with no entries', () => {
    expect(compatToolTerminalSignature([])).toBe('');
  });

  it('is empty when the only compat_tool entries are still live', () => {
    const entries = [entry({ id: 1, status: 'downloading' }), entry({ id: 2, status: 'queued' })];
    expect(compatToolTerminalSignature(entries)).toBe('');
  });

  it('ignores terminal entries of a different kind', () => {
    const entries = [entry({ id: 1, kind: 'emulator', status: 'completed' })];
    expect(compatToolTerminalSignature(entries)).toBe('');
  });

  it('includes a completed compat_tool entry', () => {
    const entries = [entry({ id: 1, status: 'completed' })];
    expect(compatToolTerminalSignature(entries)).toBe('1:completed');
  });

  it('includes failed and cancelled compat_tool entries', () => {
    const entries = [entry({ id: 1, status: 'failed' }), entry({ id: 2, status: 'cancelled' })];
    expect(compatToolTerminalSignature(entries)).toBe('1:failed,2:cancelled');
  });

  it('changes when a live entry transitions to a terminal status', () => {
    const before = compatToolTerminalSignature([entry({ id: 1, status: 'installing' })]);
    const after = compatToolTerminalSignature([entry({ id: 1, status: 'completed' })]);
    expect(before).toBe('');
    expect(after).toBe('1:completed');
    expect(before).not.toBe(after);
  });

  it('mixes terminal compat_tool entries with live and other-kind entries, keeping only terminal compat_tool ones', () => {
    const entries = [
      entry({ id: 1, status: 'completed' }),
      entry({ id: 2, status: 'downloading' }),
      entry({ id: 3, kind: 'emulator', status: 'failed' }),
    ];
    expect(compatToolTerminalSignature(entries)).toBe('1:completed');
  });
});

describe('liveCompatToolSourceIds', () => {
  it('is empty with no entries', () => {
    expect(liveCompatToolSourceIds([])).toEqual(new Set());
  });

  it('includes queued, downloading, installing and cancelling compat_tool source_ids', () => {
    const entries = [
      entry({ id: 1, source_id: 'a', status: 'queued' }),
      entry({ id: 2, source_id: 'b', status: 'downloading' }),
      entry({ id: 3, source_id: 'c', status: 'installing' }),
      entry({ id: 4, source_id: 'd', status: 'cancelling' }),
    ];
    expect(liveCompatToolSourceIds(entries)).toEqual(new Set(['a', 'b', 'c', 'd']));
  });

  it('excludes terminal compat_tool entries', () => {
    const entries = [
      entry({ id: 1, source_id: 'a', status: 'completed' }),
      entry({ id: 2, source_id: 'b', status: 'failed' }),
      entry({ id: 3, source_id: 'c', status: 'cancelled' }),
    ];
    expect(liveCompatToolSourceIds(entries)).toEqual(new Set());
  });

  it('excludes live entries of a different kind', () => {
    const entries = [entry({ id: 1, source_id: 'a', kind: 'emulator', status: 'downloading' })];
    expect(liveCompatToolSourceIds(entries)).toEqual(new Set());
  });

  it('deduplicates a source_id that appears in more than one live entry', () => {
    const entries = [
      entry({ id: 1, source_id: 'a', status: 'queued' }),
      entry({ id: 2, source_id: 'a', status: 'downloading' }),
    ];
    expect(liveCompatToolSourceIds(entries)).toEqual(new Set(['a']));
  });
});
