// Settings › Systems' data and actions, apart from its layout: the server
// systems with their emulator, the emulator list and its companions, the
// install, pick, core, update, firmware (PS3 included), open, show folder
// and remove commands. The desktop SystemsPage and the TV Systems pane and
// System page both read it. One `SystemsStore` per page, created with a
// getter for whether the page is on screen (the PS3 firmware poll stops
// when it is not).
//
// Every refusal is a card (`pushToast`), as on the page before. Which rows
// are open, the ⋯ menu and the settings sheet stay with the view.
import { listen } from '@tauri-apps/api/event';
import {
  api,
  FIRMWARE_PASS_FINISHED_EVENT,
  SETUP_DEFAULTS_EVENT,
  type CompanionLink,
  type EmulatorEntry,
  type EmulatorFacts,
  type FirmwarePassFinished,
  type LaunchDefaults,
  type Platform,
  type PlatformFirmwareStatus,
  type PlatformSetup,
  type ProfileSummary,
  type Rpcs3FirmwareStatus,
  type SetupChoice,
} from '../api';
import { isWindowsPlatform } from '../details/actions';
import { defaultEmulatorKey } from '../emulators/defaults';
import {
  emulatorFirmwareRows,
  firmwareAction,
  firmwareFinishedToast,
  firmwareStartedToast,
  type EmulatorFirmwareRow,
  type FirmwareRequest,
} from '../emulators/firmware';
import { emulatorTerminalSignature } from '../emulators/installSignal';
import { expectQueued } from '../notifications/notifier';
import { platformLabel } from '../server/platformLabel';
import { downloads } from '../stores/downloads.svelte';
import { checkOncePerSession, emulatorUpdates, recheck } from '../stores/emulatorUpdates.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { SKIPPED_TOAST_MS, skippedRemovalText } from '../uninstallSkips';
import { isRpcs3Name, pollUntilInstalled, ps3FirmwareInstalled, withPs3Firmware } from './ps3Firmware';
import {
  buildSystemRows,
  installingRecommendedLabel,
  isInstalling,
  liveEmulatorSources,
  missingRecommended,
  otherEmulators,
  parsePick,
  pickerGroups,
  statusOf,
  systemFirmwareRequests,
  updateAvailable,
  type Companion,
  type SystemStatus,
  type SystemRow,
} from './rows';

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** What a pick in a system's emulator picker did. */
export type PickResult = {
  /** The picker's value now: an install pick is not the system's emulator yet. */
  selected: string;
  /** A "use" pick set the emulator: the view opens the row. */
  opened: boolean;
};

/** The "Check for updates" card. */
export function updatesFoundText(count: number): string {
  return count === 0 ? 'Every emulator is up to date.' : `${count === 1 ? '1 update' : `${count} updates`} available.`;
}

export class SystemsStore {
  readonly #active: () => boolean;

  constructor(active: () => boolean = () => true) {
    this.#active = active;
  }

  // --- Data -----------------------------------------------------------------
  platforms = $state<Platform[]>([]);
  setupRows = $state<PlatformSetup[]>([]);
  defaults = $state<LaunchDefaults | null>(null);
  emulators = $state<EmulatorEntry[]>([]);
  /** Each `companions` pairing (ShadPS4 and its Qt launcher) as installed. */
  companions = $state<CompanionLink[]>([]);
  compatible = $state<Record<string, string[]>>({});
  coreOptions = $state<Record<string, string[]>>({});
  pendingLabels = $state<string[]>([]);
  profiles = $state<ProfileSummary[]>([]);
  facts = $state<Map<string, EmulatorFacts>>(new Map());
  rpcs3Firmware = $state<Map<string, Rpcs3FirmwareStatus>>(new Map());
  loaded = $state(false);
  loadError = $state<string | null>(null);

  rows = $derived(buildSystemRows(this.setupRows, this.platforms, this.defaults, this.compatible));
  live = $derived(liveEmulatorSources(downloads.entries));
  installingRow = $derived((row: SystemRow) => isInstalling(row, this.pendingLabels, this.live));
  /** The systems with no emulator whose recommendation can be installed. */
  missing = $derived(missingRecommended(this.rows, this.installingRow));
  others = $derived(otherEmulators(this.emulators, this.rows, this.companions));
  windowsGames = $derived(
    this.platforms
      .filter((p) => isWindowsPlatform(p.slug, platformLabel(p)))
      .reduce<number | null>((n, p) => (n ?? 0) + p.rom_count, null),
  );
  /** Changes when an emulator install reaches its end: the page re-reads everything. */
  installSignature = $derived(emulatorTerminalSignature(downloads.entries));
  /** The compatibility and core answers depend on the platforms and the emulator list. */
  compatibilityKey = $derived(
    JSON.stringify([this.platforms.map((p) => [p.name, p.slug]), this.emulators.map((e) => e.name)]),
  );
  /** The `compatibilityKey` the compatible answers were read for; '' before
   *  the first answer. */
  compatibleKey = $state('');
  /** The compatible answers are in for the current platforms and emulators,
   *  so a row's emulator (or its lack of one) can be trusted. */
  compatibleReady = $derived(this.loaded && this.compatibleKey !== '' && this.compatibleKey === this.compatibilityKey);
  emulatorNames = $derived(this.emulators.map((e) => e.name).join('\n'));
  /** Changes when a firmware download completes (a PS3UPDAT.PUP may be in place). */
  firmwareCompleted = $derived(
    downloads.entries
      .filter((e) => e.kind === 'firmware' && e.status === 'completed')
      .map((e) => e.id)
      .join(','),
  );
  checking = $derived(emulatorUpdates.running);

  // --- Action state -----------------------------------------------------------
  /** Systems with an install or pick in flight, by key. */
  busyRows = $state<Record<string, true>>({});
  launching = $state<string | null>(null);
  updating = $state<string | null>(null);
  /** A missing companion's install in flight, by `source_id`. */
  companionBusy = $state<Record<string, true>>({});
  removing = $state(false);
  /** While Install recommended runs: how many emulators it is installing; 0 otherwise. */
  recommendedInstalling = $state(0);
  /** "Installing 2 emulators…" while Install recommended runs, else `''`. */
  recommendedInstallingLabel = $derived(
    this.recommendedInstalling > 0 ? installingRecommendedLabel(this.recommendedInstalling) : '',
  );

  // --- Firmware -----------------------------------------------------------------
  firmwareStatus = $state<Record<string, PlatformFirmwareStatus>>({});
  firmwarePending = $state<Record<string, true>>({});
  firmwareRequests = $derived(systemFirmwareRequests(this.rows));
  firmwareSignature = $derived(this.firmwareRequests.map((r) => r.key).join(','));

  // --- Refresh ------------------------------------------------------------------

  async refreshEmulators(): Promise<void> {
    try {
      // Best effort for the pairings: without them companions just show
      // as Other emulators.
      const [list, links] = await Promise.all([
        api.listEmulators(),
        api.emulatorCompanions().catch((): CompanionLink[] => []),
      ]);
      this.emulators = list;
      this.companions = links;
    } catch (err) {
      this.loadError = errorMessage(err);
    }
  }

  async refreshDefaults(): Promise<void> {
    try {
      this.defaults = await api.getLaunchDefaults();
    } catch (err) {
      this.loadError = errorMessage(err);
    }
  }

  async refreshSystems(): Promise<void> {
    try {
      const [p, s] = await Promise.all([api.listPlatforms(), api.setupPlatformOptions()]);
      this.platforms = p;
      this.setupRows = s;
      this.loadError = null;
    } catch (err) {
      this.loadError = errorMessage(err);
    }
  }

  async refreshPending(): Promise<void> {
    try {
      this.pendingLabels = (await api.setupStatus()).pending;
    } catch {
      // Best effort: rows just don't say Installing… from it.
    }
  }

  async refreshAll(): Promise<void> {
    await Promise.all([this.refreshEmulators(), this.refreshDefaults(), this.refreshSystems(), this.refreshPending()]);
    this.loaded = true;
  }

  /** The once-per-session emulator update check. */
  checkOnce(): Promise<void> {
    return checkOncePerSession(this.emulators);
  }

  loadProfiles(): void {
    api
      .listProfiles()
      .then((p) => (this.profiles = p))
      .catch(() => {});
  }

  /** The compatible emulators and RetroArch cores for the given platforms. */
  loadCompatibility(refs: { name: string; slug: string }[]): void {
    if (refs.length === 0) return;
    const key = this.compatibilityKey;
    api
      .compatibleEmulators(refs)
      .then((c) => {
        this.compatible = c;
        this.compatibleKey = key;
      })
      .catch(() => {});
    api
      .retroarchCoreOptions(refs)
      .then((c) => (this.coreOptions = c))
      .catch(() => {});
  }

  /** The Eden notes' file probes and the RPCS3 firmware check for `names`. */
  loadFacts(names: string[]): void {
    void Promise.all(
      names.map(async (name) => {
        try {
          return [name, await api.emulatorFacts(name)] as const;
        } catch {
          return null;
        }
      }),
    ).then((answers) => (this.facts = new Map(answers.filter((a) => a !== null))));
    void Promise.all(
      names.filter(isRpcs3Name).map(async (name) => {
        try {
          return [name, await api.rpcs3FirmwareStatus(name)] as const;
        } catch {
          return null;
        }
      }),
    ).then((answers) => (this.rpcs3Firmware = new Map(answers.filter((a) => a !== null))));
  }

  /** Setup's defaults land after each picked emulator's install. Answers the
   *  function that stops listening. */
  watchSetupDefaults(): () => void {
    const pending = listen(SETUP_DEFAULTS_EVENT, () => {
      void this.refreshDefaults();
      void this.refreshPending();
    });
    return () => {
      void pending.then((off) => off());
    };
  }

  // --- Firmware -----------------------------------------------------------------

  ps3Installed(name: string): boolean {
    return ps3FirmwareInstalled(this.rpcs3Firmware.get(name));
  }

  // RPCS3 shows its own install dialog after `--installfw`; poll briefly
  // (every 3 s for 2 minutes, only while the page is shown) so the line
  // turns to "installed" without a revisit.
  async installPs3Firmware(name: string): Promise<void> {
    let ok: boolean;
    try {
      ok = await api.installPs3Firmware(name);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
      return;
    }
    pushToast(
      ok
        ? 'PS3 firmware is installing in RPCS3'
        : 'Couldn’t open RPCS3 to install firmware.',
      ok ? 'success' : 'error',
    );
    if (!ok) return;
    await pollUntilInstalled(
      async () => {
        const status = await api.rpcs3FirmwareStatus(name);
        this.rpcs3Firmware = new Map(this.rpcs3Firmware).set(name, status);
        return status.installed;
      },
      { intervalMs: 3000, attempts: 40, stopped: () => !this.#active() },
    );
  }

  refreshFirmwareStatus(req: FirmwareRequest): void {
    api
      .platformFirmwareStatus(req.platform.id, defaultEmulatorKey(req.platform), req.emulator)
      .then((status) => (this.firmwareStatus = { ...this.firmwareStatus, [req.key]: status }))
      .catch(() => {
        const { [req.key]: _gone, ...rest } = this.firmwareStatus;
        this.firmwareStatus = rest;
      });
  }

  /** A firmware pass finishing re-reads its platforms' status; one this
   *  page started says how it ended. Answers the function that stops listening. */
  watchFirmwarePasses(): () => void {
    const pending = listen<FirmwarePassFinished>(FIRMWARE_PASS_FINISHED_EVENT, (event) => {
      const { platform_id: id, ok, failed } = event.payload;
      for (const req of this.firmwareRequests.filter((r) => r.platform.id === id)) {
        if (this.firmwarePending[req.key] === true) {
          const { [req.key]: _done, ...rest } = this.firmwarePending;
          this.firmwarePending = rest;
          const toast = firmwareFinishedToast(defaultEmulatorKey(req.platform), req.emulator, ok, failed);
          pushToast(toast.text, toast.level);
        }
        this.refreshFirmwareStatus(req);
      }
    });
    return () => {
      void pending.then((off) => off());
    };
  }

  /** The server firmware line for a system's emulator, or `null`. */
  firmwareLine(row: SystemRow): EmulatorFirmwareRow | null {
    if (row.emulator === '' || row.platform === null) return null;
    const id = row.platform.id;
    return (
      emulatorFirmwareRows(row.emulator, this.platforms, this.compatible, this.defaults, this.firmwareStatus).find(
        (f) => f.platformId === id,
      ) ?? null
    );
  }

  installFirmware(row: SystemRow): void {
    const req = this.firmwareRequests.find((r) => r.platform.id === row.platform?.id && r.emulator === row.emulator);
    if (!req) return;
    const name = defaultEmulatorKey(req.platform);
    this.firmwarePending = { ...this.firmwarePending, [req.key]: true };
    pushToast(firmwareStartedToast(name, req.emulator));
    api.installFirmwareForPlatform(req.platform.id, name, req.emulator).catch((err) => {
      const { [req.key]: _failed, ...rest } = this.firmwarePending;
      this.firmwarePending = rest;
      pushToast(`Could not install ${name} firmware: ${errorMessage(err)}`, 'error');
    });
  }

  // --- Rows -------------------------------------------------------------------------

  /** A row's status chip and, for a firmware-only install, its text. */
  rowStatus(row: SystemRow): { status: SystemStatus; text: string | null } {
    const fw = this.firmwareLine(row);
    const action = withPs3Firmware(
      fw ? firmwareAction(fw, this.firmwarePending[fw.key] === true, false) : null,
      this.ps3Installed(row.emulator),
    );
    const installing = this.installingRow(row);
    const status = statusOf({
      emulator: row.emulator,
      installing: installing || this.busyRows[row.key] === true,
      firmware: action,
      updateAvailable: updateAvailable(row.emulator, this.companions, (name) => emulatorUpdates.hasUpdate(name)),
    });
    const text = status === 'installing' && action === 'requested' && !installing ? 'Installing firmware…' : null;
    return { status, text };
  }

  // Each queued install announces itself ("Installing <name>…", one summary
  // card for several) from the downloads queue (notifications/notifier.ts).
  async install(choices: SetupChoice[]): Promise<void> {
    if (choices.length === 0) return;
    const marks = Object.fromEntries(choices.map((c) => [c.platform, true as const]));
    this.busyRows = { ...this.busyRows, ...marks };
    try {
      const result = await api.installForSystems(choices);
      for (const e of result.errors) pushToast(e, 'error');
      await Promise.all([this.refreshPending(), this.refreshDefaults()]);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      const next = { ...this.busyRows };
      for (const c of choices) delete next[c.platform];
      this.busyRows = next;
    }
  }

  installRecommended(row: SystemRow): Promise<void> {
    if (row.recommended === null) return Promise.resolve();
    return this.install([{ platform: row.key, source_id: row.recommended.source_id }]);
  }

  /** Install recommended: every missing system's recommendation at once.
   *  A second press while it runs is ignored. */
  async installAllRecommended(): Promise<void> {
    if (this.recommendedInstalling > 0) return;
    const choices = this.missing;
    if (choices.length === 0) return;
    this.recommendedInstalling = choices.length;
    try {
      await this.install(choices);
    } finally {
      this.recommendedInstalling = 0;
    }
  }

  /** A pick in a system's emulator picker: install a download, use an
   *  installed emulator, or None. */
  async pick(row: SystemRow, value: string): Promise<PickResult> {
    const pick = parsePick(value);
    let opened = false;
    if (pick.kind === 'install') {
      await this.install([{ platform: row.key, source_id: pick.sourceId }]);
    } else {
      this.busyRows = { ...this.busyRows, [row.key]: true };
      try {
        await api.setDefaultEmulator(row.key, pick.kind === 'use' ? pick.name : '');
        await Promise.all([this.refreshDefaults(), this.refreshPending()]);
        opened = pick.kind === 'use';
      } catch (err) {
        pushToast(errorMessage(err), 'error');
      } finally {
        const { [row.key]: _done, ...rest } = this.busyRows;
        this.busyRows = rest;
      }
    }
    // An install pick is not the system's emulator yet: show the real one.
    const current = this.rows.find((r) => r.key === row.key);
    return { selected: current ? pickerGroups(current).selected : value, opened };
  }

  async changeCore(row: SystemRow, core: string): Promise<void> {
    try {
      await api.setRetroarchCore(row.key, core);
      await this.refreshDefaults();
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    }
  }

  // --- The emulator line --------------------------------------------------------------

  entryFor(name: string): EmulatorEntry | undefined {
    return this.emulators.find((e) => e.name === name);
  }

  async openEmulator(name: string): Promise<void> {
    this.launching = name;
    try {
      const warning = await api.launchEmulator(name);
      if (warning) pushToast(warning, 'error');
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      this.launching = null;
    }
  }

  async updateEmulator(name: string): Promise<void> {
    this.updating = name;
    // The queue entry then reads "Updating <name>…", not "Installing".
    expectQueued(this.entryFor(name)?.source_id ?? '', { update: true });
    try {
      await api.updateEmulator(name);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      this.updating = null;
    }
  }

  /** A missing companion's Install (installing or updating its parent adds it too). */
  async installCompanion(c: Companion): Promise<void> {
    if (c.sourceId === null) return;
    const id = c.sourceId;
    this.companionBusy = { ...this.companionBusy, [id]: true };
    try {
      await api.installEmulator(id);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    } finally {
      const { [id]: _done, ...rest } = this.companionBusy;
      this.companionBusy = rest;
    }
  }

  /** Whether a companion's install is in flight (here or in Downloads). */
  companionInstalling(c: Companion): boolean {
    return c.sourceId !== null && (this.companionBusy[c.sourceId] === true || this.live.has(c.sourceId));
  }

  async checkForUpdates(): Promise<void> {
    await recheck(this.emulators);
    const count = this.emulators.filter((e) => emulatorUpdates.hasUpdate(e.name)).length;
    pushToast(updatesFoundText(count));
  }

  async showFolder(name: string): Promise<void> {
    try {
      await api.showEmulatorFolder(name);
    } catch (err) {
      pushToast(errorMessage(err), 'error');
    }
  }

  /** Removes an emulator and its files. Answers whether it went. */
  async removeEmulator(name: string): Promise<boolean> {
    this.removing = true;
    try {
      const report = await api.deleteEmulator(name);
      pushToast(skippedRemovalText(report?.skipped), 'error', SKIPPED_TOAST_MS);
      pushToast(`${name} removed`);
      return true;
    } catch (err) {
      pushToast(errorMessage(err), 'error');
      return false;
    } finally {
      this.removing = false;
    }
  }
}
