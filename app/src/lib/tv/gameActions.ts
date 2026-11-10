// What A and X do on a TV cover (README § Tabs: A selects, which opens
// Details; X plays or installs), through the shared game actions
// (`games/run.ts`), which send their own notices.
import type { DetailsSubject } from '../details/subject';
import { fromDriveGame, fromInstalled, fromSummary } from '../details/subject';
import { drives } from '../stores/drives.svelte';
import { installFromDrive, installServerGame, playInstalled } from '../games/run';
import { pushPage } from './nav.svelte';
import type { TvGame, TvQuick } from './games';
import TvDetailsStub from './pages/TvDetailsStub.svelte';
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

/**
 * Opens Details for a game (item 172). TODO(P9): TvDetails replaces the
 * stub page; the page's title is the breadcrumb ("Server › <title>").
 */
export function openDetails(subject: DetailsSubject): void {
  void pushPage({ title: subject.name, component: TvDetailsStub, props: { subject } });
}

export function openGameDetails(game: TvGame): void {
  openDetails(subjectOf(game));
}

/**
 * X on a cover: Play, Install, or Install from drive. "Launching…", "added to download queue"
 * (with Open Downloads) and every refusal are notices from `games/run.ts`.
 * TODO(P9): Play pushes the Playing page; the umu-launcher offer and the
 * library-folder prompt have no TV screen yet, so their refusal notices
 * stand alone.
 */
export async function runQuick(game: TvGame, kind: TvQuick): Promise<void> {
  if (game.romId === null) return;
  if (kind === 'play') await playInstalled(game.romId, game.title);
  else if (kind === 'install-drive') {
    // "Install from drive": copies the game off the drive with no network,
    // with the Game backups card's notices (lib/backups/install.ts).
    if (game.drive) await installFromDrive(game.drive.key, game.drive.game);
  } else await installServerGame(game.romId, game.title);
}

/**
 * A Server header chip: the system's page (item 200). TODO(P11): TvSystem
 * replaces the stub page.
 */
export function openSystem(platform: string): void {
  void pushPage({ title: platform, component: TvSystemStub, props: { platform } });
}
