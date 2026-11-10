// A native (PC) game's settings — executable, launch parameters and compat
// tool — loaded, saved and reloaded from the server's game.json. The
// desktop Game Settings dialog and the TV Game settings page both call
// these; each keeps its own form state.
import { api, type CompatTool, type NativeGameSettings } from '../api';

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** The compat tool list: the tools and the one to preselect, or why it
 *  could not be read. */
export type CompatToolsLoad = { tools: CompatTool[]; selected: string } | { error: string };

export type NativeSettingsLoad = {
  settings: NativeGameSettings;
  /** The saved executable, else the first candidate, else `''`. */
  executable: string;
  parameters: string;
  /** `null` when no compat tool applies (a Windows host, a Linux game). */
  compat: CompatToolsLoad | null;
};

/**
 * Reads the game's settings and, when `wantsCompat`, the compat tools with
 * the game's own tool preselected (the configured default when it has
 * none). Throws when the settings themselves cannot be read; a compat list
 * failure is reported in `compat` instead.
 */
export async function loadNativeSettings(romId: number, wantsCompat: boolean): Promise<NativeSettingsLoad> {
  const settings = await api.nativeGameSettings(romId);
  let compat: CompatToolsLoad | null = null;
  if (wantsCompat) {
    try {
      const dto = await api.listCompatTools();
      compat = { tools: dto.tools, selected: settings.compat_tool || dto.default_tool };
    } catch (err) {
      compat = { error: errorMessage(err) };
    }
  }
  return {
    settings,
    executable: settings.executable || settings.candidates[0] || '',
    parameters: settings.parameters,
    compat,
  };
}

/** Saves the form. Throws the backend's refusal for the form to show. */
export function saveNativeSettings(romId: number, executable: string, parameters: string, compatTool: string): Promise<void> {
  return api.setNativeGameSettings(romId, executable, parameters, compatTool);
}

export type ReloadAnswer = {
  /** The line to show until the next reload. */
  message: string;
  failed: boolean;
  /** The server answered (even with a refusal): the caller reloads the form. */
  answered: boolean;
};

/** "Reload launch info from server": re-reads the server's game.json and
 *  applies its launch section. Never throws. */
export async function reloadFromServer(romId: number): Promise<ReloadAnswer> {
  try {
    const answer = await api.reloadNativeLaunch(romId);
    return { message: answer.message, failed: answer.outcome === 'invalid', answered: true };
  } catch (err) {
    return { message: errorMessage(err), failed: true, answered: false };
  }
}
