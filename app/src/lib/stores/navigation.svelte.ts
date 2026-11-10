// Mode-agnostic navigation intents. Code that is not a view — a toast's
// action, the setup card's Open Downloads, a lifetime listener — asks for a
// place with `navigation.go(...)` instead of holding a reference to the
// desktop Shell, so the same caller works whichever shell is mounted. The
// mounted shell registers the handler that carries the intent out
// (`registerNavigation`) and drops it when it unmounts.
import type { DetailsSubject } from '../details/subject';
import type { SettingsPage } from '../settings/pages';
import type { View } from '../shell';

export type NavigationHandler = {
  /** Shows one of the four views. */
  go: (view: View) => void;
  /** Shows a Settings page; `system` opens that system's row on Systems. */
  goSettings: (page: SettingsPage, opts?: { system?: string }) => void;
  /** Opens Details for a game. Optional: the desktop shell has no
   *  shell-level Details (Server and Library each own theirs). */
  openDetails?: (subject: DetailsSubject) => void;
};

let handler: NavigationHandler | null = null;

/**
 * Makes `next` the handler every intent goes to, and answers the function
 * that removes it again. Removing a handler that has already been replaced
 * leaves the newer one in place, so an unmount racing a mount is harmless.
 */
export function registerNavigation(next: NavigationHandler): () => void {
  handler = next;
  return () => {
    if (handler === next) handler = null;
  };
}

/** The intents. Each is a no-op while no shell is mounted (Connect, setup). */
export const navigation = {
  go(view: View): void {
    handler?.go(view);
  },
  goSettings(page: SettingsPage, opts: { system?: string } = {}): void {
    handler?.goSettings(page, opts);
  },
  openDetails(subject: DetailsSubject): void {
    handler?.openDetails?.(subject);
  },
};
