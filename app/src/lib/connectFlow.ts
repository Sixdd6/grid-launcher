// Legacy: TV mode's Connect screen (`tv/startup/TvConnect.svelte`) until
// Step 6 replaces it with the TV welcome; Step 9 deletes this file. The
// desktop welcome's folder helpers are `welcomeFlow.ts`.
import { api } from './api';
import { useOffline } from './stores/session.svelte';
import { refusedFolderLine } from './welcomeFlow';

/** Stores a typed library folder. Answers the line to show when the backend
 *  refuses it, or `null` when stored or when none was asked for. */
export async function saveConnectLibraryPath(libraryPath: string | null): Promise<string | null> {
  if (libraryPath === null) return null;
  try {
    await api.setLibraryPath(libraryPath.trim());
    return null;
  } catch (e) {
    return refusedFolderLine(e);
  }
}

/** "Use without a server": the folder first, then offline mode. Answers the
 *  refusal line for the folder, or `null`. */
export async function useWithoutServer(libraryPath: string | null): Promise<string | null> {
  const refused = await saveConnectLibraryPath(libraryPath);
  if (refused !== null) return refused;
  await useOffline();
  return null;
}
