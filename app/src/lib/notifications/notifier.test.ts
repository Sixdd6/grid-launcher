import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry, GameSession, SessionsSnapshot } from '../api';

// The wiring: each store's snapshots go through the diff and the coalescer
// once, the setup card silences emulator cards, and hints change wording.

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Zelda',
    platform: 'SNES',
    status: 'queued',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    detail: '',
    drive_id: '',
    drive_name: '',
    drive_label: '',
    batch: 0,
    ...overrides,
  };
}

type Handler = (e: { payload: unknown }) => void;

describe('initNotifications', () => {
  let downloadsListener: ((entries: DownloadEntry[], first: boolean) => void) | null;
  let sessionsListener: ((before: GameSession[], snap: SessionsSnapshot) => void) | null;
  let handlers: Record<string, Handler>;
  let setupActive: boolean;
  let launch: (romId: number) => Promise<void>;

  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    downloadsListener = null;
    sessionsListener = null;
    handlers = {};
    setupActive = false;
    launch = async () => {};
    vi.doMock('@tauri-apps/api/event', () => ({
      listen: (name: string, handler: Handler) => {
        handlers[name] = handler;
        return Promise.resolve(() => {});
      },
    }));
    vi.doMock('../stores/downloads.svelte', () => ({
      onSnapshot: (fn: typeof downloadsListener) => {
        downloadsListener = fn;
        return () => (downloadsListener = null);
      },
    }));
    vi.doMock('../stores/sessions.svelte', () => ({
      onSessionsSnapshot: (fn: typeof sessionsListener) => {
        sessionsListener = fn;
        return () => (sessionsListener = null);
      },
    }));
    vi.doMock('../stores/setupRun.svelte', () => ({
      setupRun: {
        get active() {
          return setupActive;
        },
      },
    }));
    vi.doMock('../stores/emulatorUpdates.svelte', () => ({ markCurrent: () => {} }));
    vi.doMock('../api', () => ({
      EMULATOR_INSTALLED_EVENT: 'emulator-installed',
      api: { launchGame: (romId: number) => launch(romId), installGame: async () => {} },
    }));
  });

  afterEach(() => {
    vi.useRealTimers();
    for (const m of [
      '@tauri-apps/api/event',
      '../stores/downloads.svelte',
      '../stores/sessions.svelte',
      '../stores/setupRun.svelte',
      '../stores/emulatorUpdates.svelte',
      '../api',
    ])
      vi.doUnmock(m);
  });

  async function start() {
    const notifier = await import('./notifier');
    const { toasts } = await import('../stores/toasts.svelte');
    const stop = notifier.initNotifications();
    const texts = () => toasts.list.map((t) => t.text);
    return { notifier, stop, texts };
  }

  it('shows queue and install cards once, after a baseline', async () => {
    const { stop, texts } = await start();
    downloadsListener!([entry({ id: 9, status: 'completed', title: 'Old' })], true);
    downloadsListener!([entry({ id: 1 }), entry({ id: 9, status: 'completed', title: 'Old' })], false);
    await vi.advanceTimersByTimeAsync(1_000);
    downloadsListener!([entry({ id: 1, status: 'completed' })], false);
    downloadsListener!([entry({ id: 1, status: 'completed' })], false);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts()).toEqual(['Zelda added to download queue', 'Zelda installed']);
    stop();
  });

  it('summarises a batch and silences emulators while the setup card is up', async () => {
    const { stop, texts } = await start();
    downloadsListener!([], true);
    setupActive = true;
    downloadsListener!(
      [1, 2, 3].map((id) => entry({ id, title: `G${id}` })).concat(entry({ id: 4, job: 'emulator', kind: 'emulator', title: 'PCSX2' })),
      false,
    );
    handlers['emulator-installed']({ payload: { name: 'PCSX2', fresh: true } });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts()).toEqual(['3 games added to download queue']);
    setupActive = false;
    handlers['emulator-installed']({ payload: { name: 'PCSX2', fresh: false } });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts().at(-1)).toBe('PCSX2 updated');
    stop();
  });

  it('words a hinted emulator update', async () => {
    const { notifier, stop, texts } = await start();
    downloadsListener!([], true);
    notifier.expectQueued('PCSX2/pcsx2', { update: true });
    downloadsListener!([entry({ id: 5, job: 'emulator', kind: 'emulator', title: 'PCSX2', source_id: 'PCSX2/pcsx2' })], false);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts()).toEqual(['Updating PCSX2…']);
    stop();
  });

  it('announces a launch, reports its refusal and an early exit', async () => {
    const { notifier, stop, texts } = await start();
    launch = async () => {
      throw { code: '', message: 'Emulator executable not found: /x' };
    };
    await expect(notifier.launchGame(1, 'Zelda')).rejects.toThrow('Emulator executable not found: /x');
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts()).toEqual(['Launching Zelda…', "Couldn't launch Zelda: Emulator executable not found: /x"]);
    const s = { id: 3, rom_id: 1, title: 'Zelda', emulator_name: 'E', started_at: 0, pid: 1 };
    sessionsListener!([s], { sessions: [], warning: 'Process exited immediately (code 3).' });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(texts().at(-1)).toBe('Zelda — Process exited immediately (code 3).');
    stop();
  });
});
