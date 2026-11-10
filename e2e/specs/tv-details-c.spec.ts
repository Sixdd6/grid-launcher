import { mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, focused, key, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, openLibraryDetails, pageErrors, testId, text, typeKeys, waitGone, waitOskClosed, waitText } from '../helpers/tvDetails.js';

async function setOffline(offline: boolean): Promise<void> {
  const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ offline }),
  });
  if (!res.ok) throw new Error(`the mock refused /__e2e__/offline: ${res.status}`);
}

/**
 * Stage `tv-details`, spec c: starts with --tv on the saved session. The
 * Server platform search follows the Library's rule (a blank search is no
 * filter; B restores the search it had); Search everything says when
 * nothing could be searched and tries again; a launch with no emulator
 * offers "Set up <Platform>" on its card, which View opens.
 */
describe('tv-details (c): search rules, Search everything failure, Set up <Platform>', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
    await captureErrors();
    await waitTab('server');
    await connectPad(XBOX_PAD);
  });

  it('a blank platform search is no filter, and B restores the search it had (consultant ruling, item 385)', async () => {
    await browser.waitUntil(async () => (await focused()) !== null, { timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await waitFocused('r-discover');
    await moveTo('r-1', 'ArrowDown', 6);
    await waitText('tv-srv-count', /^5 games · \d installed$/);
    await key('y');
    await typeKeys('save');
    await key('Enter');
    await waitOskClosed();
    await waitText('tv-srv-search', 'Search: save');
    await waitText('tv-srv-count', /^3 of 5 games match · \d installed$/);
    await key('y');
    await browser.keys(['Backspace', 'Backspace', 'Backspace', 'Backspace']);
    await waitGone('tv-srv-search', 'an emptied search still showed its chip');
    await waitText('tv-srv-count', /^5 games · \d installed$/);
    expect(await $(testId('tv-srv-nomatch')).isExisting()).toBe(false);
    await key('Escape');
    await waitOskClosed();
    await waitText('tv-srv-search', 'Search: save');
  });

  it('Search everything with the server not answering shows the failure and Try again (consultant ruling)', async () => {
    await key('Escape');
    await moveTo('r-discover', 'ArrowUp', 6);
    await key('ArrowRight');
    await browser.waitUntil(async () => ((await focused()) ?? '').length > 0 && (await focused()) !== 'r-discover', { timeout: TRANSITION_TIMEOUT });
    await setOffline(true);
    try {
      await key('y');
      await typeKeys('save');
      await key('Enter');
      await $(testId('tv-search-failed')).waitForExist({ timeout: 30_000, timeoutMsg: 'the failure never showed' });
      expect(await text('tv-search-failed')).toContain("Can't reach your server");
      expect(await text('tv-search-failed')).toContain("isn't answering, so nothing could be searched.");
      await waitFocused('sr-retry');
    } finally {
      await setOffline(false);
    }
    await key('Enter');
    await waitText('tv-search-heading', '3 results for “save”');
    await key('Escape');
  });

  it('a launch with no emulator offers "Set up <Platform>"; View opens the system page (item 318)', async () => {
    await key('q');
    await waitTab('library');
    await waitFocused('r-all');
    await openLibraryDetails('No Emu Game');
    await waitFocused('db-play');
    await key('Enter');
    await waitToast(/^Couldn't launch No Emu Game/);
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-toast-action"]')).map((a) => a.textContent?.trim()))).includes(
          'Set up Nintendo 64',
        ),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the card never offered Set up Nintendo 64' },
    );
    await key('v');
    await $(testId('tv-system-stub')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'View never opened the system page' });
    await waitText('tv-crumb', /Nintendo 64/);
  });

  it('had no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
