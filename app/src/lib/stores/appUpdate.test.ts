import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// The backend emits `APP_UPDATE_EVENT` only for a newer release, so an
// up-to-date result recorded after the store's init pull reaches Settings ›
// Updates only through `refreshAppUpdateStatus` (final review P5-1).
describe('refreshAppUpdateStatus', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.doUnmock('@tauri-apps/api/event');
  });

  it('picks up checked_at from an up-to-date result and keeps a dismissed badge dismissed', async () => {
    const answers = [
      { notice: null, checked_at: null },
      { notice: null, checked_at: '2026-09-04T10:00:00Z' },
    ];
    let call = 0;
    vi.doMock('../api', () => ({
      APP_UPDATE_EVENT: 'app-update-available',
      api: { appUpdateNotice: () => Promise.resolve(answers[Math.min(call++, answers.length - 1)]) },
    }));
    vi.doMock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));

    const { appUpdate, dismiss, initAppUpdate, refreshAppUpdateStatus } = await import('./appUpdate.svelte');

    await initAppUpdate();
    expect(appUpdate.checkedAt).toBeNull();

    dismiss();
    await refreshAppUpdateStatus();

    expect(appUpdate.checkedAt).toBe('2026-09-04T10:00:00Z');
    // Dismissal survives the refresh, and no notice was invented.
    expect(appUpdate.stored).toBeNull();
    expect(appUpdate.notice).toBeNull();
  });

  it('swallows a failed pull and leaves the stored state alone', async () => {
    vi.doMock('../api', () => ({
      APP_UPDATE_EVENT: 'app-update-available',
      api: { appUpdateNotice: () => Promise.reject(new Error('offline')) },
    }));
    vi.doMock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));

    const { appUpdate, refreshAppUpdateStatus } = await import('./appUpdate.svelte');

    await expect(refreshAppUpdateStatus()).resolves.toBeUndefined();
    expect(appUpdate.checkedAt).toBeNull();
  });
});
