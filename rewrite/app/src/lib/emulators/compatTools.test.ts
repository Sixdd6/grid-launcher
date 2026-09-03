import { describe, expect, it } from 'vitest';
import type { CompatTool } from '../api';
import { compatToolLabel, groupCompatTools, isWindowsHost } from './compatTools';

function tool(overrides: Partial<CompatTool>): CompatTool {
  return { name: 'Tool', kind: 'wine', path: '/opt/tool', source: 'system', ...overrides };
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
