import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

describe('outcomeToasts', () => {
  it('speaks only for a RetroArch pick that found no core', async () => {
    const { outcomeToasts } = await import('./events');
    expect(
      outcomeToasts([
        { outcome: 'assigned', platform: 'PlayStation 2', emulator: 'PCSX2', core: null },
        { outcome: 'core_missing', platform: 'SNES', emulator: 'RetroArch' },
        { outcome: 'kept_none', platform: 'Saturn' },
      ]),
    ).toEqual(['RetroArch installed, but it has no core for SNES, so SNES still has no emulator.']);
  });
});

describe('initEmulatorEvents', () => {
  type Handler = (e: { payload: unknown }) => void;
  let handlers: Record<string, Handler>;

  beforeEach(() => {
    vi.resetModules();
    handlers = {};
    vi.doMock('@tauri-apps/api/event', () => ({
      listen: (name: string, handler: Handler) => {
        handlers[name] = handler;
        return Promise.resolve(() => {});
      },
    }));
  });

  afterEach(() => {
    vi.doUnmock('@tauri-apps/api/event');
  });

  it('toasts each install once and marks an update current', async () => {
    const { initEmulatorEvents } = await import('./events');
    const { toasts } = await import('../stores/toasts.svelte');
    const stop = initEmulatorEvents();
    handlers['emulator-installed']({ payload: { name: 'PCSX2', fresh: true } });
    expect(toasts.list.map((t) => t.text)).toEqual(["Installed emulator 'PCSX2' from source."]);
    handlers['setup-defaults']({ payload: [{ outcome: 'core_missing', platform: 'SNES', emulator: 'RetroArch' }] });
    expect(toasts.list.at(-1)?.level).toBe('error');
    stop();
  });
});
