// The Connect form's rules, kept out of the component so they can be tested
// without mounting it.

/**
 * Whether the form asks for a library folder, and what the field starts
 * with. A first run has no folder stored: the field is shown, pre-filled
 * with the suggested folder, and required — nothing can install without one.
 * When a folder is already stored (a rejected credential brought the form
 * back, or a Python import carried it over) the field is not shown at all;
 * Settings › Library is where it changes.
 */
export function libraryField(stored: string, suggested: string): { show: boolean; initial: string } {
  return stored.trim() !== '' ? { show: false, initial: '' } : { show: true, initial: suggested.trim() };
}

export type ConnectFields = {
  serverUrl: string;
  username: string;
  secret: string;
  useToken: boolean;
  /** `null` while the form does not ask for a folder. */
  libraryPath: string | null;
};

/** The Connect button is enabled only once every field it shows is filled.
 *  The username is asked for only in password mode. */
export function canConnect(f: ConnectFields): boolean {
  if (f.serverUrl.trim() === '' || f.secret === '') return false;
  if (!f.useToken && f.username.trim() === '') return false;
  if (f.libraryPath !== null && f.libraryPath.trim() === '') return false;
  return true;
}
