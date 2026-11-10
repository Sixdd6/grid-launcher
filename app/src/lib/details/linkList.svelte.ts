// Q4 "Link to server game": the server ROMs on the platform an unlinked
// installed row names, and the pick that links the row. The desktop
// LinkPicker and the TV link page both read it.
//
// Kept apart from `detailsStore.svelte.ts` on purpose: it needs only
// `api.listServerRomsForPlatform`, so the picker renders on its own (its
// server-render test stubs nothing else).
import { api, type GameSummary } from '../api';
import { linkCandidates } from './unlinked';

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export class LinkList {
  /** `null` until the list arrives. */
  games = $state<GameSummary[] | null>(null);
  loadError = $state<string | null>(null);
  /** The rom being linked; one at a time. */
  pickingId = $state<number | null>(null);
  pickError = $state<string | null>(null);

  /** Loads `platform`'s server ROMs. Answers the function that drops a
   *  pending answer (the screen closed meanwhile). */
  load(platform: string): () => void {
    let cancelled = false;
    api
      .listServerRomsForPlatform(platform)
      .then((list) => {
        if (!cancelled) this.games = list;
      })
      .catch((err) => {
        if (!cancelled) this.loadError = errorMessage(err);
      });
    return () => {
      cancelled = true;
    };
  }

  /** The rows to offer for `query`, leaving out the roms other rows hold. */
  shown(query: string, held: ReadonlySet<number>): GameSummary[] {
    return this.games === null ? [] : linkCandidates(this.games, query, held);
  }

  /** Links `rom` through `link`; a rejection lands in `pickError`. A second
   *  pick while one runs is ignored. */
  async pick(rom: GameSummary, link: (rom: GameSummary) => Promise<void>): Promise<void> {
    if (this.pickingId !== null) return;
    this.pickingId = rom.id;
    this.pickError = null;
    try {
      await link(rom);
    } catch (err) {
      this.pickError = errorMessage(err);
    } finally {
      this.pickingId = null;
    }
  }
}
