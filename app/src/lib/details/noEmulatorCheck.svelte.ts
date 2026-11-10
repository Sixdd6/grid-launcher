// The no-emulator check both Details run (desktop and TV; user decision
// 2026-11-10): which Systems row the game's platform is, whether it has no
// emulator, and an Install that also installs the system's recommended
// emulator. Construct it in a component's script: it re-reads the Systems
// store when mounted and whenever an emulator install ends.
import { untrack } from 'svelte';
import * as run from '../games/run';
import { expectQueuedSub } from '../notifications/notifier';
import type { SystemsStore } from '../systems/systemsStore.svelte';
import type { DetailsStore } from './detailsStore.svelte';
import { installsRecommended, noEmulatorFor, noEmulatorSub, systemRowFor, type NoEmulator } from './noEmulator';

export class NoEmulatorCheck {
  readonly #systems: SystemsStore;
  readonly #d: DetailsStore;
  readonly #sub: boolean;

  /**
   * `systems`: the Systems store to read and install through. `queuedSub`:
   * the game's queued card gets "<rec> installs with it" as its second line
   * (TV mode's cards show one; the desktop's don't).
   */
  constructor(systems: SystemsStore, d: DetailsStore, opts: { queuedSub: boolean }) {
    this.#systems = systems;
    this.#d = d;
    this.#sub = opts.queuedSub;
    $effect(() => {
      void systems.installSignature;
      untrack(() => void systems.refreshAll());
    });
    // A row's emulator is one of its compatible installed emulators: read
    // them for the platforms and emulators now listed.
    $effect(() => {
      void systems.compatibilityKey;
      systems.loadCompatibility(untrack(() => systems.platforms.map((p) => ({ name: p.name, slug: p.slug }))));
    });
  }

  /** The game's Systems row; none for a native game or one shown from a drive
   *  (a drive install brings the drive's emulator). */
  row = $derived.by(() => {
    const d = this.#d;
    if (d.isNative || d.subject.drive !== undefined) return undefined;
    return systemRowFor(this.#systems.rows, d.subject.platformName, d.platformSlug ?? '');
  });

  /** The game's system has no emulator, and what Install brings; null once
   *  installed or when it has one. */
  info = $derived.by((): NoEmulator | null => {
    const row = this.row;
    const s = this.#systems;
    // Until the compatible emulators are in, every row reads as having none.
    if (this.#d.installedNow || !row || !s.compatibleReady) return null;
    return noEmulatorFor(row, s.installingRow(row) || s.busyRows[row.key] === true);
  });

  /** Install: the game, and once it is queued the recommended emulator when
   *  the system has none. Answers the install's outcome. */
  async install(): Promise<run.RunOutcome | null> {
    const row = this.row;
    const info = this.info;
    const romId = this.#d.subject.romId;
    const sub = noEmulatorSub(info);
    if (this.#sub && sub !== null && romId !== null) expectQueuedSub(romId, sub);
    const outcome = await this.#d.install();
    if (outcome?.kind === 'ok' && installsRecommended(info) && row) void this.#systems.installRecommended(row);
    return outcome;
  }
}
