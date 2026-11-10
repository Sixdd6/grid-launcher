import { describe, expect, it } from 'vitest';
import type { MediaItem } from '../../details/media';
import type { SystemRow } from '../../systems/rows';
import { readStep } from '../focus/reveal';
import { noEmulatorFor, noEmulatorSub, systemRowFor } from '../../details/noEmulator';
import {
  achSummary,
  seekTarget,
  viewerHints,
  viewerAction,
  viewerRepeats,
  viewerShow,
  VIEWER_BUTTONS,
  type ViewerShow,
  releaseFact,
  unlockedWhen,
  ADD_SAVE_FOLDER,
  BACKUP_NEEDS_SERVER,
  LIBRARY_FOLDER_BROWSER_TITLE,
  LIBRARY_FOLDER_SET,
  libraryFolderConfirm,
  removeLocationConfirm,
  SAVE_FOLDER_ADDED,
  SAVE_LOCATIONS_HEADING,
  saveFolderBrowserTitle,
  umuConfirm,
  cancelConfirm,
  deleteConfirm,
  detButtons,
  factsOf,
  lastPlayedWhen,
  leftLines,
  linkLine,
  playingLine,
  playingNote,
  removeDriveConfirm,
  removeRowConfirm,
  restoreConfirm,
  saveLine,
  saveListTitle,
  savesEmpty,
  savesHeading,
  savesNote,
  sessionTime,
  tabStep,
  tvHeaderLine,
  uninstallConfirm,
  updateConfirm,
  uploadLabel,
  viewerCaption,
  viewerStep,
  launchFileSub,
  type DetState,
  type LeftInput,
  type Line,
} from './details';

const base: DetState = {
  title: 'Chrono Trigger',
  native: false,
  installed: false,
  missing: false,
  unlinked: false,
  noId: false,
  live: null,
  playing: false,
  updateLabel: null,
  content: { update: false, dlc: false },
  drive: null,
  connected: true,
};

const labels = (s: Partial<DetState>) => detButtons({ ...base, ...s }).map((b) => b.label);
const text = (lines: Line[]) => lines.map((l) => l.map((s) => (s.b ? `<b>${s.text}</b>` : s.text)).join(''));

const left: LeftInput = {
  installed: false,
  missing: false,
  unlinked: false,
  noId: false,
  lastPlayedAt: 0,
  nowSeconds: 1_762_560_000 + 2 * 86_400,
  launchTarget: 'Snes9x',
  installBlocked: '',
  noEmulator: null,
  size: '',
  onDrives: '',
  drive: null,
  driveCopy: [],
};

describe('Details buttons per state (items 276–283, 431, 435, 437)', () => {
  it('installed: Play, Update, Game settings (native), Uninstall, Backup to drive…', () => {
    expect(labels({ installed: true })).toEqual(['Play', 'Uninstall', 'Backup to drive…']);
    expect(labels({ installed: true, native: true, updateLabel: 'Update' })).toEqual(['Play', 'Update', 'Game settings', 'Uninstall', 'Backup to drive…']);
    const [play, uninstall] = detButtons({ ...base, installed: true });
    expect(play).toMatchObject({ kind: 'play', primary: true, icon: 'play' });
    expect(uninstall.danger).toBe(true);
  });

  it('not installed: Install ("Install app" when native) and Backup to drive…', () => {
    expect(labels({})).toEqual(['Install', 'Backup to drive…']);
    expect(labels({ native: true })).toEqual(['Install app', 'Backup to drive…']);
    expect(detButtons(base)[0]).toMatchObject({ primary: true, icon: 'download' });
  });

  it('offline: Install and Backup to drive… are disabled, each saying why', () => {
    const [install, backup] = detButtons({ ...base, connected: false });
    expect(install).toMatchObject({ disabled: true, why: "Couldn't add Chrono Trigger to the download queue: the server can't be reached" });
    expect(backup).toMatchObject({ disabled: true, why: BACKUP_NEEDS_SERVER });
  });

  it('queued or installing: Cancel · queued / Cancel install · n%', () => {
    expect(labels({ live: { queued: true, pct: 0, cancellable: true } })[0]).toBe('Cancel · queued');
    expect(labels({ live: { queued: false, pct: 42, cancellable: true } })[0]).toBe('Cancel install · 42%');
    expect(detButtons({ ...base, live: { queued: false, pct: 99, cancellable: false } })[0]).toMatchObject({ disabled: true });
  });

  it('missing: Locate…, Remove from library, Backup to drive…; unlinked: Link, Remove, no Backup', () => {
    expect(labels({ installed: true, missing: true })).toEqual(['Locate…', 'Remove from library (keeps files)', 'Backup to drive…']);
    expect(labels({ installed: true, unlinked: true })).toEqual(['Link to server game', 'Remove from library (keeps files)']);
    expect(detButtons({ ...base, installed: true, unlinked: true })[1].danger).toBe(true);
  });

  it('from a drive: Install from drive / Play / Cancel, then Remove from drive…, never Backup', () => {
    expect(labels({ drive: 'Couch kit' })).toEqual(['Install from drive', 'Remove from drive…']);
    expect(labels({ drive: 'Couch kit', installed: true })).toEqual(['Play', 'Remove from drive…']);
    expect(labels({ drive: 'Couch kit', live: { queued: true, pct: 0, cancellable: true } })).toEqual(['Cancel · queued', 'Remove from drive…']);
  });

  it('a running game shows Stop game in place of Play; PS4 content buttons follow Update', () => {
    expect(labels({ installed: true, playing: true })[0]).toBe('Stop game');
    expect(labels({ installed: true, content: { update: true, dlc: true } })).toEqual(['Play', 'Install update', 'Install DLC', 'Uninstall', 'Backup to drive…']);
  });
});

describe('the left column (items 272–273, 281–282, 432–433, 436)', () => {
  it('installed: last played and the emulator', () => {
    expect(text(leftLines({ ...left, installed: true }))).toEqual(['Last played <b>never</b>', 'Launches with <b>Snes9x</b>']);
    expect(text(leftLines({ ...left, installed: true, lastPlayedAt: 1_762_560_000, launchTarget: 'No default emulator' }))).toEqual([
      'Last played <b>2 days ago</b>',
      'Launches with <b>no emulator</b>',
    ]);
    expect(text(leftLines({ ...left, installed: true, launchTarget: '' }))).toEqual(['Last played <b>never</b>']);
    expect(lastPlayedWhen(0, 1_762_560_000)).toBe('never');
    expect(lastPlayedWhen(1_762_560_000, 1_762_560_000 + 3600)).toBe('today');
    expect(lastPlayedWhen(1_762_560_000, 1_762_560_000 + 86_400)).toBe('yesterday');
  });

  it('not installed: the size, or why it would not install', () => {
    expect(text(leftLines({ ...left, size: '1.2 GB' }))).toEqual(['Not installed · 1.2 GB']);
    expect(text(leftLines({ ...left, installBlocked: 'No available emulator is configured.' }))).toEqual(['<b>No available emulator is configured.</b>']);
  });

  it('missing and unlinked', () => {
    expect(text(leftLines({ ...left, installed: true, missing: true }))).toEqual([
      '<b>Files not found.</b> Play and Uninstall are unavailable until the files are found.',
    ]);
    expect(text(leftLines({ ...left, installed: true, unlinked: true }))).toEqual([
      '<b>Not linked to a server game.</b> Play, cloud saves and updates need the link.',
    ]);
  });

  it('on a connected drive, and from a drive', () => {
    expect(text(leftLines({ ...left, installed: true, onDrives: 'On Couch kit' })).at(-1)).toBe('On <b>Couch kit</b>');
    expect(text(leftLines({ ...left, drive: 'Couch kit', size: '1.7 GB', installed: true, driveCopy: ['A copy of an installed game · v1.1'] }))).toEqual([
      'Installed on this computer.',
      'From <b>Couch kit</b> · 1.7 GB',
      'Installing copies it from the drive, no network needed.',
      'A copy of an installed game · v1.1',
    ]);
  });
});

describe('header, tabs, facts', () => {
  it('the header line leads with the drive and ends with the rating (items 274, 430)', () => {
    expect(tvHeaderLine('SNES · 1995 · Square · RPG', null, '8.4')).toBe('SNES · 1995 · Square · RPG · 8.4');
    expect(tvHeaderLine('SNES · 1995', 'Couch kit', '')).toBe('From Couch kit · SNES · 1995');
  });

  it('LB / RB wrap at the ends (item 292); Achievements only when listed (item 291)', () => {
    const tabs = ['overview', 'media', 'saves'] as const;
    expect(tabStep(tabs, 'overview', -1)).toBe('saves');
    expect(tabStep(tabs, 'saves', 1)).toBe('overview');
    expect(tabStep([...tabs, 'achievements'], 'saves', 1)).toBe('achievements');
  });

  it('facts keep only those with a value', () => {
    expect(factsOf([['developer', 'Developer', 'Square'], ['players', 'Players', ' ']]).map((f) => f.label)).toEqual(['Developer']);
  });

  it('the summary reads through in steps of up to 6rem before focus moves on (item 297)', () => {
    // A 20px rem: the view is 0–400; the summary runs 0–1000.
    expect(readStep('down', 0, 1000, 0, 400, 20)).toBe(120);
    expect(readStep('down', -700, 300, 0, 400, 20)).toBeNull();
    expect(readStep('up', -50, 950, 0, 400, 20)).toBe(-(50 + 24));
    expect(readStep('up', 0, 1000, 0, 400, 20)).toBeNull();
  });
});

describe('media (items 301–305)', () => {
  const items: MediaItem[] = [
    { kind: 'screenshot', url: 'a', caption: '' },
    { kind: 'screenshot', url: 'b', caption: '' },
    { kind: 'youtube', videoId: 'x', caption: '' },
  ];
  it('the caption and stepping', () => {
    expect(viewerCaption(items, 1, 'Zelda')).toBe('Screenshot 2 of 2 · Zelda');
    expect(viewerCaption(items, 2, 'Zelda')).toBe('Trailer · Zelda');
    expect(viewerStep(0, 3, -1)).toBe(0);
    expect(viewerStep(2, 3, 1)).toBe(2);
    expect(viewerStep(1, 3, 1)).toBe(2);
  });
});

describe('saves (items 306–314)', () => {
  const rec = { relative_time: '2 hours ago', size_text: '128 KB', slot: null };
  it('the notes, headings, rows and empty text', () => {
    expect(savesNote({ unlinked: true, noId: false, installed: true, lastPlayedAt: 0 })).toBe('Cloud saves need the link to a server game.');
    expect(savesNote({ unlinked: false, noId: false, installed: false, lastPlayedAt: 0 })).toBe('No cloud saves yet. They appear after you play.');
    expect(savesNote({ unlinked: false, noId: false, installed: true, lastPlayedAt: 0 })).toBeNull();
    expect(savesHeading('save', 'per_game')).toBe('Cloud Saves');
    expect(savesHeading('state', 'per_game')).toBe('Cloud States');
    expect(uploadLabel('save', 'per_game')).toBe('Upload latest save');
    expect(uploadLabel('state', 'per_game')).toBe('Upload latest state');
    expect(saveLine(rec)).toBe('2 hours ago · 128 KB');
    expect(saveLine({ ...rec, slot: '2' })).toBe('2 hours ago · 128 KB · slot 2');
    expect(saveListTitle('state', rec)).toBe('State from 2 hours ago');
    expect(savesEmpty('save')).toBe('No cloud saves for this game. Upload the latest one, or play and GRID uploads it when the game closes.');
    expect(savesEmpty('state')).toBe('No cloud states for this game. Upload the latest one, or play and GRID uploads it when the game closes.');
  });

  it('the confirmations', () => {
    expect(restoreConfirm('save', 'Zelda')).toEqual({
      title: 'Restore this save?',
      body: 'It replaces the save for Zelda on this computer. The one there now is overwritten.',
      ok: 'Restore',
    });
    expect(deleteConfirm('state')).toEqual({ title: 'Delete this state?', body: "It is removed from your server. This can't be undone.", ok: 'Delete' });
  });
});

describe('action confirmations (items 284–287, 419, 434)', () => {
  it('words', () => {
    expect(cancelConfirm('Zelda').ok).toBe('Cancel install');
    expect(updateConfirm('Zelda')).toEqual({ title: 'Update Zelda?', body: 'Saves and configuration are kept.', ok: 'Update' });
    expect(uninstallConfirm('Zelda').body).toBe('Its files are deleted from this computer. Cloud saves stay on your server.');
    expect(removeRowConfirm('Zelda', '/g/Zelda').body).toBe('Only the library entry goes. Its folder stays on disk: /g/Zelda.');
    expect(removeDriveConfirm('Zelda', 'Couch kit', false).body).toBe('Its files leave the drive. It can be backed up again later.');
    expect(removeDriveConfirm('Zelda', 'Couch kit', true).body).toBe('Its files leave the drive. The copy installed on this computer stays.');
  });
});

describe('Playing, Link, Game settings', () => {
  it('the session line and note (items 321–322)', () => {
    expect(sessionTime(65_400)).toBe('1:05');
    expect(sessionTime(0)).toBe('0:00');
    expect(playingLine('SNES', 'Snes9x', '0:07')).toBe('SNES · Snes9x · 0:07');
    expect(playingLine('Windows', '', '0:07')).toBe('Windows · 0:07');
    expect(playingNote(true)).toBe(
      "The game has the controller while it's in front. GRID listens again when you switch back or the game closes, and uploads your save when it closes.",
    );
    expect(playingNote(false)).toBe("The game has the controller while it's in front. GRID listens again when you switch back or the game closes.");
  });

  it('a link row and the launch file line', () => {
    expect(linkLine('SNES', 795_571_200)).toBe('SNES · 1995');
    expect(linkLine('SNES', null)).toBe('SNES');
    expect(launchFileSub('sidecar', 'a.exe', 'a.exe')).toBe('Set by game.json');
    expect(launchFileSub('sidecar', 'b.exe', 'a.exe')).toBe('Your choice');
    expect(launchFileSub('user', 'a.exe', 'a.exe')).toBe('Your choice');
  });
});

describe('refusal prompts and save locations (checklist 529–531)', () => {
  it('the umu-launcher and library-folder confirms', () => {
    expect(umuConfirm('Static Bloom')).toEqual({
      title: 'Install umu-launcher?',
      body: 'Static Bloom runs through Proton, which needs umu-run. umu-launcher downloads as a Downloads row; launch Static Bloom again when it finishes.',
      ok: 'Install umu-launcher',
    });
    expect(libraryFolderConfirm()).toEqual({
      title: 'Set a library folder',
      body: 'GRID installs games into a library folder, and none is set. Choose one, then press Install again.',
      ok: 'Choose a folder…',
    });
    expect(LIBRARY_FOLDER_BROWSER_TITLE).toBe('Library folder');
    expect(LIBRARY_FOLDER_SET).toBe('Library folder set');
  });

  it('the save location words', () => {
    expect(SAVE_LOCATIONS_HEADING).toBe('Save locations');
    expect(ADD_SAVE_FOLDER).toBe('Add a save folder…');
    expect(SAVE_FOLDER_ADDED).toBe('Save folder added');
    expect(saveFolderBrowserTitle('My Game')).toBe('Save folder for My Game');
    expect(removeLocationConfirm('<home>/saves')).toEqual({ title: 'Remove this save location?', body: '<home>/saves', ok: 'Remove' });
  });
});

describe('achievements (item 315)', () => {
  const view = (hardcore: number, progress_known = true) => ({
    progress_known,
    summary: { earned: 3, total: 40, points_earned: 30, points_total: 400, hardcore },
    rows: [],
  });

  it('always ends with "· <h> hardcore" when progress is known, 0 included', () => {
    expect(achSummary(view(1) as never)).toBe('3 of 40 unlocked · 30 of 400 points · 1 hardcore');
    expect(achSummary(view(0) as never)).toBe('3 of 40 unlocked · 30 of 400 points · 0 hardcore');
    expect(achSummary(view(0, false) as never)).toBe('40 achievements · 400 points');
  });

  it('"Unlocked <when>" is relative', () => {
    const now = Date.parse('2024-03-06T12:00:00Z') / 1000;
    expect(unlockedWhen('2024-03-02 10:00:00', now)).toBe('4 days ago');
    expect(unlockedWhen('2024-03-06T08:00:00+00:00', now)).toBe('today');
    expect(unlockedWhen('2024-03-05 08:00:00', now)).toBe('yesterday');
    expect(unlockedWhen('soon', now)).toBe('soon');
    expect(unlockedWhen(null, now)).toBe('');
  });
});

describe('the Release fact (item 295)', () => {
  it('reads "d Mon yyyy" in UTC; blank without a date', () => {
    expect(releaseFact(795_571_200)).toBe('19 Mar 1995');
    expect(releaseFact(0)).toBe('');
    expect(releaseFact(Number(''))).toBe('');
    expect(releaseFact(Number('x'))).toBe('');
  });
});

describe('a system with no emulator (ruling A, items 273, 288)', () => {
  const sys = (over: Partial<SystemRow>): SystemRow => ({
    key: 'PlayStation 2',
    label: 'PlayStation 2',
    name: 'PlayStation 2',
    slug: 'ps2',
    romCount: 3,
    platform: null,
    emulator: '',
    explicitNone: false,
    installed: [],
    downloads: [],
    recommended: { source_id: 'pcsx2', name: 'PCSX2 (Playstation 2)', kind: 'standalone', installed: false } as SystemRow['recommended'],
    core: null,
    ...over,
  });

  it('finds the row by label, name or slug', () => {
    const rows = [sys({}), sys({ key: 'SNES', label: 'SNES', name: 'Super Nintendo', slug: 'snes' })];
    expect(systemRowFor(rows, 'PlayStation 2', '')?.slug).toBe('ps2');
    expect(systemRowFor(rows, 'Super Nintendo', '')?.slug).toBe('snes');
    expect(systemRowFor(rows, 'Other', 'snes')?.slug).toBe('snes');
    expect(systemRowFor(rows, 'Other', '')).toBeUndefined();
  });

  it('reads "No emulator for <system>." and what Install brings', () => {
    const rec = noEmulatorFor(sys({}), false);
    expect(text(leftLines({ ...left, size: '1.2 GB', noEmulator: rec }))).toEqual(['<b>No emulator for PlayStation 2.</b> Install installs PCSX2 too.']);
    expect(noEmulatorSub(rec)).toBe('PCSX2 installs with it');
    const busy = noEmulatorFor(sys({}), true);
    expect(text(leftLines({ ...left, noEmulator: busy }))).toEqual(['<b>No emulator for PlayStation 2.</b> PCSX2 is installing.']);
    expect(noEmulatorSub(busy)).toBeNull();
    const none = noEmulatorFor(sys({ recommended: null }), false);
    expect(text(leftLines({ ...left, noEmulator: none }))).toEqual(['<b>No emulator for PlayStation 2.</b> Add one in Settings › Systems.']);
    expect(noEmulatorSub(none)).toBeNull();
  });

  it('a system with an emulator, or set to None, says nothing of it', () => {
    expect(noEmulatorFor(sys({ emulator: 'PCSX2' }), false)).toBeNull();
    expect(noEmulatorFor(sys({ explicitNone: true }), false)).toBeNull();
    expect(noEmulatorFor(undefined, false)).toBeNull();
  });
});

describe('the media viewer controls (user decision 2026-11-10)', () => {
  const labels = (h: { button: string; label: string }[]) => h.map((x) => `${x.button} ${x.label}`);
  it('a video: A pauses or plays, LB / RB seek 10 s, LT / RT set the volume', () => {
    expect(labels(viewerHints('video', false))).toEqual([
      'A Pause',
      'LB Back 10 s',
      'RB Forward 10 s',
      'LT Volume −',
      'RT Volume +',
      'B Close',
    ]);
    expect(labels(viewerHints('video', true))[0]).toBe('A Play');
    expect(labels(viewerHints('screenshot', false))).toEqual(['LB Previous', 'RB Next', 'B Close']);
    expect(labels(viewerHints('poster', false))).toEqual(['A Watch on YouTube', 'LB Previous', 'RB Next', 'B Close']);
  });

  it('a server video and a YouTube player are videos; a fallen-back trailer is its poster', () => {
    expect(viewerShow('video', false)).toBe('video');
    expect(viewerShow('youtube', true)).toBe('video');
    expect(viewerShow('youtube', false)).toBe('poster');
    expect(viewerShow('screenshot', false)).toBe('screenshot');
    expect(viewerShow(null, false)).toBeNull();
  });

  it('maps every button while a video shows', () => {
    const map = (show: ViewerShow, youtube: boolean) => VIEWER_BUTTONS.map((b) => `${b}:${viewerAction(b, show, youtube)}`);
    expect(map('video', true)).toEqual(['LB:back', 'RB:forward', 'LEFT:prev', 'RIGHT:next', 'LT:volDown', 'RT:volUp', 'X:watch']);
    expect(map('video', false)).toEqual(['LB:back', 'RB:forward', 'LEFT:prev', 'RIGHT:next', 'LT:volDown', 'RT:volUp', 'X:null']);
    expect(map('screenshot', false)).toEqual(['LB:prev', 'RB:next', 'LEFT:prev', 'RIGHT:next', 'LT:null', 'RT:null', 'X:null']);
    expect(map('poster', true)).toEqual(['LB:prev', 'RB:next', 'LEFT:prev', 'RIGHT:next', 'LT:null', 'RT:null', 'X:watch']);
  });

  it('held LT / RT repeat only while a video shows', () => {
    expect(viewerRepeats('LT', 'video')).toBe(true);
    expect(viewerRepeats('RT', 'video')).toBe(true);
    expect(viewerRepeats('LB', 'video')).toBe(false);
    expect(viewerRepeats('LT', 'screenshot')).toBe(false);
    expect(viewerRepeats('RT', 'poster')).toBe(false);
  });

  it('a seek stays inside the video', () => {
    expect(seekTarget(30, 120, 10)).toBe(40);
    expect(seekTarget(5, 120, -10)).toBe(0);
    expect(seekTarget(115, 120, 10)).toBe(120);
    expect(seekTarget(5, NaN, 10)).toBe(15);
  });
});
