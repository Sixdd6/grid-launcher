/**
 * Per-stage environment, handed down by `rewrite/scripts/e2e.sh`.
 *
 * The embedded WebDriver provider keeps ONE app instance alive for a whole
 * `wdio run`, so the runner starts one `wdio run` per stage and passes the
 * stage's values in the process environment. Specs read them from here; the
 * wdio config forwards the same values into the app process through
 * `wdio:tauriServiceOptions.env`.
 */

function required(name: string): string {
  const value = process.env[name];
  if (!value || value.trim() === '') {
    throw new Error(
      `${name} is not set. Run the E2E suite through rewrite/scripts/e2e.sh, ` +
        'which builds the app and exports the per-stage environment.',
    );
  }
  return value;
}

/** Base URL of this stage's mock RomM server, e.g. `http://127.0.0.1:41235`. */
export const mockUrl = (): string => required('E2E_MOCK_URL');

/** This stage's `GRID_LAUNCHER_DATA_DIR` (config.toml, the db and covers/ live here). */
export const dataDir = (): string => required('E2E_DATA_DIR');

/** Path to this stage's `config.toml`. */
export const configPath = (): string => `${dataDir()}/config.toml`;

/**
 * The token the mock server accepts. It is a fixture string, not a credential:
 * `rewrite/scripts/check_secret_hygiene.sh` allowlists this exact literal.
 */
export const FIXTURE_TOKEN = 'FAKE-E2E-TOKEN-not-real';

/** A token the mock server rejects with 401. */
export const WRONG_TOKEN = 'FAKE-E2E-TOKEN-wrong';

/** The account name `GET /api/users/me` reports for the fixture token. */
export const FIXTURE_USERNAME = 'e2euser';

/** Waits: app start is slow (webview + embedded server); UI transitions are not. */
export const APP_START_TIMEOUT = 15_000;
export const TRANSITION_TIMEOUT = 5_000;
/** An install of one of the small (non-throttled) content fixtures. */
export const INSTALL_TIMEOUT = 10_000;
/**
 * A download against the `downloads` stage group's throttled mock (100ms
 * per ~20KB chunk against the ~300KB "Big Arcade Game" fixture — see
 * mock-romm/server.mjs and rewrite/scripts/e2e.sh's mock_args_for_group).
 * Generous on top of the ~1.5-2s the throttle itself adds, to leave room for
 * wdio/WebDriver round-trip overhead without becoming a flaky spec.
 */
export const THROTTLED_DOWNLOAD_TIMEOUT = 20_000;
