// The Connect screen's effects, for the desktop form and the TV startup
// screen alike: the library-folder field's starting value, storing a typed
// folder, "Use without a server", and the connect itself. The rules (which
// fields a submit needs) are `connectForm.ts`.
//
// The secret is never stored or kept here: `startConnect` hands it straight
// to `connect` and returns at once, so the caller can blank its own field in
// the same tick.
import { api } from './api';
import { libraryField } from './connectForm';
import { connect, useOffline } from './stores/session.svelte';

/**
 * The library-folder field's starting value: the suggested folder when none
 * is stored, `null` when one is stored (the field is not shown), and `''`
 * when the stored folder cannot be read (ask, with no suggestion).
 */
export async function loadLibraryField(): Promise<string | null> {
  try {
    const [stored, suggested] = await Promise.all([api.getLibraryPath(), api.suggestedLibraryPath()]);
    const field = libraryField(stored, suggested);
    return field.show ? field.initial : null;
  } catch {
    return '';
  }
}

/** Stores a typed library folder. Answers the line to show when the backend
 *  refuses it, or `null` when stored or when none was asked for. */
export async function saveConnectLibraryPath(libraryPath: string | null): Promise<string | null> {
  if (libraryPath === null) return null;
  try {
    await api.setLibraryPath(libraryPath.trim());
    return null;
  } catch (e) {
    return `That folder can't be used: ${e}`;
  }
}

/**
 * "Use without a server": only the library folder is needed. It is stored
 * first, then the backend turns offline mode on and the shell opens on
 * Library. Answers the refusal line for the folder, or `null`.
 */
export async function useWithoutServer(libraryPath: string | null): Promise<string | null> {
  const refused = await saveConnectLibraryPath(libraryPath);
  if (refused !== null) return refused;
  await useOffline();
  return null;
}

/**
 * Starts the connect and returns at once (`session.busy` is set in the same
 * tick). Token auth identifies the account by itself; the username input
 * only exists for Basic mode, so a stale one is never sent with a token.
 */
export function startConnect(serverUrl: string, username: string, secret: string, useToken: boolean): void {
  void connect(serverUrl, useToken ? '' : username, secret, useToken);
}
