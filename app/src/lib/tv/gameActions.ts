// What A and X do on a TV cover (README § Tabs: A selects, which opens
// Details; X plays or installs), and the game actions Details shares with
// it, through the shared game actions (`games/run.ts`), which send their own
// notices. Play pushes the Playing page; a refusal that needs something
// first gets its TV prompt: the umu-launcher install (`needs-umu`) and the
// library folder (`needs-library-folder`), as the desktop offers them.
import { api } from '../api';
import type { DetailsSubject } from '../details/subject';
import { fromDriveGame, fromInstalled, fromSummary } from '../details/subject';
import { umuInstallBusy } from '../emulators/compatTools';
import { installFromDrive, installServerGame, playInstalled, type RunOutcome } from '../games/run';
import { downloads } from '../stores/downloads.svelte';
import { drives } from '../stores/drives.svelte';
import { navigation } from '../stores/navigation.svelte';
import { pushToast, TOAST_DURATION_MS } from '../stores/toasts.svelte';
import { pushPage } from './nav.svelte';
import { confirmBox } from './overlays/overlays.svelte';
import type { TvGame, TvQuick } from './games';
import TvDetails from './pages/TvDetails.svelte';
import TvFolderBrowser from './pages/TvFolderBrowser.svelte';
import TvPlaying from './pages/TvPlaying.svelte';
import TvSystemStub from './pages/TvSystemStub.svelte';

/** The Details subject for a cover's game. */
export function subjectOf(game: TvGame): DetailsSubject {
  if (game.drive) {
    const drive = drives.byKey(game.drive.key);
    if (drive) return fromDriveGame(game.drive.game, drive);
  }
  if (game.summary) return fromSummary(game.summary, game.platform, game.platformSlug);
  if (game.row) return fromInstalled(game.row);
  throw new Error('a TV game has a server game or an installed row');
}

/** Opens Details for a game (items 172, 270); its title is the breadcrumb.
 *  `sizeBytes` is the size the list knew ("Not installed · 1.2 GB") until
 *  the server's detail says. */
export function openDetails(subject: DetailsSubject, sizeBytes = 0): void {
  void pushPage({ title: subject.name, component: TvDetails, props: { subject, sizeBytes } });
}

export function openGameDetails(game: TvGame): void {
  openDetails(subjectOf(game), game.drive?.game.size ?? game.summary?.fs_size_bytes ?? 0);
}

export type FolderPick = {
  /** The page's title ("Where is <title>?", "Library folder"). */
  title: string;
  /** Where it opens; '' is the home folder. */
  start?: string;
  /** File mode (Program): files show, A on one picks it, no Y. */
  files?: boolean;
  /** The folder (or file) picked; the page has closed. */
  onPick: (path: string) => void;
};

/** The folder browser (items 349–352), one page for every path. */
export function openFolderBrowser(pick: FolderPick): void {
  void pushPage({ title: pick.title, component: TvFolderBrowser, props: { pick } });
}

/** What the Playing page needs to show a game (items 321–322). */
export type PlayingGame = { romId: number; title: string; platform: string };

/**
 * Play (item 316): "Launching <title>…" and any refusal are cards from
 * `games/run.ts`; a launch with no emulator offers "Set up <Platform>" on
 * its card (item 318). A started game pushes Playing.
 */
export async function playGame(game: PlayingGame): Promise<void> {
  const outcome = await playInstalled(game.romId, game.title, game.platform);
  if (outcome.kind === 'ok') {
    void pushPage({ title: 'Playing', component: TvPlaying, props: { game } });
    return;
  }
  promptFor(outcome);
}

/** Install from the server. A missing library folder offers to set one. */
export async function installGame(romId: number, title: string): Promise<void> {
  promptFor(await installServerGame(romId, title));
}

/** The TV prompt a refusal asks for, if any (the desktop's umu-launcher
 *  offer beside Play, and its library-folder banner). */
export function promptFor(outcome: RunOutcome): void {
  if (outcome.kind === 'needs-umu') offerUmu(outcome.reason);
  else if (outcome.kind === 'needs-library-folder') promptLibraryFolder();
}

/** The desktop offer's note once the install is queued (UmuInstallOffer). */
export const UMU_INSTALLING = 'umu-launcher is installing (see Downloads). Launch the game again when it finishes.';

/**
 * A Proton launch found no umu-run: offer the umu-launcher install, the one
 * Settings › Systems › Windows queues (UmuInstallOffer.svelte). While it is
 * already installing, say so instead.
 */
export function offerUmu(reason: string): void {
  const toDownloads = { label: 'Open Downloads', run: () => navigation.go('downloads') };
  if (umuInstallBusy(downloads.entries)) {
    pushToast(UMU_INSTALLING, 'success', TOAST_DURATION_MS, { action: toDownloads });
    return;
  }
  void confirmBox({
    title: 'Install umu-launcher?',
    body: reason,
    ok: 'Install umu-launcher',
    onOk: () => {
      api.installUmuLauncher().then(
        () => pushToast(UMU_INSTALLING, 'success', TOAST_DURATION_MS, { action: toDownloads }),
        (err: unknown) => pushToast(err instanceof Error ? err.message : String(err), 'error'),
      );
    },
  });
}

/** The desktop banner's words (Server.svelte). */
export const LIBRARY_FOLDER_LINE = 'Set a library folder to install games.';

/**
 * No library folder is set: offer to pick one in the folder browser and
 * save it, as the desktop's banner does (Browse…, Save). The install is
 * pressed again afterwards, as on the desktop.
 */
export function promptLibraryFolder(): void {
  void confirmBox({
    title: 'Set a library folder',
    body: LIBRARY_FOLDER_LINE,
    ok: 'Choose a folder…',
    onOk: () =>
      openFolderBrowser({
        title: 'Library folder',
        start: '',
        onPick: (path) => {
          api.setLibraryPath(path.trim()).catch((err: unknown) => pushToast(err instanceof Error ? err.message : String(err), 'error'));
        },
      }),
  });
}

/**
 * X on a cover: Play, Install, or Install from drive. "Launching…", "added
 * to download queue" (with Open Downloads) and every refusal are notices
 * from `games/run.ts`.
 */
export async function runQuick(game: TvGame, kind: TvQuick): Promise<void> {
  if (game.romId === null) return;
  if (kind === 'play') await playGame({ romId: game.romId, title: game.title, platform: game.platform });
  else if (kind === 'install-drive') {
    // "Install from drive": copies the game off the drive with no network,
    // with the Game backups card's notices (lib/backups/install.ts).
    if (game.drive) await installFromDrive(game.drive.key, game.drive.game);
  } else await installGame(game.romId, game.title);
}

/**
 * A Server header chip, or a launch card's "Set up <Platform>": the
 * system's page (items 200, 318). TODO(P11): TvSystem replaces the stub
 * page.
 */
export function openSystem(platform: string): void {
  void pushPage({ title: platform, component: TvSystemStub, props: { platform } });
}
