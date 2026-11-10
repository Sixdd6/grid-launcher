// Reading an install refusal (the backend's error text) to decide what the
// screen offers beside the error card. Pure, so it is tested without a
// component.

/**
 * The refusal when no library folder is set ("Set a library folder in
 * Settings › Library before installing games"): the screen then offers the
 * library-folder banner. Other refusals that name the library folder (it is
 * too full) do not.
 */
export function needsLibraryFolder(message: string): boolean {
  return message.trim().startsWith('Set a library folder');
}
