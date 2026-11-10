// The server catalog a Server screen browses: the platforms, the selected
// platform's games, the saved launch defaults (the emulator chip and the
// cards' cloud badges) and the platform's firmware status. One `Catalog`
// per Server screen, created when it mounts, so a screen that mounts again
// (after a reconnect) starts on fresh data the way it always has.
//
// It holds data and the calls that fetch it; which rail row is selected,
// the search text and the focus stay with the view.
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  api,
  EMULATOR_INSTALLED_EVENT,
  FIRMWARE_PASS_FINISHED_EVENT,
  SETUP_DEFAULTS_EVENT,
  type FirmwarePassFinished,
  type GameSummary,
  type Platform,
} from '../api';
import { cloudPlatformSet } from '../cards/badges';
import { showsChips, type FirmwareChipState } from './header';
import { platformLabel } from './platformLabel';

export class Catalog {
  platforms = $state<Platform[]>([]);
  // `$state.raw`: a platform can hold thousands of games and the list is
  // only ever replaced whole, so a deep proxy over every game (and its
  // screenshot and genre arrays) cost time on load and on every read.
  games = $state.raw<GameSummary[]>([]);
  activePlatform = $state<number | null>(null);
  // The game list belongs to the previous platform until `listGames`
  // answers; the view renders neither counts nor an empty message then.
  gamesLoading = $state(false);
  cloudPlatforms = $state<ReadonlySet<string>>(new Set<string>());
  /** Platform name -> default emulator name, for the header's emulator chip. */
  defaultEmulators = $state<Record<string, string>>({});
  firmware = $state<FirmwareChipState>(null);
  /** No library folder is set: the screen offers to set one. */
  libraryPathUnset = $state(false);

  activePlatformRow = $derived(this.platforms.find((p) => p.id === this.activePlatform) ?? null);
  /** The active platform's label, which is also the name the installed
   *  registry and the defaults know it by. */
  activePlatformName = $derived(this.activePlatformRow ? platformLabel(this.activePlatformRow) : '');
  /** Native and Windows platforms, and the Emulators shelf, show neither the
   *  emulator nor the firmware chip, and their firmware is never asked about. */
  chipsShown = $derived(showsChips(this.activePlatformName, this.activePlatformRow?.slug ?? ''));

  /** The platform row a game belongs to (Discover lists every platform's games). */
  platformOf(game: GameSummary): Platform | null {
    return this.platforms.find((p) => p.id === game.platform_id) ?? null;
  }

  /** The label a game's platform shows, which is also the name the installed
   *  registry and the defaults know it by. */
  labelFor(game: GameSummary): string {
    const row = this.platformOf(game);
    return row ? platformLabel(row) : game.platform_display_name;
  }

  /** Loads the platform list. A refusal leaves the list as it is. */
  loadPlatforms(): Promise<void> {
    return api.listPlatforms().then((p) => {
      this.platforms = p;
    });
  }

  /**
   * Selects platform `id` and loads its games. Answers true when this
   * selection's games are now shown, false when it failed (the list is
   * emptied) or a newer selection superseded it.
   */
  async selectPlatform(id: number): Promise<boolean> {
    this.activePlatform = id;
    // The header renders the new platform's name at once; `games` still
    // holds the old platform's rows until the call below answers.
    this.gamesLoading = true;
    let g: GameSummary[];
    try {
      g = await api.listGames(id);
    } catch {
      if (this.activePlatform === id) {
        this.games = [];
        this.gamesLoading = false;
      }
      return false;
    }
    if (this.activePlatform !== id) return false; // superseded by a newer selection
    this.games = g;
    this.gamesLoading = false;
    return true;
  }

  /** Re-reads the saved launch defaults: the emulator chip and the cloud badges. */
  loadLaunchDefaults(): Promise<void> {
    return api
      .getLaunchDefaults()
      .then((defaults) => {
        this.cloudPlatforms = cloudPlatformSet(defaults.default_emulators);
        this.defaultEmulators = defaults.default_emulators;
      })
      .catch(() => {
        // No defaults readable: no cloud badges and a "No default emulator"
        // chip. Both are honest fallbacks, neither is worth an error line.
      });
  }

  /** One firmware status read, kept only while `id` is still the active platform. */
  refreshFirmwareStatus(id: number, name: string): Promise<void> {
    return api
      .platformFirmwareStatus(id, name)
      .then((status) => {
        if (this.activePlatform === id) this.firmware = status;
      })
      .catch(() => {
        // Unreachable or refused. Say so: "checking…" forever would claim a
        // call is still in flight when none is, and reporting "no firmware"
        // would claim an answer we never got.
        if (this.activePlatform === id) this.firmware = 'unavailable';
      });
  }

  /** Whether a library folder is set; an unreadable config leaves the answer as is. */
  async checkLibraryPath(): Promise<void> {
    try {
      const path = await api.getLibraryPath();
      this.libraryPathUnset = path.trim() === '';
    } catch {
      // Unreadable config: leave the prompt as-is rather than nag with a
      // guess — install errors will surface the real problem if there is one.
    }
  }

  /** Saves the library folder. Throws the backend's refusal for the screen to show. */
  async saveLibraryPath(path: string): Promise<void> {
    await api.setLibraryPath(path.trim());
    this.libraryPathUnset = false;
  }

  /**
   * Follows the events that change what the catalog shows while it is on
   * screen: a firmware install finishing re-reads the active platform's
   * status, and an emulator install or first-run setup's defaults re-read
   * the launch defaults. Answers the function that stops following them.
   */
  watch(): () => void {
    let stopped = false;
    const offs: UnlistenFn[] = [];
    const keep = (p: Promise<UnlistenFn>) =>
      void p.then((un) => {
        if (stopped) un();
        else offs.push(un);
      });
    keep(
      listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (e) => {
        const id = this.activePlatform;
        if (e.payload.platform_id === id && id !== null && this.activePlatformName !== '' && this.chipsShown) {
          void this.refreshFirmwareStatus(id, this.activePlatformName);
        }
      }),
    );
    keep(listen(EMULATOR_INSTALLED_EVENT, () => void this.loadLaunchDefaults()));
    keep(listen(SETUP_DEFAULTS_EVENT, () => void this.loadLaunchDefaults()));
    return () => {
      stopped = true;
      for (const un of offs) un();
    };
  }
}
