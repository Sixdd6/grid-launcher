import { describe, expect, it } from 'vitest';
import type { RaFanOutRow, RaStatus } from '../api';
import { canSubmit, fanOutSummary, statusLabel } from './retroachievements';

describe('canSubmit', () => {
  it('requires both fields', () => {
    expect(canSubmit('', '')).toBe(false);
    expect(canSubmit('sixdd6', '')).toBe(false);
    expect(canSubmit('', 'FAKE-RA-TOKEN-not-real')).toBe(false);
    expect(canSubmit('sixdd6', 'FAKE-RA-TOKEN-not-real')).toBe(true);
  });

  it('whitespace-only fields do not count as filled', () => {
    expect(canSubmit('   ', 'FAKE-RA-TOKEN-not-real')).toBe(false);
    expect(canSubmit('sixdd6', '   ')).toBe(false);
  });
});

describe('statusLabel', () => {
  function status(username: string, tokenPresent: boolean): RaStatus {
    return { username, token_present: tokenPresent };
  }

  it('renders set and unset states', () => {
    expect(statusLabel(status('', false))).toBe('Not set');
    expect(statusLabel(null)).toBe('Not set');
    expect(statusLabel(status('sixdd6', true))).toBe('Set for sixdd6');
  });

  it('a username with no stored token still renders unset', () => {
    expect(statusLabel(status('sixdd6', false))).toBe('Not set');
  });
});

describe('fanOutSummary', () => {
  function row(emulator: string, changed: boolean): RaFanOutRow {
    return { emulator, changed };
  }

  it('lists only changed emulators', () => {
    const rows = [row('RetroArch', true), row('PCSX2', false), row('PPSSPP', true)];
    expect(fanOutSummary(rows)).toBe('Updated: RetroArch, PPSSPP');
  });

  it('reports no changes', () => {
    expect(fanOutSummary([row('RetroArch', false), row('PCSX2', false)])).toBe('No changes');
    expect(fanOutSummary([])).toBe('No changes');
  });
});
