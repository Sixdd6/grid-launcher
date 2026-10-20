// The `launch_game` command's error shape (app/src-tauri/src/commands.rs's
// `LaunchCommandError`): `{ code, message }`, where `code` is
// `'umu_missing'` when a Proton launch found no umu-run and `''` otherwise.
// `api.launchGame` turns every rejection into a `LaunchFailure`, so the
// three launch sites (Library, Server, Details) keep reading `err.message`
// and can also ask `isUmuMissing(err)` to offer the umu-launcher install.

/** `LaunchCommandError.code` for a Proton launch with no umu-run. */
export const UMU_MISSING = 'umu_missing';

export class LaunchFailure extends Error {
  readonly code: string;

  constructor(message: string, code = '') {
    super(message);
    this.name = 'LaunchFailure';
    this.code = code;
  }
}

/**
 * Normalizes a `launch_game` rejection: the structured `{ code, message }`
 * object, a plain string (an IPC-level failure), or an `Error`.
 */
export function toLaunchFailure(err: unknown): LaunchFailure {
  if (err instanceof LaunchFailure) return err;
  if (err instanceof Error) return new LaunchFailure(err.message);
  if (typeof err === 'object' && err !== null && typeof (err as { message?: unknown }).message === 'string') {
    const { message, code } = err as { message: string; code?: unknown };
    return new LaunchFailure(message, typeof code === 'string' ? code : '');
  }
  return new LaunchFailure(String(err));
}

/** Whether `err` is a launch that failed for want of umu-run. */
export function isUmuMissing(err: unknown): boolean {
  return toLaunchFailure(err).code === UMU_MISSING;
}
