// Pure helpers for the Emulators panel's CompatTools section (task-17-brief.md):
// grouping compat tools by kind/source, their radio-row label, the
// Windows-host guard, and (fix round 1) the catalog's terminal-signature /
// live-source-id tracking. No store/API imports here so this stays trivially
// unit-testable — CompatTools.svelte owns the fetching/wiring.
import type { CompatTool, DownloadEntry } from '../api';

export type CompatGroup = { title: 'Wine' | 'Proton (system)' | 'Managed'; tools: CompatTool[] };

/**
 * Groups `tools` into up to three sections, in a fixed display order, with
 * empty groups omitted:
 *  - `kind === 'wine'` → "Wine" (any source — system wine builds are the only
 *    ones the app expects to see here)
 *  - `kind === 'proton' && source === 'steam'` → "Proton (system)"
 *  - everything else (in practice `kind === 'proton'` from a managed
 *    install) → "Managed"
 */
export function groupCompatTools(tools: CompatTool[]): CompatGroup[] {
  const wine = tools.filter((t) => t.kind === 'wine');
  const protonSystem = tools.filter((t) => t.kind === 'proton' && t.source === 'steam');
  const managed = tools.filter((t) => t.kind !== 'wine' && !(t.kind === 'proton' && t.source === 'steam'));

  const groups: CompatGroup[] = [];
  if (wine.length > 0) groups.push({ title: 'Wine', tools: wine });
  if (protonSystem.length > 0) groups.push({ title: 'Proton (system)', tools: protonSystem });
  if (managed.length > 0) groups.push({ title: 'Managed', tools: managed });
  return groups;
}

/**
 * The default-tool radio row's label. The backend already names the system
 * wine entry "Wine (system)" (see `CompatTool.name` in api.ts) — this
 * function does not re-derive that suffix for wine, it only appends
 * " (system)" itself for a steam-sourced tool. Every other case (managed
 * installs, and wine) reads as "<name> — <path>".
 */
export function compatToolLabel(tool: CompatTool): string {
  if (tool.source === 'steam') {
    return `${tool.name} (system) — ${tool.path}`;
  }
  return `${tool.name} — ${tool.path}`;
}

const TERMINAL_STATUSES: DownloadEntry['status'][] = ['completed', 'failed', 'cancelled'];
const LIVE_STATUSES: DownloadEntry['status'][] = ['queued', 'downloading', 'installing', 'cancelling'];

/**
 * Signature of every `compat_tool`-kind drawer entry that has reached a
 * terminal status — mirrors `emulatorTerminalSignature` in Emulators.svelte
 * (task-7-brief.md), scoped to `kind === 'compat_tool'` instead of
 * `job === 'emulator'`. Read inside a `$effect` (approximate on purpose,
 * same as the emulator catalog: any terminal compat_tool entry is signal
 * enough, not just the one just installed) so a fresh terminal entry — an
 * install completing, failing, or getting cancelled — triggers a catalog
 * re-fetch, keeping the Install/Installed buttons from going stale
 * (fix round 1 finding: the catalog previously only loaded once at mount).
 */
export function compatToolTerminalSignature(entries: DownloadEntry[]): string {
  return entries
    .filter((e) => e.kind === 'compat_tool' && TERMINAL_STATUSES.includes(e.status))
    .map((e) => `${e.id}:${e.status}`)
    .join(',');
}

/**
 * The `source_id`s of every `compat_tool`-kind drawer entry that is still
 * live (queued/downloading/installing/cancelling). CompatTools.svelte uses
 * this to keep a catalog row's Install button disabled for the whole
 * background install — not just while the initial `installCompatTool` call
 * itself is in flight — so a duplicate click during the install is
 * impossible (fix round 1 finding).
 */
export function liveCompatToolSourceIds(entries: DownloadEntry[]): Set<string> {
  return new Set(
    entries.filter((e) => e.kind === 'compat_tool' && LIVE_STATUSES.includes(e.status)).map((e) => e.source_id)
  );
}

// Re-exported rather than redefined: `isWindowsHost` already exists in
// details/actions.ts (task-16-brief.md) for the same purpose (reading
// `navigator.platform`, not the server's platform name). Exporting it from
// here too lets CompatTools.svelte and Emulators.svelte import the
// Windows-host guard alongside the other compat-tool helpers without a
// second implementation to keep in sync.
export { isWindowsHost } from '../details/actions';
