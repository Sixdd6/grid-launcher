// Pure helpers for the Emulators panel's CompatTools section (task-17-brief.md):
// grouping compat tools by kind/source, their radio-row label, and the
// Windows-host guard. No store/API imports here so this stays trivially
// unit-testable — CompatTools.svelte owns the fetching/wiring.
import type { CompatTool } from '../api';

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

// Re-exported rather than redefined: `isWindowsHost` already exists in
// details/actions.ts (task-16-brief.md) for the same purpose (reading
// `navigator.platform`, not the server's platform name). Exporting it from
// here too lets CompatTools.svelte and Emulators.svelte import the
// Windows-host guard alongside the other compat-tool helpers without a
// second implementation to keep in sync.
export { isWindowsHost } from '../details/actions';
