import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry, GameSession } from '../api';
import {
  backupNoticeCard,
  coalesce,
  Coalescer,
  COALESCE_MAX_MS,
  COALESCE_WINDOW_MS,
  diffSnapshot,
  driveOutcomeNotice,
  driveQueuedNotice,
  earlyExitNotice,
  emulatorInstalledNotice,
  launchFailedNotice,
  launchingNotice,
  noticesFor,
  queueRefusedNotice,
  QuietQueued,
  QueuedSubs,
  summaryText,
  type Notice,
  type StatusBook,
} from './notices';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Chrono Trigger',
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
    phase: 'none',
    failure: null,
    ...overrides,
  };
}

/** Every card text a run of snapshots earns, starting from `initial`. */
function texts(snapshots: DownloadEntry[][], setupActive = false): string[] {
  let book: StatusBook | null = null;
  const out: string[] = [];
  for (const snap of snapshots) {
    const { book: next, transitions } = diffSnapshot(book, snap);
    book = next;
    for (const t of transitions) for (const n of noticesFor(t, { setupActive })) out.push(n.text);
  }
  return out;
}

describe('diffSnapshot / noticesFor', () => {
  it('treats the first snapshot as a baseline', () => {
    expect(texts([[entry({ status: 'downloading' })]])).toEqual([]);
  });

  it('announces a queued game, then its install, once each', () => {
    expect(
      texts([
        [],
        [entry({ status: 'downloading' })],
        [entry({ status: 'downloading', downloaded_bytes: 10 })],
        [entry({ status: 'installing' })],
        [entry({ status: 'completed' })],
        [entry({ status: 'completed' })],
      ]),
    ).toEqual(['Chrono Trigger added to download queue', 'Chrono Trigger installed']);
  });

  it('reports a failure with its reason, and a cancel', () => {
    expect(
      texts([
        [],
        [entry({ id: 1 }), entry({ id: 2, title: 'Zelda' })],
        [entry({ id: 1, status: 'failed', error: 'Disk full' }), entry({ id: 2, title: 'Zelda', status: 'cancelled' })],
      ]),
    ).toEqual([
      // A snapshot lists newest first; cards follow the order things happened.
      'Zelda added to download queue',
      'Chrono Trigger added to download queue',
      'Cancelled Zelda',
      "Couldn't install Chrono Trigger: Disk full",
    ]);
  });

  it('words a failure without a reason plainly', () => {
    expect(texts([[], [entry({ status: 'failed' })]])).toEqual([
      'Chrono Trigger added to download queue',
      "Couldn't install Chrono Trigger",
    ]);
  });

  it('words game updates and extra content', () => {
    expect(
      texts([
        [],
        [entry({ id: 1, kind: 'update' }), entry({ id: 2, kind: 'ps4_content', title: 'Bloodborne' })],
        [
          entry({ id: 1, kind: 'update', status: 'completed' }),
          entry({ id: 2, kind: 'ps4_content', title: 'Bloodborne', status: 'completed' }),
        ],
      ]),
    ).toEqual([
      'Extra content for Bloodborne added to download queue',
      'Update for Chrono Trigger added to download queue',
      'Extra content for Bloodborne installed',
      'Chrono Trigger updated',
    ]);
    // The queue's own kind suffix is not repeated.
    expect(texts([[], [entry({ kind: 'native_update', title: 'My Game (update)', status: 'completed' })]])).toEqual([
      'Update for My Game added to download queue',
      'My Game updated',
    ]);
    expect(texts([[], [entry({ kind: 'ps4_content', title: 'Bloodborne (dlc)' })]])).toEqual([
      'Extra content for Bloodborne added to download queue',
    ]);
    expect(texts([[], [entry({ kind: 'native_update', status: 'cancelled' })]])).toEqual([
      'Update for Chrono Trigger added to download queue',
      'Cancelled the update for Chrono Trigger',
    ]);
  });

  it('announces a retried entry as queued again', () => {
    expect(
      texts([
        [entry({ status: 'failed' })],
        [entry({ status: 'queued' })],
      ]),
    ).toEqual(['Chrono Trigger added to download queue']);
  });

  it('lets the emulator-installed event announce an emulator completion', () => {
    const emu = { job: 'emulator' as const, kind: 'emulator' as const, title: 'PCSX2', source_id: 'PCSX2/pcsx2' };
    expect(texts([[], [entry(emu)], [entry({ ...emu, status: 'completed' })]])).toEqual(['Installing PCSX2…']);
    expect(texts([[], [entry({ ...emu, status: 'failed', error: 'No asset' })]])).toEqual([
      'Installing PCSX2…',
      "Couldn't install PCSX2: No asset",
    ]);
  });

  it('announces a compat tool from queue to install (umu-launcher)', () => {
    const umu = { job: 'emulator' as const, kind: 'compat_tool' as const, title: 'umu-launcher' };
    expect(texts([[], [entry(umu)], [entry({ ...umu, status: 'completed' })]])).toEqual([
      'Installing umu-launcher…',
      'umu-launcher installed',
    ]);
  });

  it('uses a hint for an emulator update and a Proton picked as the default', () => {
    const emu = entry({ job: 'emulator', kind: 'emulator', title: 'PCSX2' });
    expect(noticesFor({ entry: emu, from: null }, { setupActive: false, hint: { update: true } })[0].text).toBe(
      'Updating PCSX2…',
    );
    const proton = entry({ job: 'emulator', kind: 'compat_tool', title: 'GE-Proton', status: 'completed' });
    expect(
      noticesFor({ entry: proton, from: 'installing' }, { setupActive: false, hint: { done: 'GE-Proton installed and used for Windows games' } }).map(
        (n) => n.text,
      ),
    ).toEqual(['GE-Proton installed and used for Windows games']);
  });

  it('stays quiet about emulators while the setup card counts them, but not about games', () => {
    expect(
      texts(
        [
          [],
          [entry({ id: 1, job: 'emulator', kind: 'emulator', title: 'PCSX2' }), entry({ id: 2 })],
          [entry({ id: 1, job: 'emulator', kind: 'emulator', title: 'PCSX2', status: 'failed' }), entry({ id: 2 })],
        ],
        true,
      ),
    ).toEqual(['Chrono Trigger added to download queue']);
  });

  it('never speaks for firmware or library-move rows', () => {
    expect(
      texts([
        [],
        [entry({ id: 1, job: 'firmware', kind: 'firmware' }), entry({ id: 2, job: 'library', kind: 'library_move' })],
        [
          entry({ id: 1, job: 'firmware', kind: 'firmware', status: 'completed' }),
          entry({ id: 2, job: 'library', kind: 'library_move', status: 'failed' }),
        ],
      ]),
    ).toEqual([]);
  });
});

describe('single notices', () => {
  it('words the emulator completion, launches and refusals', () => {
    expect(emulatorInstalledNotice('PCSX2', true).text).toBe('PCSX2 installed');
    expect(emulatorInstalledNotice('PCSX2', false).text).toBe('PCSX2 updated');
    expect(launchingNotice('Zelda').text).toBe('Launching Zelda…');
    expect(launchFailedNotice('Zelda', 'Emulator executable not found: /x')).toEqual({
      group: 'launch-failed',
      text: "Couldn't launch Zelda: Emulator executable not found: /x",
      level: 'error',
    });
    expect(queueRefusedNotice('Zelda', 'Set a library folder first').text).toBe(
      "Couldn't add Zelda to the download queue: Set a library folder first",
    );
  });

  it('gives backup rows no per-row card: their batch speaks for them', () => {
    const row = (status: DownloadEntry['status']) =>
      entry({ job: 'backup', kind: 'backup_game', status, drive_name: 'Couch kit', drive_label: 'SAMSUNG T7', batch: 2 });
    expect(texts([[], [row('queued')], [row('completed')]])).toEqual([]);
    expect(texts([[], [row('queued')], [row('failed')]])).toEqual([]);
  });

  it('keeps an install from a drive announced as a game, but not the emulator it brings', () => {
    const fromDrive = { drive_id: '/run/media/sam/T7', drive_name: 'Couch kit', drive_label: 'SAMSUNG T7' };
    expect(texts([[], [entry({ ...fromDrive, title: 'Velvet Static' })]])).toEqual(['Velvet Static added to download queue']);
    expect(
      texts([[], [entry({ ...fromDrive, job: 'emulator', kind: 'emulator', title: 'DuckStation', source_id: 'duckstation' })]]),
    ).toEqual([]);
  });

  it('shows a backup-notice as the backend worded it, failures as errors', () => {
    expect(backupNoticeCard({ message: 'PlayStation firmware installed from Couch kit', error: false })).toEqual({
      group: 'drive-installed',
      text: 'PlayStation firmware installed from Couch kit',
      level: 'success',
    });
    expect(backupNoticeCard({ message: 'Some PlayStation firmware from Couch kit could not be installed', error: true })).toEqual({
      group: 'drive-failed',
      text: 'Some PlayStation firmware from Couch kit could not be installed',
      level: 'error',
    });
  });

  it('keeps what an install from a drive queued apart from what a drive installed', () => {
    const queued = driveQueuedNotice('DuckStation from Couch kit added to download queue');
    expect(queued).toEqual({
      group: 'drive-queued',
      text: 'DuckStation from Couch kit added to download queue',
      level: 'success',
      action: 'downloads',
    });
    // The emulator line and the firmware card in one window: two cards, each
    // as worded, never "2 items installed from a backup drive".
    const firmware = backupNoticeCard({ message: 'PlayStation firmware installed from Couch kit', error: false });
    expect(coalesce([queued, firmware]).map((n) => n.text)).toEqual([
      'DuckStation from Couch kit added to download queue',
      'PlayStation firmware installed from Couch kit',
    ]);
  });

  it('names the game an early exit was about when exactly one session went', () => {
    const s = (id: number, title: string): GameSession => ({
      id,
      key: `romm:${id}`,
      rom_id: id,
      title,
      emulator_name: 'E',
      started_at: 0,
      pid: 1,
    });
    expect(earlyExitNotice([s(1, 'Zelda'), s(2, 'Mario')], [s(2, 'Mario')], 'Process exited immediately (code 3).').text).toBe(
      'Zelda — Process exited immediately (code 3).',
    );
    expect(earlyExitNotice([], [], 'Process exited immediately.').text).toBe('Process exited immediately.');
  });
});

describe('coalesce', () => {
  const n = (group: Notice['group'], text: string, level: Notice['level'] = 'success'): Notice => ({ group, text, level });

  it('keeps a lone notice as it is and summarises several of one group', () => {
    expect(coalesce([n('game-queued', 'A added to download queue')])).toEqual([
      n('game-queued', 'A added to download queue'),
    ]);
    expect(
      coalesce([
        n('game-queued', 'A added to download queue'),
        n('tool-queued', 'Installing PCSX2…'),
        n('game-queued', 'B added to download queue'),
        n('game-failed', "Couldn't install C", 'error'),
        n('game-failed', "Couldn't install D", 'error'),
      ]),
    ).toEqual([
      n('game-queued', '2 games added to download queue'),
      n('tool-queued', 'Installing PCSX2…'),
      n('game-failed', '2 downloads failed — see Downloads', 'error'),
    ]);
  });

  it('has a summary for every group', () => {
    expect(summaryText('tool-queued', 5)).toBe('Installing 5 emulators…');
    expect(summaryText('cancelled', 3)).toBe('Cancelled 3 downloads');
    expect(summaryText('game-installed', 4)).toBe('4 games installed');
    expect(summaryText('update-queued', 2)).toBe('2 updates added to download queue');
    expect(summaryText('drive-queued', 2)).toBe('2 items from a backup drive added to download queue');
    expect(summaryText('drive-installed', 2)).toBe('2 items installed from a backup drive');
    expect(summaryText('drive-failed', 1)).toBe('1 item from a backup drive could not be installed');
    expect(summaryText('backup-queued', 2)).toBe('2 backups added to the backup queue');
    expect(summaryText('backup-done', 2)).toBe('2 backups finished');
    expect(summaryText('backup-failed', 2)).toBe('2 backups stopped — see Downloads');
  });
});

describe('actions (TV mode: View runs them, items 144, 148)', () => {
  function cards(snapshots: DownloadEntry[][]): Notice[] {
    let book: StatusBook | null = null;
    const out: Notice[] = [];
    for (const snap of snapshots) {
      const { book: next, transitions } = diffSnapshot(book, snap);
      book = next;
      for (const t of transitions) out.push(...noticesFor(t, { setupActive: false }));
    }
    return out;
  }

  it('a queued game, update, emulator or failed install opens Downloads', () => {
    const [queued, failed] = cards([[], [entry({})], [entry({ status: 'failed', error: '503' })]]);
    expect(queued).toMatchObject({ text: 'Chrono Trigger added to download queue', action: 'downloads' });
    expect(failed).toMatchObject({ text: "Couldn't install Chrono Trigger: 503", action: 'downloads' });
    const [emu] = cards([[], [entry({ job: 'emulator', kind: 'emulator', title: 'PCSX2', source_id: 'pcsx2' })]]);
    expect(emu).toMatchObject({ text: 'Installing PCSX2…', action: 'downloads' });
  });

  it('a finished install, a launch or a refused queue offers nothing', () => {
    const [, done] = cards([[], [entry({})], [entry({ status: 'completed' })]]);
    expect(done.action).toBeUndefined();
    expect(launchingNotice('Zelda').action).toBeUndefined();
    expect(queueRefusedNotice('Zelda', 'offline').action).toBeUndefined();
  });

  it('a summary keeps the action its cards share', () => {
    const q = (t: string): Notice => ({ group: 'game-queued', text: t, level: 'success', action: 'downloads' });
    expect(coalesce([q('A'), q('B')])[0]).toEqual({ group: 'game-queued', text: '2 games added to download queue', level: 'success', action: 'downloads' });
    const failed = coalesce([
      { group: 'game-failed', text: "Couldn't install C", level: 'error', action: 'downloads' },
      queueRefusedNotice('D', 'offline'),
    ]);
    expect(failed[0].action).toBeUndefined();
  });

  it('the Coalescer hands the card with its action to the show callback', () => {
    vi.useFakeTimers();
    const shown: Notice[] = [];
    const c = new Coalescer((_t, _l, card) => shown.push(card));
    c.add({ group: 'game-queued', text: 'A added to download queue', level: 'success', action: 'downloads' });
    c.flush();
    expect(shown).toEqual([{ group: 'game-queued', text: 'A added to download queue', level: 'success', action: 'downloads' }]);
    vi.useRealTimers();
  });
});

describe('Coalescer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  function make() {
    const shown: string[] = [];
    const c = new Coalescer((text) => shown.push(text));
    return { c, shown };
  }

  it('shows a lone notice after the window', () => {
    const { c, shown } = make();
    c.add(launchingNotice('Zelda'));
    expect(shown).toEqual([]);
    vi.advanceTimersByTime(COALESCE_WINDOW_MS);
    expect(shown).toEqual(['Launching Zelda…']);
  });

  it('merges a burst into one card', () => {
    const { c, shown } = make();
    for (let i = 0; i < 5; i++) {
      c.add({ group: 'game-queued', text: `G${i} added to download queue`, level: 'success' });
      vi.advanceTimersByTime(100);
    }
    vi.advanceTimersByTime(COALESCE_WINDOW_MS);
    expect(shown).toEqual(['5 games added to download queue']);
  });

  it('never holds a steady trickle past the maximum', () => {
    const { c, shown } = make();
    const start = Date.now();
    while (shown.length === 0) {
      c.add({ group: 'game-queued', text: 'G added to download queue', level: 'success' });
      vi.advanceTimersByTime(COALESCE_WINDOW_MS - 50);
    }
    expect(Date.now() - start).toBeLessThanOrEqual(COALESCE_MAX_MS + COALESCE_WINDOW_MS);
  });

  it('keeps separate bursts separate', () => {
    const { c, shown } = make();
    c.add(launchingNotice('A'));
    vi.advanceTimersByTime(COALESCE_WINDOW_MS);
    c.add(launchingNotice('B'));
    vi.advanceTimersByTime(COALESCE_WINDOW_MS);
    expect(shown).toEqual(['Launching A…', 'Launching B…']);
  });

  it('flush shows at once and dispose drops', () => {
    const { c, shown } = make();
    c.add(launchingNotice('A'));
    c.flush();
    expect(shown).toEqual(['Launching A…']);
    c.add(launchingNotice('B'));
    c.dispose();
    vi.advanceTimersByTime(COALESCE_MAX_MS);
    expect(shown).toEqual(['Launching A…']);
  });
});

describe('second lines (TV cards)', () => {
  it('names the systems a fresh emulator install is now used for', () => {
    expect(emulatorInstalledNotice('DuckStation', true, ['Sony PlayStation']).sub).toBe('Used for Sony PlayStation');
    expect(emulatorInstalledNotice('RetroArch', true, ['SNES', 'Sega Genesis']).sub).toBe('Used for SNES and Sega Genesis');
    expect(emulatorInstalledNotice('RetroArch', true, ['A', 'B', 'C']).sub).toBe('Used for A, B and C');
    expect(emulatorInstalledNotice('PCSX2', true, []).sub).toBeUndefined();
    expect(emulatorInstalledNotice('PCSX2', true).sub).toBeUndefined();
    // An update switched nothing: no second line.
    expect(emulatorInstalledNotice('PCSX2', false, ['PlayStation 2']).sub).toBeUndefined();
  });

  it('says where an install from a drive copies from', () => {
    const [queued] = noticesFor({ entry: entry({ drive_label: 'SAMSUNG T7', drive_id: '/m/T7' }), from: null }, { setupActive: false });
    expect(queued).toMatchObject({ text: 'Chrono Trigger added to download queue', sub: 'Copying from SAMSUNG T7', action: 'downloads' });
    const [plain] = noticesFor({ entry: entry({}), from: null }, { setupActive: false });
    expect(plain.sub).toBeUndefined();
  });

  it('drops the second line from a summary card', () => {
    const cards = coalesce([
      emulatorInstalledNotice('A', true, ['X']),
      emulatorInstalledNotice('B', true, ['Y']),
    ]);
    expect(cards).toHaveLength(1);
    expect(cards[0].sub).toBeUndefined();
  });
});

describe('installs from a drive (items 428, 450)', () => {
  const q = (t: string, sub?: string): Notice => ({ group: 'game-queued', text: t, level: 'success', action: 'downloads', ...(sub ? { sub } : {}) });

  it('a summary keeps the second line every card shares', () => {
    expect(coalesce([q('A', 'Copying from T7'), q('B', 'Copying from T7')])).toEqual([
      { group: 'game-queued', text: '2 games added to download queue', level: 'success', action: 'downloads', sub: 'Copying from T7' },
    ]);
    expect(coalesce([q('A', 'Copying from T7'), q('B')])[0].sub).toBeUndefined();
    expect(coalesce([q('A', 'Copying from T7'), q('B', 'Copying from X5')])[0].sub).toBeUndefined();
  });

  it('"<pack> has no emulator for <system>" is its own card, once per system', () => {
    const note = driveOutcomeNotice('Couch kit has no emulator for Sony PlayStation 2');
    expect(note).toEqual({ group: 'drive-note', text: 'Couch kit has no emulator for Sony PlayStation 2', level: 'success' });
    expect(driveOutcomeNotice('DuckStation from Couch kit added to download queue')).toEqual(
      driveQueuedNotice('DuckStation from Couch kit added to download queue'),
    );
    const queued = driveQueuedNotice('DuckStation from Couch kit added to download queue');
    expect(coalesce([note, queued, note]).map((n) => n.text)).toEqual([note.text, queued.text]);
    const other = driveOutcomeNotice('Couch kit has no emulator for Nintendo 64');
    expect(coalesce([note, other])[0].text).toBe('A backup drive has no emulator for 2 systems');
  });

  it('a batch takes each game\'s queued card once, within its time', () => {
    const quiet = new QuietQueued(1000);
    quiet.add([1, 2], 0);
    expect(quiet.takes(q('A'), 1, 10)).toBe(true);
    expect(quiet.takes(q('A'), 1, 20)).toBe(false);
    expect(quiet.takes({ ...q('B'), group: 'update-queued' }, 2, 20)).toBe(false);
    expect(quiet.takes(q('B'), 2, 5000)).toBe(false);
    quiet.add([3], 0);
    quiet.drop(3);
    expect(quiet.takes(q('C'), 3, 10)).toBe(false);
  });
});

describe('a queued card\'s second line (ruling A)', () => {
  it('goes on the game\'s next queued card once, within its time', () => {
    const subs = new QueuedSubs(1000);
    const q: Notice = { group: 'game-queued', text: 'A added to download queue', level: 'success', action: 'downloads' };
    subs.add(7, 'PCSX2 installs with it', 0);
    expect(subs.apply({ ...q, group: 'update-queued' }, 7, 1)).toEqual({ ...q, group: 'update-queued' });
    expect(subs.apply(q, 8, 1)).toEqual(q);
    expect(subs.apply(q, 7, 1)).toEqual({ ...q, sub: 'PCSX2 installs with it' });
    expect(subs.apply(q, 7, 2)).toEqual(q);
    subs.add(9, 'X', 0);
    expect(subs.apply(q, 9, 5000)).toEqual(q);
  });
});
