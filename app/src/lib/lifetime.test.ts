import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Handler = (e: { payload: unknown }) => void;

describe('startShellLifetime', () => {
  let handlers: Record<string, Handler>;
  let unlistened: string[];
  let calls: string[];
  let toasts: [string, string][];

  beforeEach(() => {
    vi.resetModules();
    handlers = {};
    unlistened = [];
    calls = [];
    toasts = [];
    vi.doMock('@tauri-apps/api/event', () => ({
      listen: (name: string, handler: Handler) => {
        handlers[name] = handler;
        return Promise.resolve(() => unlistened.push(name));
      },
    }));
    vi.doMock('./notifications/notifier', () => ({
      initNotifications: () => {
        calls.push('notifications');
        return () => calls.push('notifications-stop');
      },
    }));
    vi.doMock('./systems/events', () => ({
      initEmulatorEvents: () => {
        calls.push('emulator-events');
        return () => calls.push('emulator-events-stop');
      },
    }));
    vi.doMock('./stores/installed.svelte', () => ({
      installed: { list: [{ title: 'Zelda' }] },
      refresh: () => {
        calls.push('refresh');
        return Promise.resolve();
      },
    }));
    vi.doMock('./stores/lastViewed.svelte', () => ({
      seedLastViewed: (rows: unknown[]) => calls.push(`seed:${rows.length}`),
    }));
    vi.doMock('./stores/toasts.svelte', () => ({
      pushToast: (text: string, level: string) => toasts.push([text, level]),
    }));
  });

  afterEach(() => {
    vi.doUnmock('@tauri-apps/api/event');
    vi.doUnmock('./notifications/notifier');
    vi.doUnmock('./systems/events');
    vi.doUnmock('./stores/installed.svelte');
    vi.doUnmock('./stores/lastViewed.svelte');
    vi.doUnmock('./stores/toasts.svelte');
  });

  it('starts every shell-lifetime listener once and stops them all', async () => {
    const { startShellLifetime } = await import('./lifetime.svelte');
    const stop = startShellLifetime();
    // Vitest compiles Svelte for the server, where effects never run, so the
    // background-art seed (an effect over the registry) is not observed here.
    expect(calls).toEqual(['refresh', 'emulator-events', 'notifications']);
    expect(Object.keys(handlers).sort()).toEqual(['cloud-restore-notice', 'cloud-upload-finished']);

    stop();
    await Promise.resolve();
    await Promise.resolve();
    expect(calls).toContain('notifications-stop');
    expect(calls).toContain('emulator-events-stop');
    expect(unlistened.sort()).toEqual(['cloud-restore-notice', 'cloud-upload-finished']);
  });

  it('turns the cloud events into cards', async () => {
    const { startShellLifetime } = await import('./lifetime.svelte');
    const stop = startShellLifetime();
    handlers['cloud-upload-finished']({ payload: { title: 'Zelda', message: 'Saves uploaded', failed: false } });
    handlers['cloud-upload-finished']({ payload: { title: '', message: 'Upload failed', failed: true } });
    handlers['cloud-restore-notice']({ payload: { message: 'Cloud save for Zelda skipped', failed: true } });
    expect(toasts).toEqual([
      ['Zelda — Saves uploaded', 'success'],
      ['Upload failed', 'error'],
      ['Cloud save for Zelda skipped', 'error'],
    ]);
    stop();
  });
});
