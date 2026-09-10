import { readFileSync } from 'node:fs';
import {
  APP_START_TIMEOUT,
  configPath,
  FIXTURE_TOKEN,
  FIXTURE_USERNAME,
  mockUrl,
  TRANSITION_TIMEOUT,
  WRONG_TOKEN,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `connect`: one app instance against a fresh data directory.
 *
 * The tests are ordered on purpose. The app offers no sign-out control, so
 * every rejected-credential check has to happen before the successful
 * connect that moves the app to the library for good.
 */
describe('connect', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
  });

  it('rejects a wrong token and keeps the connect form', async () => {
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(WRONG_TOKEN);
    await $(testId('connect-submit')).click();

    const error = $(testId('connect-error'));
    await error.waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no error was shown for a token the server rejects',
    });
    await expect(error).toHaveText(expect.stringContaining('rejected the credentials'));
    await expect($(testId('connect-submit'))).toExist();
  });

  it('connects with the fixture token and shows the library', async () => {
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();

    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after a successful connect',
    });
    await expect($(testId('platform-btn-2'))).toExist();
    await expect($(testId('connect-submit'))).not.toExist();
  });

  it('keeps the token out of the DOM', async () => {
    const found = await browser.execute((token: string) => {
      const markup = document.documentElement.outerHTML;
      const values = Array.from(document.querySelectorAll('input')).map((el) => el.value);
      return markup.includes(token) || values.some((v) => v.includes(token));
    }, FIXTURE_TOKEN);
    expect(found).toBe(false);
  });

  it('writes server_url and username to config.toml but never the token', () => {
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`server_url = "${mockUrl()}"`);
    expect(text).toContain(`username = "${FIXTURE_USERNAME}"`);
    expect(text).not.toContain(FIXTURE_TOKEN);
    expect(text).not.toContain('token');
    expect(text).not.toContain('password');
  });

  it('re-connects from Settings › Connection with the same credentials', async () => {
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-connection')).click();
    await $(testId('settings-connection-url')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the connection settings page never rendered',
    });

    await $(testId('settings-connection-edit')).click();
    await $(testId('settings-connection-server-url')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the edit-connection form never opened',
    });
    await expect($(testId('settings-connection-server-url'))).toHaveValue(mockUrl());
    await $(testId('settings-connection-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('settings-connection-save')).click();

    await $(testId('settings-connection-edit-form')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the edit form stayed open after a successful reconnect',
    });
    // `toHaveText` with a matcher, not `toHaveTextContaining` (removed in
    // expect-webdriverio 6) — the same form the wrong-token case uses.
    await expect($(testId('settings-connection-status'))).toHaveText(
      expect.stringContaining('Connected'),
    );
    // Still no secret anywhere on disk.
    const text = readFileSync(configPath(), 'utf8');
    expect(text).toContain(`server_url = "${mockUrl()}"`);
    expect(text).not.toContain(FIXTURE_TOKEN);
  });

  it('offers Open Config Folder without navigating away', async () => {
    // The opener has nothing to open into under Xvfb, so this asserts the
    // control exists and is reachable — the command's own path rule is unit
    // tested (commands.rs `config_dir_tests`).
    await expect($(testId('settings-open-config-folder'))).toBeDisplayed();
    await expect($(testId('settings-open-config-folder'))).toHaveText('Open Config Folder');
  });
});
