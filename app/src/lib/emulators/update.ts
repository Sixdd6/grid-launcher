// The emulator install/update completion toast, verbatim from the
// reference's post-install toast (`install_mixin.py:1690-1716`). Pure, so
// vitest owns the wording.

/** The completion toast: an update over an existing entry reads differently. */
export function installedToastText(name: string, fresh: boolean): string {
  return fresh
    ? `Installed emulator '${name}' from source.`
    : `Updated emulator '${name}' from source.`;
}
