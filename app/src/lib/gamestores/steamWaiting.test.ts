// Waiting for Steam to start the download, and Stop waiting (GameStores
// README "Steam: waiting for Steam to start", 2026-11-14; SPEC's Steam
// install paragraph; phase 4 plan user decision 4). Every desktop state of the
// prototype's `stJobLine`, `stCard`, `stRenderDetails`, `renderDownloads` and
// `stStopWait` that a pure module decides.
import { describe, expect, it } from 'vitest';
import { actionFor, footerLine, rowAction } from '../downloads/format';
import { coverPrimary, coverProgress, coverTags } from './cover';
import { managedBox, storeButtons } from './details';
import { isErrorLine, storeEntryLine, storeStripLine } from './downloads';
import { isWaitingForStore } from './jobs';
import { stoppedWaitingNotice } from './notices';
import { STOP_WAITING, STOP_WAITING_TITLE, stoppedWaitingSub, stoppedWaitingText, WAITING_FOR_STEAM, WAITING_NOTE } from './steam';
import { row, steamGame, steamRow, steamStatus } from './fixtures.test-util';
import { FAKE_STOPPED_WAITING, FAKE_TIMED_OUT, FakeStores } from './fakeApi';

const waiting = (over: Parameters<typeof steamRow>[0] = {}) =>
  steamRow({ id: 7, source_id: 'steam:9100007', title: 'Signal Lost', phase: 'waiting_for_store', downloaded_bytes: 0, ...over });

describe('which rows wait for Steam', () => {
  it('a live Steam row in the waiting phase, never one under way, ended, or another store’s', () => {
    expect(isWaitingForStore(waiting())).toBe(true);
    expect(isWaitingForStore(steamRow())).toBe(false);
    expect(isWaitingForStore(waiting({ status: 'cancelled' }))).toBe(false);
    expect(isWaitingForStore(row({ phase: 'waiting_for_store' }))).toBe(false);
  });
});

describe('the Downloads row while waiting', () => {
  it('reads "Waiting for Steam to start the download", with no "Managed by Steam"', () => {
    expect(storeEntryLine(waiting())).toBe('Waiting for Steam to start the download');
    expect(WAITING_FOR_STEAM).toBe('Waiting for Steam to start the download');
  });

  it('offers the small Stop waiting button with its tooltip, never Cancel', () => {
    expect(rowAction(waiting())).toBe('stop-waiting');
    expect(rowAction(steamRow())).toBe('managed');
    expect(STOP_WAITING).toBe('Stop waiting');
    expect(STOP_WAITING_TITLE).toBe('Cancelled it in Steam? GRID stops watching for this install.');
  });

  it('the strip names the store and the wait', () => {
    expect(storeStripLine(waiting())).toBe('Signal Lost · Steam · Waiting for Steam to start the download');
    expect(footerLine([waiting()])).toBe('Signal Lost · Steam · Waiting for Steam to start the download');
  });

  it('once Steam has started: Installing in Steam N% · Managed by Steam, and no Stop waiting', () => {
    const started = waiting({ phase: 'none', downloaded_bytes: 12 });
    expect(storeEntryLine(started)).toBe('Installing in Steam 12% · Managed by Steam');
    expect(rowAction(started)).toBe('managed');
  });
});

describe('the ended rows', () => {
  it('Stop waiting and the timeout end muted with the backend’s line and Dismiss, never Retry', () => {
    const stopped = waiting({ status: 'cancelled', phase: 'none', detail: 'Not installed · you stopped waiting for Steam' });
    const late = waiting({ status: 'cancelled', phase: 'none', detail: 'Not installed · Steam didn’t start the download in 5 minutes' });
    expect(storeEntryLine(stopped)).toBe('Not installed · you stopped waiting for Steam');
    expect(storeEntryLine(late)).toBe('Not installed · Steam didn’t start the download in 5 minutes');
    for (const e of [stopped, late]) {
      expect(isErrorLine(e)).toBe(false);
      expect(rowAction(e)).toBe('dismiss');
    }
    expect(actionFor('cancelled', 'store_external')).toBe('dismiss');
  });

  it('an install that went away stays plain Cancelled', () => {
    expect(storeEntryLine(steamRow({ status: 'cancelled' }))).toBe('Cancelled');
  });

  it('the card after Stop waiting, one line on the desktop', () => {
    expect(stoppedWaitingNotice('Signal Lost')).toEqual({
      text: 'Stopped waiting for Steam. If Steam installs Signal Lost later, GRID adds it.',
      error: false,
      action: null,
    });
    expect(stoppedWaitingText('Signal Lost')).toBe('Stopped waiting for Steam. If Steam installs Signal Lost later, GRID adds it.');
    expect(stoppedWaitingSub('Harbor of Bells')).toBe('If Steam installs Harbor of Bells later, GRID adds it.');
  });
});

describe('the cover while waiting', () => {
  it('WAITING over an empty bar, and a disabled "Waiting for Steam"', () => {
    const g = steamGame('Signal Lost');
    expect(coverTags(g, waiting())).toEqual([{ text: 'WAITING', tone: 'queue' }]);
    expect(coverProgress(waiting({ downloaded_bytes: 40 }))).toBe(0);
    expect(coverPrimary(g, steamStatus(), waiting())).toEqual({
      label: 'Waiting for Steam',
      act: 'details',
      disabled: true,
      title: 'Waiting for Steam to start the download',
    });
  });
});

describe('Details while waiting', () => {
  const ctx = (job: ReturnType<typeof steamRow>) => ({ game: steamGame('Signal Lost'), status: steamStatus(), job, playing: false, confirm: null });

  it('the waiting box with no bar, and a secondary Stop waiting', () => {
    expect(managedBox(waiting())).toEqual({ title: 'Waiting for Steam to start the download', pct: null, note: WAITING_NOTE });
    expect(WAITING_NOTE).toBe('Steam’s install window is open. If you cancelled it there, stop waiting.');
    expect(storeButtons(ctx(waiting()))).toEqual([{ label: 'Stop waiting', act: 'stop_waiting', primary: false }]);
  });

  it('once Steam has started: the managed box and the disabled Cancel', () => {
    const started = waiting({ phase: 'none', downloaded_bytes: 30 });
    expect(managedBox(started)?.title).toBe('Installing in Steam · 30%');
    expect(storeButtons(ctx(started)).map((b) => b.act)).toEqual(['cancel']);
  });
});

describe('the fake backend', () => {
  it('boots the prototype’s waiting row and timeout row', () => {
    const fake = new FakeStores();
    fake.bootJobs();
    const sl = fake.entries.find((e) => e.source_id === 'steam:9100007')!;
    expect(isWaitingForStore(sl)).toBe(true);
    const ra = fake.entries.find((e) => e.source_id === 'steam:9100006')!;
    expect(storeEntryLine(ra)).toBe(FAKE_TIMED_OUT);
    // Signal Lost waits forever.
    for (let i = 0; i < 30; i += 1) fake.tick();
    expect(isWaitingForStore(fake.entries.find((e) => e.id === sl.id)!)).toBe(true);
  });

  it('Install in Steam waits, then runs; Stop waiting only while it waits', () => {
    const fake = new FakeStores();
    fake.handle('steam_open', { key: 'steam:9100005', action: 'install' });
    const e = fake.entries.find((x) => x.source_id === 'steam:9100005')!;
    expect(e.phase).toBe('waiting_for_store');
    for (let i = 0; i < 10; i += 1) fake.tick();
    expect(e.phase).toBe('none');
    expect(() => fake.handle('steam_stop_waiting', { entryId: e.id })).toThrow('Steam has started the download. Pause or cancel it in Steam.');

    fake.handle('steam_open', { key: 'steam:9100007', action: 'install' });
    const w = fake.entries.find((x) => x.source_id === 'steam:9100007')!;
    fake.handle('steam_stop_waiting', { entryId: w.id });
    expect(w).toMatchObject({ status: 'cancelled', detail: FAKE_STOPPED_WAITING });
    expect(() => fake.handle('steam_stop_waiting', { entryId: w.id })).toThrow('That download isn’t waiting for Steam.');
  });
});
