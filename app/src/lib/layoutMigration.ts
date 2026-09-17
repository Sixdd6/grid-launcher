// The one line the user sees when the startup library-layout migration did
// not finish. Pure, so the wording is pinned by vitest rather than by
// reading a toast.

/**
 * The failure toast. `message` comes from grid-core and carries paths only —
 * never a credential — so it is safe to show verbatim.
 */
export function migrationToastText(message: string): string {
  return `Library reorganization did not finish: ${message}. It will retry on next launch.`;
}
