// Settings › Systems (ProposedApp README, "One row per system on the
// server"): one row per server system with its emulator picker and status.
// Pure, so vitest owns the rules; SystemsPage.svelte does the fetching.
//
// The rows come from first-run setup's answer (`setup_platform_options`):
// it already lists exactly the systems that need an emulator — never a
// native platform or the Emulators shelf — with what GRID can install for
// each, recommended first. The installed side of the picker is the
// backend's `compatible_emulators` answer, the same list the old Platform
// defaults select offered, and the emulator shown is the one a launch would
// use (`platformDefaultSelect`: the saved default, else the first
// compatible one, never one after an explicit "(none)").
import type {
  CompanionLink,
  DownloadEntry,
  EmulatorEntry,
  LaunchDefaults,
  Platform,
  PlatformSetup,
  SetupChoice,
  SetupOption,
} from '../api';
import { NO_DEFAULT_VALUE, NO_EMULATOR_MARKER, platformDefaultSelect, savedDefaultFor } from '../emulators/defaults';
import { firmwareKey, type FirmwareAction, type FirmwareRequest } from '../emulators/firmware';
import { optionLabel } from '../setup/choices';

export type SystemRow = {
  /** The display label: shown, and the key every default is stored under. */
  key: string;
  label: string;
  /** The raw server name: what `compatible_emulators` answers are keyed by. */
  name: string;
  slug: string;
  romCount: number;
  /** The server platform, for its id (firmware); `null` if it vanished. */
  platform: Platform | null;
  /** The emulator a launch would use, or `''` for none. */
  emulator: string;
  /** The user chose "None" for this system. */
  explicitNone: boolean;
  /** Installed emulators that play the system, in config order. */
  installed: string[];
  /** What GRID can install for it that is not installed, recommended first. */
  downloads: SetupOption[];
  /** The recommended option (installed or not), or `null` when nothing
   *  installable plays the system. */
  recommended: SetupOption | null;
  /** The core RetroArch would run it with, when RetroArch is offered. */
  core: string | null;
};

const byLabel = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: 'base' });

/** Every system row, systems with no emulator first, then alphabetical. */
export function buildSystemRows(
  setup: readonly PlatformSetup[],
  platforms: readonly Platform[],
  defaults: LaunchDefaults | null,
  compatible: Record<string, string[]>,
): SystemRow[] {
  const rows = setup.map((row): SystemRow => {
    const installed = compatible[row.name] ?? [];
    const recommended = row.options.find((o) => o.source_id === row.recommended) ?? null;
    const downloads = row.options.filter((o) => !o.installed);
    downloads.sort((a, b) => Number(b === recommended) - Number(a === recommended));
    const choice = platformDefaultSelect(defaults, row.label, installed);
    return {
      key: row.label,
      label: row.label,
      name: row.name,
      slug: row.slug,
      romCount: row.rom_count,
      platform: platforms.find((p) => p.name === row.name) ?? null,
      emulator: choice.selected === NO_DEFAULT_VALUE ? '' : choice.selected,
      explicitNone: savedDefaultFor(defaults?.default_emulators, row.label) === NO_EMULATOR_MARKER,
      installed,
      downloads,
      recommended,
      core: row.core,
    };
  });
  return rows.sort((a, b) => Number(a.emulator !== '') - Number(b.emulator !== '') || byLabel(a.label, b.label));
}

/** The installed emulator name an option stands for: an installed server
 *  build's entry may carry " (server)". */
export function optionEntryName(option: SetupOption): string {
  return option.entry ?? option.name;
}

/** Whether `name` is the row's recommended emulator. */
export function isRecommended(row: Pick<SystemRow, 'recommended'>, name: string): boolean {
  const rec = row.recommended;
  if (rec === null) return false;
  const folded = name.trim().toLowerCase();
  return optionEntryName(rec).toLowerCase() === folded || rec.name.toLowerCase() === folded;
}

// --- The picker --------------------------------------------------------------

/** The picker's "None" value: writes the explicit-none marker. */
export const NONE_VALUE = NO_DEFAULT_VALUE;
const USE = 'use:';
const INSTALL = 'install:';

export type PickerOption = { value: string; label: string; recommended: boolean };

export type PickerGroups = {
  /** The value shown selected. */
  selected: string;
  /** Installed emulators that play the system. */
  installed: PickerOption[];
  /** The rest it can install, recommended first. */
  download: PickerOption[];
};

const REC = ' (recommended)';

/** The picker's two groups (Installed, Download) and its selected value. */
export function pickerGroups(row: SystemRow): PickerGroups {
  return {
    selected: row.emulator === '' ? NONE_VALUE : USE + row.emulator,
    installed: row.installed.map((name) => {
      const recommended = isRecommended(row, name);
      return { value: USE + name, label: recommended ? name + REC : name, recommended };
    }),
    download: row.downloads.map((o) => {
      const recommended = o === row.recommended;
      return {
        value: INSTALL + o.source_id,
        label: `Install ${optionLabel(o, row.core)}${recommended ? REC : ''}`,
        recommended,
      };
    }),
  };
}

export type PickerChoice =
  | { kind: 'none' }
  | { kind: 'use'; name: string }
  | { kind: 'install'; sourceId: string };

/** What a picker value asks for. */
export function parsePick(value: string): PickerChoice {
  if (value.startsWith(USE)) return { kind: 'use', name: value.slice(USE.length) };
  if (value.startsWith(INSTALL)) return { kind: 'install', sourceId: value.slice(INSTALL.length) };
  return { kind: 'none' };
}

// --- Status ------------------------------------------------------------------

export type SystemStatus = 'ready' | 'update' | 'firmware' | 'installing' | 'none';

export const STATUS_LABELS: Record<SystemStatus, string> = {
  ready: 'Ready',
  update: 'Update available',
  firmware: 'Firmware needed',
  installing: 'Installing…',
  none: 'No emulator',
};

export type StatusInput = {
  emulator: string;
  /** Its emulator is being installed. */
  installing: boolean;
  /** The system's firmware line, when it has one. */
  firmware: FirmwareAction | null;
  /** A newer build of its emulator is out. */
  updateAvailable: boolean;
};

/** One status per row, the most pressing first. */
export function statusOf(input: StatusInput): SystemStatus {
  if (input.installing) return 'installing';
  if (input.emulator === '') return 'none';
  if (input.firmware === 'requested') return 'installing';
  if (input.firmware === 'install') return 'firmware';
  if (input.updateAvailable) return 'update';
  return 'ready';
}

const LIVE = new Set<DownloadEntry['status']>(['queued', 'downloading', 'installing', 'cancelling']);

/** The `source_id`s of every emulator install still running. */
export function liveEmulatorSources(entries: readonly DownloadEntry[]): Set<string> {
  return new Set(entries.filter((e) => e.job === 'emulator' && LIVE.has(e.status)).map((e) => e.source_id));
}

/**
 * Whether a system's emulator is installing: an install picked for it is
 * still waiting (setup's pending defaults, `pending` holds their labels)
 * and one of the emulators it can install has a running Downloads row.
 */
export function isInstalling(
  row: Pick<SystemRow, 'key' | 'downloads'>,
  pending: readonly string[],
  live: ReadonlySet<string>,
): boolean {
  const folded = row.key.toLowerCase();
  if (!pending.some((p) => p.toLowerCase() === folded)) return false;
  return row.downloads.some((o) => live.has(o.source_id));
}

/** Install recommended (N): one choice per system with no emulator whose
 *  recommended emulator GRID can install, skipping any already installing. */
export function missingRecommended(
  rows: readonly SystemRow[],
  installing: (row: SystemRow) => boolean,
): SetupChoice[] {
  return rows
    .filter((r) => r.emulator === '' && r.recommended !== null && !installing(r))
    .map((r) => ({ platform: r.key, source_id: r.recommended!.source_id }));
}

/** The header button's label: "Install recommended (3)". */
export function installRecommendedLabel(count: number): string {
  return `Install recommended (${count})`;
}

// --- Emulators across systems -------------------------------------------------

/** The other systems that use `name`, by label. */
export function alsoPlays(name: string, rows: readonly SystemRow[], exceptKey: string): string[] {
  return rows.filter((r) => r.emulator === name && r.key !== exceptKey).map((r) => r.label);
}

/** "Also plays Dreamcast, Saturn", or `''` when no other system uses it. */
export function alsoPlaysText(labels: readonly string[]): string {
  return labels.length === 0 ? '' : `Also plays ${labels.join(', ')}`;
}

/** Installed emulators no system uses: the "Other emulators" list. A
 *  companion whose parent is some system's emulator is shown under that
 *  system's row instead, so it is never here. */
export function otherEmulators(
  emulators: readonly EmulatorEntry[],
  rows: readonly SystemRow[],
  links: readonly CompanionLink[] = [],
): EmulatorEntry[] {
  const used = new Set(rows.map((r) => r.emulator).filter((n) => n !== ''));
  const shown = new Set(
    links
      .filter((l) => l.companion_entry !== null && l.parent_entry !== null && used.has(l.parent_entry))
      .map((l) => l.companion_entry!),
  );
  return emulators.filter((e) => !used.has(e.name) && !shown.has(e.name));
}

/** The systems `name` could play, by label. */
export function canPlay(name: string, rows: readonly SystemRow[]): string[] {
  return rows.filter((r) => r.installed.includes(name)).map((r) => r.label);
}

/** The line under an emulator no system uses. A companion says what it
 *  belongs to (`companionOf`, from `companionParent`) instead. */
export function otherEmulatorNote(labels: readonly string[], companionOf: string | null = null): string {
  if (companionOf !== null) return `Companion to ${companionOf}.`;
  return labels.length === 0
    ? 'Not used by any system on your server.'
    : `Not used by any system. It can play ${labels.join(', ')}.`;
}

// --- Companions ----------------------------------------------------------------

/** One companion under an emulator's line (ShadPS4's Qt launcher). */
export type Companion = {
  /** The companion's profile name: its label while it is not installed. */
  profile: string;
  /** Its installed entry, or `null` when it is missing. */
  entry: string | null;
  /** What Install takes; `null` when GRID can't install it here. */
  sourceId: string | null;
};

/** The companions shown under `emulator`'s line, one per companion profile. */
export function companionsOf(emulator: string, links: readonly CompanionLink[]): Companion[] {
  if (emulator === '') return [];
  const out: Companion[] = [];
  for (const l of links) {
    if (l.parent_entry !== emulator || out.some((c) => c.profile === l.companion_profile)) continue;
    out.push({ profile: l.companion_profile, entry: l.companion_entry, sourceId: l.companion_source_id });
  }
  return out;
}

/** What an installed companion belongs to, for its Other emulators note:
 *  its parent's installed entry, else the parent profile; `null` when
 *  `name` is nobody's companion. */
export function companionParent(name: string, links: readonly CompanionLink[]): string | null {
  const link = links.find((l) => l.companion_entry === name);
  return link ? (link.parent_entry ?? link.parent_profile) : null;
}

/** Whether a newer build is out for `emulator` or any installed companion
 *  of it: either reads as "Update available" on the system's row. */
export function updateAvailable(
  emulator: string,
  links: readonly CompanionLink[],
  hasUpdate: (name: string) => boolean,
): boolean {
  if (emulator === '') return false;
  return hasUpdate(emulator) || companionsOf(emulator, links).some((c) => c.entry !== null && hasUpdate(c.entry));
}

/** The line for a companion that is not installed. */
export function missingCompanionText(companion: Pick<Companion, 'profile'>, emulator: string): string {
  return `${companion.profile} is not installed. Installing or updating ${shortName(emulator)} adds it too.`;
}

/** The firmware status reads the Systems page needs: each system's own
 *  emulator, on that system. */
export function systemFirmwareRequests(rows: readonly SystemRow[]): FirmwareRequest[] {
  return rows.flatMap((r) =>
    r.emulator !== '' && r.platform !== null
      ? [{ key: firmwareKey(r.emulator, r.platform.id), emulator: r.emulator, platform: r.platform }]
      : [],
  );
}

/** The no-emulator row's body: why games can't start and what fixes it. */
export function missingText(row: Pick<SystemRow, 'recommended'>): string {
  if (row.recommended === null) {
    return 'No emulator GRID installs plays this system. If you have one, use Add an emulator manually.';
  }
  return `Games for this system can't start yet. ${shortName(row.recommended.name)} is recommended.`;
}

/** A profile name without its "(System)" suffix: "PCSX2 (Playstation 2)" → "PCSX2". */
export function shortName(name: string): string {
  return name.replace(/\s*\(.*\)\s*$/, '');
}

/** A row's rom count line. */
export function gamesText(count: number): string {
  return count === 1 ? '1 game' : `${count} games`;
}

/** The version line beside an emulator's name: the checked version (with
 *  "up to date" once a check said so), else the installed release tag. */
export function versionText(
  entry: Pick<EmulatorEntry, 'source_installed_tag' | 'source_release_tag'> | undefined,
  check: { installed_display: string; up_to_date: boolean } | undefined,
): string {
  const checked = check?.installed_display.trim() ?? '';
  const version =
    (checked.toLowerCase() === 'unknown' ? '' : checked) ||
    entry?.source_installed_tag?.trim() ||
    entry?.source_release_tag?.trim() ||
    '';
  if (check?.up_to_date) return version ? `${version} · up to date` : 'up to date';
  return version;
}

/** A test-id-safe key: lower case, runs of anything else as one dash. */
export function testKeyOf(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}
