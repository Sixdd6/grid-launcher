// The welcome's library-folder effects (the rules are `welcome.ts`): what the
// field starts with, and storing a typed folder before Continue or Skip.
// Taken from the old Connect screen's `connectFlow.ts`.
import { api } from './api';
import { folderToSave, initialFolder } from './welcome';

export type LibraryFolderStart = {
  /** The stored `library_path`; `''` when none (or it could not be read). */
  stored: string;
  /** What the field starts with. */
  initial: string;
};

/** The stored folder and the field's starting value (the stored folder, else
 *  the suggested one). An unreadable config starts blank, with no suggestion. */
export async function loadLibraryFolder(): Promise<LibraryFolderStart> {
  try {
    const [stored, suggested] = await Promise.all([api.getLibraryPath(), api.suggestedLibraryPath()]);
    return { stored: stored.trim(), initial: initialFolder(stored, suggested) };
  } catch {
    return { stored: '', initial: '' };
  }
}

/** The line shown when the backend refuses a folder. */
export function refusedFolderLine(e: unknown): string {
  return `That folder can't be used: ${e}`;
}

/** Stores the typed folder when it differs from the stored one. Answers the
 *  refusal line, or `null` when stored (or nothing needed storing). */
export async function saveLibraryFolder(folder: string, stored: string): Promise<string | null> {
  const next = folderToSave(folder, stored);
  if (next === null) return null;
  try {
    await api.setLibraryPath(next);
    return null;
  } catch (e) {
    return refusedFolderLine(e);
  }
}
