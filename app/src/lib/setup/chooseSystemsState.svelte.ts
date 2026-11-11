// First-run setup's Choose your systems (ProposedApp README, "Fresh
// start"), apart from its layout: the server's systems, the ticks and picks,
// the Windows (Proton) row on Linux, the names each install is shown by,
// and Install / Skip for now. The desktop ChooseSystems and the TV startup
// screen both read it. The pure rules are `choices.ts` and `systems/proton.ts`.
import {
  api,
  type CatalogEntry,
  type CompatTool,
  type Platform,
  type PlatformSetup,
  type SetupStarted,
} from '../api';
import { isWindowsPlatform } from '../details/actions';
import { platformLabel } from '../server/platformLabel';
import { pushToast } from '../stores/toasts.svelte';
import { shortName } from '../systems/rows';
import { protonInstallSource, protonPicker } from '../systems/proton';
import {
  choices,
  compatChoice,
  installCount,
  pick,
  protonPreselect,
  sortRows,
  toggle,
  toInstall,
  type Selection,
} from './choices';

/** What Install started, with the name each install shows on the setup card. */
export type SetupDone = { started: SetupStarted; names: Record<string, string> };

export class ChooseSystems {
  readonly #windowsHost: boolean;

  /** `windowsHost`: on Windows there is no Proton row. */
  constructor(windowsHost: boolean) {
    this.#windowsHost = windowsHost;
  }

  rows = $state.raw<PlatformSetup[] | null>(null);
  loadError = $state<string | null>(null);
  selection = $state.raw<Selection>({});
  tools = $state.raw<CompatTool[]>([]);
  defaultTool = $state('');
  catalog = $state.raw<CatalogEntry[]>([]);
  platforms = $state.raw<Platform[]>([]);
  /** The Windows row's value; `null` while unticked. */
  proton = $state<string | null>(null);
  windowsLoaded = $state(false);
  busy = $state(false);
  startError = $state<string | null>(null);

  protonOptions = $derived(protonPicker(this.tools, this.catalog, this.defaultTool));
  protonDefault = $derived(protonPreselect(this.protonOptions));
  windowsGames = $derived(
    this.platforms
      .filter((p) => isWindowsPlatform(p.slug, platformLabel(p)))
      .reduce<number | null>((n, p) => (n ?? 0) + p.rom_count, null),
  );
  ticked = $derived(Object.keys(this.selection).length);
  button = $derived({
    installs: this.rows === null ? 0 : installCount(this.rows, this.selection, this.proton),
    ticked: this.ticked,
    proton: this.proton,
  });

  async load(): Promise<void> {
    this.loadError = null;
    try {
      this.rows = sortRows(await api.setupPlatformOptions());
    } catch (e) {
      this.loadError = String(e);
    }
  }

  /** The Windows row's Proton versions. Best effort: without them it offers less. */
  async loadWindows(): Promise<void> {
    if (this.#windowsHost) return;
    const [t, c, p] = await Promise.allSettled([
      api.listCompatTools(),
      api.listCompatToolCatalog(),
      api.listPlatforms(),
    ]);
    if (t.status === 'fulfilled') {
      this.tools = t.value.tools;
      this.defaultTool = t.value.default_tool;
    }
    if (c.status === 'fulfilled') this.catalog = c.value;
    if (p.status === 'fulfilled') this.platforms = p.value;
    this.windowsLoaded = true;
  }

  /** Ticks or unticks a system; ticking pre-selects GRID's recommendation. */
  tick(row: PlatformSetup): void {
    this.selection = toggle(this.selection, row);
  }

  pick(row: PlatformSetup, sourceId: string): void {
    this.selection = pick(this.selection, row, sourceId);
  }

  /** Ticks the Windows row with the preselected Proton version, or unticks it. */
  tickWindows(): void {
    this.proton = this.proton === null ? this.protonDefault : null;
  }

  pickProton(value: string): void {
    this.proton = value;
  }

  /** The name each install is shown by on the setup card. */
  installNames(): Record<string, string> {
    const names: Record<string, string> = {};
    for (const option of toInstall(this.rows ?? [], this.selection)) names[option.source_id] = shortName(option.name);
    const source = this.proton === null ? null : protonInstallSource(this.proton);
    if (source !== null) names[source] = this.catalog.find((e) => e.source_id === source)?.name ?? source;
    return names;
  }

  /** Starts the picked installs. Answers what started, or `null` on a
   *  refusal (shown as `startError`). */
  async install(): Promise<SetupDone | null> {
    this.busy = true;
    this.startError = null;
    try {
      const names = this.installNames();
      const started = await api.startPlatformSetup(choices(this.selection), compatChoice(this.proton));
      if (started.compat_set && started.queued.length === 0) {
        const tool = this.protonOptions.installed.find((o) => o.value === this.proton);
        pushToast(`Windows games will use ${tool?.label ?? 'the Proton version you picked'}`);
      }
      return { started, names };
    } catch (e) {
      this.startError = String(e);
      return null;
    } finally {
      this.busy = false;
    }
  }

  /** Skip for now. A skip that could not be saved brings the chooser back
   *  on the next start, which is the honest outcome. */
  async skip(): Promise<void> {
    this.busy = true;
    try {
      await api.skipPlatformSetup();
    } catch {
      // Not recorded; see above.
    } finally {
      this.busy = false;
    }
  }
}
