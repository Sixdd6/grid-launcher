// TV mode's one Settings › Systems store (systems/systemsStore.svelte.ts),
// shared by the Systems pane, the System page and the emulator form. A tab
// or page remounts on every switch, so the store lives here: an install,
// "Installing N emulators…" and the lists survive going into a System page
// and back. `useTvSystems()` runs, while the calling screen is mounted,
// what the desktop SystemsPage runs while it is shown (refresh after an
// install ends, the update check once per session, compatibility, facts,
// firmware status, the Proton catalog). The setup-defaults and
// firmware-pass listeners stay on for the session, so a firmware install
// started on a System page still says how it ended after leaving it.
import { untrack } from 'svelte';
import { api, type CatalogEntry } from '../api';
import { compatToolTerminalSignature } from '../emulators/compatTools';
import { expectQueued } from '../notifications/notifier';
import { compatTools, refresh as refreshCompatTools } from '../stores/compatTools.svelte';
import { downloads } from '../stores/downloads.svelte';
import { pushToast } from '../stores/toasts.svelte';
import { protonInstalling, protonInstallSource, protonPicker, protonStatus } from '../systems/proton';
import { NONE_VALUE, pickerGroups, type SystemRow } from '../systems/rows';
import { SystemsStore } from '../systems/systemsStore.svelte';
import { openList } from './overlays/overlays.svelte';
import { emulatorListItems, protonListItems } from './pages/systemPage';

let users = $state(0);

export const tvSystems = new SystemsStore(() => users > 0);

/** The Proton catalog (the Windows page's Download group) and whether a
 *  Proton pick is being applied. */
export const tvProton = $state<{ catalog: CatalogEntry[]; pending: boolean }>({ catalog: [], pending: false });

let listening = false;

function listenOnce(): void {
  if (listening) return;
  listening = true;
  tvSystems.watchSetupDefaults();
  tvSystems.watchFirmwarePasses();
}

/** Call from a screen's script: keeps the shared store fresh while it is
 *  mounted, and answers it. */
export function useTvSystems(): SystemsStore {
  const s = tvSystems;
  listenOnce();

  $effect(() => {
    untrack(() => (users += 1));
    return () => untrack(() => (users -= 1));
  });

  // Mounted, or an emulator install reached its end: re-read everything.
  $effect(() => {
    void s.installSignature;
    untrack(() => void s.refreshAll());
  });

  // The once-per-session update check, once the emulator list is in.
  $effect(() => {
    if (!s.loaded) return;
    untrack(() => void s.checkOnce());
  });

  $effect(() => {
    untrack(() => s.loadProfiles());
  });

  $effect(() => {
    void s.compatibilityKey;
    s.loadCompatibility(untrack(() => s.platforms.map((p) => ({ name: p.name, slug: p.slug }))));
  });

  $effect(() => {
    void s.emulatorNames;
    void s.firmwareCompleted;
    s.loadFacts(untrack(() => s.emulators.map((e) => e.name)));
  });

  $effect(() => {
    void s.firmwareSignature;
    for (const req of untrack(() => s.firmwareRequests)) s.refreshFirmwareStatus(req);
  });

  // The catalog's Installed flags change when a compat-tool install ends.
  $effect(() => {
    void compatToolTerminalSignature(downloads.entries);
    api
      .listCompatToolCatalog()
      .then((entries) => (tvProton.catalog = entries))
      .catch(() => {});
  });

  return s;
}

/** The Windows (Proton) system now: its picker, whether a Proton download
 *  runs, its status and the default tool's label. Read inside `$derived`. */
export function windowsNow() {
  const picker = protonPicker(compatTools.tools, tvProton.catalog, compatTools.defaultTool);
  const installing = protonInstalling(downloads.entries, tvProton.catalog);
  return {
    picker,
    installing,
    status: protonStatus(picker.unset, installing),
    tool: picker.unset ? null : (picker.installed.find((o) => o.value === picker.selected)?.label ?? null),
  };
}

const errText = (err: unknown) => (err instanceof Error ? err.message : String(err));

/** A pick in the Proton list (WindowsRow.svelte's): an installed tool
 *  becomes the default; a Download one installs and becomes the default
 *  when it finishes. */
async function pickProton(value: string): Promise<void> {
  const source = protonInstallSource(value);
  const picker = windowsNow().picker;
  tvProton.pending = true;
  try {
    if (source === null) {
      await api.setDefaultCompatTool(value);
      await refreshCompatTools();
      const tool = picker.installed.find((o) => o.value === value);
      pushToast(`Windows games now use ${tool?.label ?? value}`);
    } else {
      const entry = tvProton.catalog.find((e) => e.source_id === source);
      expectQueued(source, { done: `${entry?.name ?? source} installed and used for Windows games` });
      await api.installCompatToolAsDefault(source);
      await refreshCompatTools();
    }
  } catch (err) {
    pushToast(errText(err), 'error');
  } finally {
    tvProton.pending = false;
  }
}

/** "Default Proton version" (336): Installed and Download groups. */
export function openProtonList(): void {
  const picker = windowsNow().picker;
  void openList({ title: 'Default Proton version', items: protonListItems(picker, (v) => void pickProton(v)) });
}

/** "Emulator for <system>" (327–328): picking a Download one installs it;
 *  an installed one becomes the system's emulator; None stores the
 *  explicit none. The choices are only the system's own (rows.ts). */
export function openEmulatorList(row: SystemRow): void {
  void openList({
    title: `Emulator for ${row.label}`,
    items: emulatorListItems(pickerGroups(row), NONE_VALUE, (v) => void tvSystems.pick(row, v)),
  });
}
