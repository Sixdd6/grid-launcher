// Library › RomM › Emulators (RommStorefront README "RomM › Emulators";
// TVRommStorefront README, same rules and words): the rail row, the row
// states and the words. Pure; desktop and TV share it.
import type {
  DownloadEntry,
  Platform,
  RommEmulator,
  RommEmulators,
  RommEmulatorsView,
  RommOtherFile,
  RommOtherReason,
  RommOtherWhy,
} from '../api';
import { sizeText } from '../backups/dialog';
import { plural } from '../gamestores/text';
import { platformLabel } from '../server/platformLabel';
import { isEmulatorsShelf, platformRailKey, railSlugs, serverPlatformSlug } from './platformKey';
import type { RommRailRow } from './rail';

/** Where the view's list is: loading, failed (Can't load), or here. */
export type EmulatorsLoad =
  | { phase: 'loading' }
  | { phase: 'error'; message: string }
  | { phase: 'ready'; data: RommEmulators };

/** The server's Emulators platform, when it lists one with files. */
export function emulatorsPlatform(platforms: readonly Platform[]): Platform | null {
  return platforms.find((p) => isEmulatorsShelf(p) && p.rom_count > 0) ?? null;
}

/** The view's count: emulators plus other files this computer can install. */
export function emulatorsCount(view: RommEmulatorsView | null): number {
  return view === null ? 0 : view.emulators.length + view.other_files.length;
}

/**
 * The rail row for the Emulators platform, or `null` when it does not show:
 * no such platform (or no files on it), or no load of its list started yet. While the list loads or failed the
 * row shows without a count; loaded, its count is what this computer can
 * install (0 when nothing is for it).
 */
export function emulatorsRailRow(platforms: readonly Platform[], load: EmulatorsLoad | null): RommRailRow | null {
  const p = emulatorsPlatform(platforms);
  if (p === null || load === null) return null;
  if (load.phase === 'ready' && (!load.data.present || load.data.file_count === 0)) return null;
  const slug = railSlugs(platforms).get(p.id) ?? serverPlatformSlug(p);
  const count = load.phase === 'ready' ? emulatorsCount(load.data.view) : null;
  return {
    key: platformRailKey(slug),
    kind: 'emulators',
    label: platformLabel(p) || 'Emulators',
    count,
    testId: `library-rail-platform-${slug}`,
    countTestId: count === null ? null : `library-rail-count-platform-${slug}`,
    slug,
    platform: p,
  };
}

// ---- row states -----------------------------------------------------------

/** What an emulator or other-file row shows. */
export type EmuRowState =
  | { kind: 'none' }
  | { kind: 'queued' }
  | { kind: 'downloading'; pct: number; done: number; total: number }
  | { kind: 'installing' }
  | { kind: 'installed' }
  | { kind: 'update' }
  /** `builtFor`: the program's machine ("Linux ARM64") for a wrong-processor
   *  failure, else `null` and `message` is the backend's text. */
  | { kind: 'failed'; builtFor: string | null; message: string };

/** The newest Downloads row for `sourceId` (a Retry adds a new row). */
export function latestEntry(entries: readonly DownloadEntry[], sourceId: string): DownloadEntry | null {
  let best: DownloadEntry | null = null;
  for (const e of entries) {
    if (e.job !== 'emulator' || e.source_id !== sourceId) continue;
    if (best === null || e.id > best.id) best = e;
  }
  return best;
}

/** The `source_id` an Other file's Downloads row carries (`romm-file:<rom>/<file>`). */
export function otherFileSourceId(f: Pick<RommOtherFile, 'rom_id' | 'file_id'>): string {
  return `romm-file:${f.rom_id}/${f.file_id}`;
}

/** A live or failed Downloads row's state; `null` for none, or a finished
 *  or cancelled one (the view's own answer then holds). */
function entryState(e: DownloadEntry | null): EmuRowState | null {
  if (e === null) return null;
  switch (e.status) {
    case 'queued':
      return { kind: 'queued' };
    case 'downloading':
    case 'cancelling': {
      const total = Math.max(0, e.total_bytes);
      const done = Math.max(0, e.downloaded_bytes);
      const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
      return { kind: 'downloading', pct, done, total };
    }
    case 'installing':
      return { kind: 'installing' };
    case 'failed':
      return {
        kind: 'failed',
        builtFor: e.failure?.code === 'wrong_machine' && e.failure.reason.trim() !== '' ? e.failure.reason.trim() : null,
        message: e.error.trim(),
      };
    default:
      return null;
  }
}

/** An emulator row's state: its Downloads row while one is live or failed,
 *  else installed, update or not installed. */
export function emulatorRowState(e: RommEmulator, entries: readonly DownloadEntry[]): EmuRowState {
  const live = entryState(latestEntry(entries, e.source_id));
  if (live !== null) return live;
  if (e.entry === null) return { kind: 'none' };
  return e.update ? { kind: 'update' } : { kind: 'installed' };
}

export function otherFileRowState(f: RommOtherFile, entries: readonly DownloadEntry[]): EmuRowState {
  const live = entryState(latestEntry(entries, otherFileSourceId(f)));
  if (live !== null) return live;
  return f.installed ? { kind: 'installed' } : { kind: 'none' };
}

/** "Installed" counts emulators installed at any version, plus installed other files. */
export function installedCount(view: RommEmulatorsView): number {
  return view.emulators.filter((e) => e.entry !== null).length + view.other_files.filter((f) => f.installed).length;
}

// ---- search ----------------------------------------------------------------

/** The search matches an emulator's name, systems, file name and folder. */
export function emulatorMatches(e: RommEmulator, query: string): boolean {
  const q = query.trim().toLowerCase();
  return q === '' || [e.name, e.systems, e.file_name, e.folder].some((s) => s.toLowerCase().includes(q));
}

export function otherFileMatches(f: RommOtherFile, query: string): boolean {
  const q = query.trim().toLowerCase();
  return q === '' || [f.file_name, f.folder].some((s) => s.toLowerCase().includes(q));
}

// ---- words -----------------------------------------------------------------

export const EMULATORS_SEARCH = 'Search emulators or files';
export const EMULATORS_SEARCH_FOLDED = 'Search Emulators…';
export const EMULATORS_NOTE =
  'Named for GRID. Each installs as an emulator that Settings › Systems sets up, and RomM’s newer builds show as updates.';
export const OTHER_FILES_NOTE =
  'These don’t follow GRID’s naming, so GRID installs them as plain files in your library’s Emulators folder. GRID doesn’t set them up for a system or check them for updates.';
export const BUILDS_CHIP_TITLE = 'GRID installs the newest build for this computer’s system and processor';

/** The counts line under the title. */
export function emulatorsCountsLine(load: EmulatorsLoad, host: string): string {
  if (load.phase === 'loading') return 'Loading…';
  if (load.phase === 'error') return `On ${host}`;
  const view = load.data.view;
  if (view === null || emulatorsCount(view) === 0) return 'No builds for this computer';
  return `${plural(view.emulators.length, 'emulator')} · ${plural(view.other_files.length, 'other file')} · ${installedCount(view)} installed`;
}

/** TV's counts line while a search narrows the list: "2 of 8 items match". */
export function emulatorsMatchLine(shown: number, total: number): string {
  return `${shown} of ${plural(total, 'item')} match`;
}

/** "v2.0.2". */
export function versionText(v: string): string {
  return v.startsWith('v') ? v : `v${v}`;
}

/** An emulator row's line: "v2.0.2 · 38 MB". */
export function emulatorMetaLine(e: RommEmulator): string {
  return `${versionText(e.version)} · ${sizeText(e.size_bytes)}`;
}

/** "Downloading · 46% · 17 of 38 MB" (the unit once when both share it). */
export function downloadingLine(pct: number, done: number, total: number): string {
  const a = sizeText(done);
  const b = sizeText(total);
  const unit = (t: string) => t.split(' ').pop() ?? '';
  const got = unit(a) === unit(b) ? a.slice(0, -unit(a).length - 1) : a;
  return `Downloading · ${pct}% · ${got} of ${b}`;
}

/** Why a file is in Other files. */
export function otherWhyText(why: RommOtherWhy): string {
  switch (why.kind) {
    case 'no_arch_tag':
      return 'It has no arch tag';
    case 'no_processor':
      return 'Its arch tag has no processor';
    case 'unknown_emulator':
      return `GRID doesn’t know an emulator called “${why.value}”`;
    case 'extension':
      return `GRID doesn’t unpack ${why.value} files`;
    default:
      return 'Not named for GRID';
  }
}

/** "xbox folder · 18 MB · Not named for GRID". */
export function otherFileLine(f: RommOtherFile): string {
  return `${f.folder} folder · ${sizeText(f.size_bytes)} · ${otherWhyText(f.why)}`;
}

/** Why GRID doesn't install a build here: "For Windows x64", "Older than v2.0.2". */
export function otherReasonText(r: RommOtherReason): string {
  return r.kind === 'older' ? `Older than ${versionText(r.value)}` : `For ${r.value}`;
}

/** The disclosure: "6 other builds GRID doesn’t install on this computer". */
export function otherBuildsSummary(n: number): string {
  return `${plural(n, 'other build')} GRID doesn’t install on this computer`;
}

export function otherBuildsNote(host: string): string {
  return `GRID installs the newest build for this computer’s system and processor (${host}).`;
}

/** The Nothing-for-this-computer panel's line. */
export function nothingHereText(builds: number, host: string): string {
  return `RomM’s Emulators platform has ${plural(builds, 'build')}, all for other systems or processors. GRID installs builds for ${host}.`;
}

export function loadingText(host: string): string {
  return `Loading the Emulators platform from ${host}…`;
}

export function cantLoadText(host: string): string {
  return `${host} didn’t answer. Installed emulators still work.`;
}

export function noMatchText(query: string): string {
  return `No emulators or files match “${query.trim()}”`;
}

/** The catalog-copy line under a not-installed emulator. */
export function catalogCopyLine(e: RommEmulator): string {
  return `GRID’s catalog copy of ${e.name} is installed. This build installs beside it as “${e.install_as}”.`;
}

/** The wrong-processor line on a failed row. */
export function wrongMachineLine(builtFor: string, host: string): string {
  return `Couldn’t install. The program in this file is built for ${builtFor}, not ${host}. Check the file’s arch tag on RomM. Nothing was set up.`;
}

/** An emulator's Uninstall confirm. */
export function emulatorUninstallConfirm(e: RommEmulator): { title: string; body: string } {
  const who = e.many_systems ? `Games set to ${e.name}` : `${e.systems} games set to it`;
  return {
    title: `Uninstall ${e.name} ${versionText(e.installed_version ?? e.version)}?`,
    body: `GRID deletes it from the emulators folder in your library. Games and saves stay. ${who} can’t start until another emulator is picked.`,
  };
}

export function otherFileUninstallConfirm(f: RommOtherFile): { title: string; body: string } {
  return { title: `Uninstall ${f.file_name}?`, body: 'GRID deletes it from the Emulators folder in your library.' };
}

/** The cards an emulator install says (desktop text; TV text and sub). */
export function emulatorInstalledCard(e: RommEmulator, update: boolean): { text: string; tvText: string; sub?: string } {
  if (update) {
    const text = `${e.name} updated to ${versionText(e.version)}`;
    return { text, tvText: text };
  }
  const systems = e.many_systems ? 'its systems' : e.systems;
  return {
    text: `${e.name} ${versionText(e.version)} installed from RomM · set up for ${systems}`,
    tvText: `${e.name} ${versionText(e.version)} installed from RomM`,
    sub: `Set up for ${systems} in Settings › Systems`,
  };
}

export function otherFileInstalledCard(f: RommOtherFile): { text: string; tvText: string; sub: string } {
  return {
    text: `${f.file_name} installed in your library’s Emulators folder`,
    tvText: `${f.file_name} installed`,
    sub: 'In your library’s Emulators folder',
  };
}

// ---- TV mode's words (TVRommStorefront README "RomM › Emulators") -----------

export const TV_SEARCH_TITLE = 'Search Emulators';

/** An emulator row's line: "PlayStation 2 · v2.0.2 · 38 MB"; with an update,
 *  "PlayStation 2 · installed v1.6.0 · v2.0.2 is on RomM". */
export function tvEmulatorLine(e: RommEmulator, s: EmuRowState): string {
  if (s.kind === 'update') return `${e.systems} · installed ${versionText(e.installed_version ?? '')} · ${versionText(e.version)} is on RomM`;
  return `${e.systems} · ${versionText(e.version)} · ${sizeText(e.size_bytes)}`;
}

export function tvCatalogCopyLine(e: RommEmulator): string {
  return `GRID’s catalog copy is installed. This build installs beside it as “${e.install_as}”.`;
}

export function tvWrongMachineLine(builtFor: string, host: string): string {
  return `Couldn’t install: its program is built for ${builtFor}, not ${host}. Check the file’s arch tag on RomM.`;
}

/** A on an Installing row says why it does nothing. */
export function tvInstallingWhy(label: string): string {
  return `GRID is checking ${label}’s program. It finishes in a moment.`;
}

export function tvCancelConfirm(label: string): { title: string; body: string } {
  return { title: `Cancel the ${label} install?`, body: 'The download stops and its files are removed. You can install it again later.' };
}

/** The Other builds row and page. */
export const TV_OTHER_BUILDS_ROW = { label: 'Other builds', sub: 'For other computers, or older. GRID doesn’t install these here.' };

export function tvOtherBuildsSub(host: string): string {
  return `On RomM’s Emulators platform. GRID installs the newest build for this computer’s system and processor (${host}), so it doesn’t install these here.`;
}

/** A on an other build: "For Windows x64. GRID installs builds for Linux x64 on this computer." */
export function tvOtherBuildWhy(r: RommOtherReason, host: string): string {
  return `${otherReasonText(r)}. GRID installs builds for ${host} on this computer.`;
}
