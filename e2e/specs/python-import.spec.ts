import { readFileSync } from 'node:fs';
import { APP_START_TIMEOUT, configPath, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `python-import`: an empty data directory plus a Python-era
 * `config.json` fixture, pointed at by `GRID_LAUNCHER_PYTHON_CONFIG`
 * (written by e2e.sh with this attempt's mock URL inside it).
 *
 * This is the one stage that exercises the startup importer end to end: the
 * Rust unit tests stop at `ImportReport` and vitest starts at `pushToast`,
 * and the seam between them is exactly where the notice went missing.
 *
 * The fixture holds one emulator and two games and NO credential, so the
 * app comes up on the Connect form with the imported server and account
 * already filled in and one toast telling the user what is left to do.
 */
describe('python-import', () => {
  const IMPORTED_USERNAME = 'importer';
  const NOTICE =
    'Imported 1 emulator and 2 games from the previous version. ' +
    'Enter your RomM token to reconnect.';

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
  });

  it('shows the import notice on the connect screen', async () => {
    await $(testId('toast')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no toast appeared on the connect screen after a Python import',
    });
    await expect($(testId('toast'))).toHaveText(NOTICE);
  });

  it('prefills the connect form with the imported server url and username', async () => {
    await expect($(testId('connect-server-url'))).toHaveValue(mockUrl());
    // The username input is behind the "Use API token" checkbox, which is
    // on by default; clearing it reveals the field the import filled.
    await $(testId('connect-use-token')).click();
    await $(testId('connect-username')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the username field never appeared after turning token auth off',
    });
    await expect($(testId('connect-username'))).toHaveValue(IMPORTED_USERNAME);
  });

  it('wrote the imported settings to config.toml and no credential', () => {
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`server_url = "${mockUrl()}"`);
    expect(text).toContain(`username = "${IMPORTED_USERNAME}"`);
    expect(text).toContain('RetroArch');
    expect(text).not.toContain('token');
    expect(text).not.toContain('password');
  });
});
