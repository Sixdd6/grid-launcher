// The one action a game offers in place — a card's button, Discover's
// button, a TV cover's A press — and the main button set Details shows, as
// pure rules over what is known about the game. Both shells read these, so
// a game says Play, Install or Install from drive the same way everywhere.
// The effects themselves are in `run.ts`.
import type { DownloadStatus } from '../api';

export type QuickKind = 'play' | 'install' | 'install-drive' | 'locate' | 'queued' | 'installing' | 'none';

export type QuickAction = {
  kind: QuickKind;
  /** The button's text; `''` for `none`. */
  label: string;
};

export type QuickInput = {
  /** In the installed registry. */
  installed: boolean;
  /** The server rom id; `null` for an installed row not linked to a server game. */
  romId: number | null;
  /** 9c: installed, but its files are not on disk. */
  missing?: boolean;
  /** The live install for the game, when the caller tracks one (Discover). */
  live?: DownloadStatus | null;
  /** The game is shown from a backup drive: installing copies it from there. */
  fromDrive?: boolean;
  /** A server is connected (defaults to true). Only a drive installs without one. */
  connected?: boolean;
};

const LABELS: Record<QuickKind, string> = {
  play: 'Play',
  install: 'Install',
  'install-drive': 'Install from drive',
  locate: 'Locate…',
  queued: 'Queued',
  installing: 'Installing…',
  none: '',
};

function action(kind: QuickKind): QuickAction {
  return { kind, label: LABELS[kind] };
}

/**
 * The quick action for a game in its context.
 *
 * - Installed: Play; Locate… when its files are missing (Details opens to
 *   find them); nothing for a row with no server id (it cannot launch).
 * - A live install: Queued, then Installing… (never a second install).
 * - Not installed: Install from drive on a drive; Install from the server
 *   when one is connected; otherwise nothing.
 */
export function quickAction(input: QuickInput): QuickAction {
  const { installed, romId, missing = false, live = null, fromDrive = false, connected = true } = input;
  if (installed) {
    if (missing) return action('locate');
    return romId === null ? action('none') : action('play');
  }
  if (live === 'queued') return action('queued');
  if (live !== null) return action('installing');
  if (romId === null) return action('none');
  if (fromDrive) return action('install-drive');
  return connected ? action('install') : action('none');
}

/** Which main action group Details shows for a game (the left column). */
export type DetailsPrimary =
  /** No server id, an installed row: Link to server game / Remove from library. */
  | 'link'
  /** No server id and nothing installed: "This entry has no server id". */
  | 'no-id'
  /** A live install: Installing… and Cancel. */
  | 'cancel'
  /** The game is running: Stop. */
  | 'stop'
  /** Installed: Play, then Update / content / Game Settings / Uninstall. */
  | 'play'
  /** Not installed, shown from a drive: Install from drive. */
  | 'install-drive'
  /** Not installed: Install (Install App on a native platform). */
  | 'install';

export type DetailsInput = {
  romId: number | null;
  /** An installed registry row with no server id (Q4). */
  unlinked: boolean;
  /** A live install exists for the rom. */
  live: boolean;
  /** A session is running for the rom. */
  playing: boolean;
  installed: boolean;
  fromDrive: boolean;
};

/** The order Details tests them in: id, live install, running, installed, drive. */
export function detailsPrimary(input: DetailsInput): DetailsPrimary {
  if (input.romId === null) return input.unlinked ? 'link' : 'no-id';
  if (input.live) return 'cancel';
  if (input.playing) return 'stop';
  if (input.installed) return 'play';
  return input.fromDrive ? 'install-drive' : 'install';
}
