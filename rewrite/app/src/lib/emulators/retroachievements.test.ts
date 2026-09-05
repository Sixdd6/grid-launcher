import { describe, expect, it } from 'vitest';
import type { RaFanOutRow, RaStatus } from '../api';
import {
  canLogin,
  canSubmit,
  CREDENTIALS_CLEARED_TOAST,
  fanOutSummary,
  LOGIN_MISSING_FIELDS_TOAST,
  loginFailedToast,
  loginToast,
  statusLabel,
} from './retroachievements';

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

describe('canLogin', () => {
  it('needs both fields', () => {
    expect(canLogin('six', 'pw')).toBe(true);
    expect(canLogin('', 'pw')).toBe(false);
    expect(canLogin('six', '')).toBe(false);
    expect(canLogin('   ', '   ')).toBe(false);
  });

  it('does not trim the password, which may legitimately have spaces', () => {
    expect(canLogin('six', '  ')).toBe(true);
  });
});

describe('the RetroAchievements toast texts', () => {
  it('are the reference strings verbatim', () => {
    expect(LOGIN_MISSING_FIELDS_TOAST).toBe('Enter both username and password.');
    expect(loginToast('Sixdd6')).toBe('Logged in as Sixdd6');
    expect(loginFailedToast('Invalid credentials')).toBe('RA login failed: Invalid credentials');
    expect(CREDENTIALS_CLEARED_TOAST).toBe('RetroAchievements credentials cleared.');
  });
});
