import { existsSync, readFileSync } from 'node:fs';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

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
 * The fixture holds one emulator and five games and NO credential, so the
 * app comes up on the Connect form with the imported server and account
 * already filled in and one toast telling the user what is left to do.
 *
 * Three of the games carry no rom id (Q4). After the spec connects:
 * "Super Mario World" has exactly one match on the mock (rom 101), so the
 * relink pass links it; "Mystery Cart" has none, so Details offers "Link to
 * server game", and the spec links it to rom 103 by hand; "Old Demo" is
 * removed from the library and its folder stays on disk.
 */
describe('python-import', () => {
  const IMPORTED_USERNAME = 'importer';
  const NOTICE =
    'Imported 1 emulator and 5 games from the previous version. ' +
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

  /** The Library card whose text holds `title`, by its test id. Unlinked
   *  rows have an index-based id, so the spec finds them by title. */
  async function cardIdFor(title: string): Promise<string | null> {
    return browser.execute((wanted: string) => {
      const cards = Array.from(document.querySelectorAll('[data-testid^="library-card-"]'));
      const card = cards.find((el) => (el.textContent ?? '').includes(wanted));
      return card ? card.getAttribute('data-testid') : null;
    }, title);
  }

  async function openCard(title: string) {
    await browser.waitUntil(async () => (await cardIdFor(title)) !== null, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `no Library card for "${title}"`,
    });
    const id = await cardIdFor(title);
    await $(testId(id!)).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  }

  async function closeDetails() {
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  }

  it('connects, and the relink pass links the row with one server match', async () => {
    // The previous test turned token auth off to reveal the username.
    await $(testId('connect-use-token')).click();
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('connect-submit')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });

    await $(testId('nav-library')).click();
    // Rom 101 is "Super Mario World" on the mock: the imported row gets its
    // id, so its card carries it.
    await $(testId('library-card-101')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the imported "Super Mario World" row was never linked to rom 101',
    });
    await expect($(testId('library-card-101'))).toHaveText(expect.stringContaining('Super Mario World'));
  });

  it('offers Link to server game for a row with no match, and links it by hand', async () => {
    await openCard('Mystery Cart');
    await expect($(testId('details-play'))).not.toExist();
    await expect($(testId('details-remove-from-library'))).toExist();
    await $(testId('details-link-server')).click();

    await $(testId('link-picker')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('link-picker-item')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the picker never listed the SNES server games',
    });
    // Rom 101 is already held by "Super Mario World", so it is not offered.
    await expect($('[data-testid="link-picker-item"][data-rom-id="101"]')).not.toExist();
    await $(testId('link-picker-search')).setValue('secret');
    await browser.waitUntil(async () => (await $$(testId('link-picker-item')).length) === 1, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the search did not narrow the picker to "Secret of Mana Disc Set"',
    });
    await $('[data-testid="link-picker-item"][data-rom-id="103"]').click();

    await $(testId('link-picker')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    // Details re-opens on the linked game: the server-backed actions show.
    await $(testId('details-play')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Details did not switch to the linked game',
    });
    await closeDetails();
    await expect($(testId('library-card-103'))).toHaveText(expect.stringContaining('Mystery Cart'));
  });

  it('removes a row from the library and keeps its folder on disk', async () => {
    const folder = `${dataDir()}/kept/Old Demo`;
    await openCard('Old Demo');
    await $(testId('details-remove-from-library')).click();

    await $(testId('remove-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('remove-confirm'))).toHaveText(expect.stringContaining(folder));
    await $(testId('remove-confirm-yes')).click();

    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await browser.waitUntil(async () => (await cardIdFor('Old Demo')) === null, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the "Old Demo" card is still in the Library',
    });
    expect(existsSync(`${folder}/demo.sfc`)).toBe(true);
  });
});
