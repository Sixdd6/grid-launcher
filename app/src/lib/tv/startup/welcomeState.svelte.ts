// What the welcome holds while it is on show (TVRommStorefront README § The
// welcome). It lives outside the screens because the folder browser, step 2
// and the RomM sign-in page are pages pushed over step 1, which remounts it
// when they close.
import { api } from '../../api';

export const welcome = $state<{
  /** The library folder chosen ('' until one is). */
  lib: string;
  /** The folder already stored ('' when none): Next doesn't store it again. */
  libStored: string;
  /** A Python import filled the welcome in. */
  imported: boolean;
  /** The import dialog was shown this run. */
  dialogShown: boolean;
  /** A refusal (the folder, finishing the welcome), or null. */
  error: string | null;
  busy: boolean;
  /** The stored values have been read. */
  loaded: boolean;
}>({ lib: '', libStored: '', imported: false, dialogShown: false, error: null, busy: false, loaded: false });

/** A fresh welcome: the stored folder (a Python import's), and whether an
 *  import happened. A fresh profile starts with no folder chosen. */
export async function resetWelcome(): Promise<void> {
  Object.assign(welcome, { lib: '', libStored: '', imported: false, dialogShown: false, error: null, busy: false, loaded: false });
  const [stored, report] = await Promise.all([api.getLibraryPath().catch(() => ''), api.pythonImportNotice().catch(() => null)]);
  welcome.libStored = stored.trim();
  if (welcome.lib === '') welcome.lib = stored.trim();
  welcome.imported = report !== null;
  welcome.loaded = true;
}
