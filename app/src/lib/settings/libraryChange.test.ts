import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import type { LibraryMoveStartOutcome, MovePreview, StartFreshOutcome } from '../api';

type Fn = ReturnType<typeof vi.fn>;

function preview(over: Partial<MovePreview> = {}): MovePreview {
  return {
    old_root: '/old',
    new_root: '/new',
    items: [{ name: 'Games', strategy: 'copy', bytes: 100 }] as never,
    total_bytes: 100,
    copy_bytes: 100,
    required_bytes: 110,
    free_bytes: 1000,
    ...over,
  };
}

describe('moveTone / moveHint', () => {
  it('reads the space from the preview', async () => {
    const { moveHint, moveTone } = await import('./libraryChange.svelte');
    expect(moveTone(null)).toBe('ok');
    expect(moveTone(preview({ copy_bytes: 0 }))).toBe('ok');
    expect(moveTone(preview({ free_bytes: null }))).toBe('warn');
    expect(moveTone(preview({ free_bytes: 100 }))).toBe('bad');
    expect(moveTone(preview({ free_bytes: 120 }))).toBe('warn');
    expect(moveTone(preview())).toBe('ok');
    expect(moveHint(null)).toBe('');
    expect(moveHint(preview({ copy_bytes: 0 }))).toBe('Each folder is renamed. Nothing is copied.');
    expect(moveHint(preview({ free_bytes: 100 }))).toMatch(/^The new drive does not have enough free space/);
    expect(moveHint(preview({ free_bytes: null }))).toBe('GRID could not read the free space on the new drive.');
    expect(moveHint(preview({ free_bytes: 120 }))).toBe('Space is tight: the new drive has little room to spare.');
    expect(moveHint(preview())).toMatch(/^Files are copied and checked first/);
  });
});

describe('LibraryChange', () => {
  let api: Record<string, Fn>;
  let events: {
    onDone: Mock<(outcome: StartFreshOutcome, deleted: boolean) => void>;
    onMoveStarted: Mock<(outcome: LibraryMoveStartOutcome) => void>;
    onClose: Mock<() => void>;
  };

  beforeEach(() => {
    vi.resetModules();
    api = {
      checkLibraryPath: vi.fn(async (p: string) => ({ status: 'ok', path: p })),
      libraryStartFreshKeep: vi.fn(async () => ({ status: 'switched' })),
      libraryStartFreshDelete: vi.fn(async () => ({ status: 'switched' })),
      libraryStartFreshPreview: vi.fn(async () => ({ games: [], total_bytes: 0 })),
      libraryMovePreview: vi.fn(async () => ({ status: 'ok', preview: preview() })),
      libraryMoveStart: vi.fn(async () => ({ status: 'started' })),
    };
    events = {
      onDone: vi.fn<(outcome: StartFreshOutcome, deleted: boolean) => void>(),
      onMoveStarted: vi.fn<(outcome: LibraryMoveStartOutcome) => void>(),
      onClose: vi.fn<() => void>(),
    };
    vi.doMock('../api', () => ({ api }));
  });

  afterEach(() => {
    vi.doUnmock('../api');
    vi.useRealTimers();
  });

  async function flow(current = '/old') {
    const { LibraryChange } = await import('./libraryChange.svelte');
    return new LibraryChange(() => current, events);
  }

  async function checked(current = '/old') {
    const lc = await flow(current);
    lc.typed = '/new';
    vi.useFakeTimers();
    lc.checkTyped();
    await vi.advanceTimersByTimeAsync(250);
    vi.useRealTimers();
    return lc;
  }

  it('checks the typed folder after the pause, and drops a stale answer', async () => {
    const lc = await flow();
    expect(lc.checkTyped()).toBeTypeOf('function');
    expect(lc.checking).toBe(false);
    lc.typed = '/new';
    vi.useFakeTimers();
    const stop = lc.checkTyped();
    expect(lc.checking).toBe(true);
    stop();
    await vi.advanceTimersByTimeAsync(250);
    expect(api.checkLibraryPath).not.toHaveBeenCalled();
    lc.checkTyped();
    await vi.advanceTimersByTimeAsync(250);
    expect(lc.check).toEqual({ status: 'ok', path: '/new' });
    expect(lc.checking).toBe(false);
    expect(lc.ready).toBe(true);
    expect(lc.moveReady).toBe(true);
  });

  it('Start fresh asks about the old files, then keeps or deletes them', async () => {
    const lc = await checked();
    lc.startFresh();
    expect(lc.step).toBe('fresh-ask');
    expect(lc.meta.eyebrow).toBe('Step 2 · Old games');
    await lc.askDelete();
    expect(lc.step).toBe('delete-confirm');
    expect(lc.preview).not.toBeNull();
    await lc.run(true);
    expect(api.libraryStartFreshDelete).toHaveBeenCalledWith('/new', '/old');
    expect(events.onDone).toHaveBeenCalledWith({ status: 'switched' }, true);
  });

  it('Start fresh with no library folder switches at once', async () => {
    const lc = await checked('');
    expect(lc.moveReady).toBe(false);
    lc.startFresh();
    await vi.waitFor(() => expect(events.onDone).toHaveBeenCalledWith({ status: 'switched' }, false));
    expect(lc.step).toBe('choose');
  });

  it('a move previews first, then starts; a refusal stays on the step', async () => {
    const lc = await checked();
    api.libraryMovePreview.mockResolvedValueOnce({ status: 'refused', message: 'Same folder' });
    await lc.askMove();
    expect(lc.step).toBe('move-confirm');
    expect(lc.error).toBe('Same folder');
    await lc.startMove();
    expect(api.libraryMoveStart).not.toHaveBeenCalled();
    lc.back();
    expect(lc.step).toBe('choose');
    await lc.askMove();
    expect(lc.moveHint).toMatch(/^Files are copied/);
    await lc.startMove();
    expect(events.onMoveStarted).toHaveBeenCalledWith({ status: 'started' });
  });

  it('back steps back, then closes; cancel waits for a running call', async () => {
    const lc = await checked();
    lc.startFresh();
    await lc.askDelete();
    lc.back();
    expect(lc.step).toBe('fresh-ask');
    lc.back();
    expect(lc.step).toBe('choose');
    lc.back();
    expect(events.onClose).toHaveBeenCalledTimes(1);
    lc.pending = true;
    lc.cancel();
    lc.back();
    expect(events.onClose).toHaveBeenCalledTimes(1);
  });
});
