import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, invoke, key, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { captureErrors, moveTo, pageErrors, testId, typeKeys, waitOskClosed, waitText } from '../helpers/tvDetails.js';

/** Two installs from the mock forge, each held back a moment. */
const SETUP_TIMEOUT = 60_000;

type Card = { title: string; segs: string[]; step: string; countdown: boolean; pinned: boolean };

/** The setup card (pinned until it closes itself), or null. */
async function setupCard(): Promise<Card | null> {
  return (await browser.execute(() => {
    const c = Array.from(document.querySelectorAll('[data-testid="tv-toast"]')).find((t) => t.querySelector('.segs'));
    if (!c) return null;
    const lines = Array.from(c.querySelectorAll(':scope > .ts')).map((e) => (e.textContent ?? '').trim());
    return {
      title: (c.querySelector('[data-testid="tv-toast-title"]')?.textContent ?? '').trim(),
      segs: Array.from(c.querySelectorAll('.segs i')).map((i) => i.className),
      step: lines[lines.length - 1] ?? '',
      countdown: !!c.querySelector('.tbar'),
      pinned: c.hasAttribute('data-pinned'),
    };
  })) as Card | null;
}

/**
 * Stage `tv-startup`, spec d (TV mode plan P12): a fresh profile again.
 * Connect (pasted server, typed token, pasted library), then Choose your
 * systems: tick Dreamcast and PlayStation 2, Install 2 emulators. GRID
 * opens on Server › Discover with the setup card pinned at the top: one
 * segment per install, a step line, no countdown while anything installs;
 * once both are done, "2 installs done", the ready line, and it closes
 * after 10 seconds.
 */
describe('tv-startup (d): Install N emulators and the setup card', () => {
  before(async () => {
    await $(testId('tv-connect')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'TV mode never showed Connect' });
    await captureErrors();
    await connectPad(XBOX_PAD);
    await invoke('e2e_clipboard_write', { text: mockUrl() });
    await waitFocused('cf-url');
    await key('v');
    await waitText('tv-cf-url-value', mockUrl());
    await moveTo('cf-token', 'ArrowDown');
    await key('Enter');
    await typeKeys(FIXTURE_TOKEN);
    await key('Enter');
    await waitOskClosed();
    await moveTo('cf-lib', 'ArrowDown');
    await invoke('e2e_clipboard_write', { text: path.join(dataDir(), 'library') });
    await key('v');
    await waitText('tv-cf-lib-value', path.join(dataDir(), 'library'));
    await $(testId('tv-connect-go')).click();
    await $(testId('tv-choose')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT * 2, timeoutMsg: 'Choose your systems never opened' });
    await $(testId('tv-ch-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });

  it('Install 2 emulators opens Server › Discover with the setup card pinned, one segment each (items 151, 152, 154, 155, 407)', async () => {
    await waitFocused('ch-dc');
    await key('Enter');
    await key('ArrowDown');
    await waitFocused('ch-ps2');
    await key('Enter');
    await waitText('tv-choose-install', 'Install 2 emulators');
    await moveTo('ch-windows', 'ArrowDown');
    await key('ArrowDown');
    await waitFocused('ch-go');
    await key('Enter');
    await waitTab('server');
    let card: Card | null = null;
    await browser.waitUntil(async () => (card = await setupCard()) !== null, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the setup card never showed' });
    expect(card!.title).toMatch(/^(Setting up 2 installs · [01] done|2 installs done)$/);
    expect(card!.segs).toHaveLength(2);
    // No countdown while anything still installs.
    if (card!.title !== '2 installs done') expect(card!.countdown).toBe(false);
    // The step line names the install in progress (the mock forge is quick,
    // so the card may already be done; vitest covers every step's words).
    await browser.waitUntil(
      async () => {
        const c = await setupCard();
        return c === null || c.title === '2 installs done' || /^(PCSX2|Redream) · (downloading \d+%|installing…)$/.test(c.step);
      },
      { timeout: SETUP_TIMEOUT, interval: 50, timeoutMsg: `the setup card never named a step (${JSON.stringify(await setupCard())})` },
    );
    // Pinned to the top of the stack while it installs.
    const pinnedOnTop = (await browser.execute(() => {
      const cards = Array.from(document.querySelectorAll('[data-testid="tv-toast"]'));
      const pinned = cards.findIndex((c) => c.hasAttribute('data-pinned'));
      return pinned <= 0;
    })) as boolean;
    expect(pinnedOnTop).toBe(true);
  });

  it('once both are done: "2 installs done", the ready line, a countdown, then it closes (items 153, 156)', async () => {
    await browser.waitUntil(async () => (await setupCard())?.title === '2 installs done', {
      timeout: SETUP_TIMEOUT,
      timeoutMsg: `the installs never finished (${JSON.stringify(await setupCard())}; rows ${JSON.stringify(await invoke('list_downloads').catch((e) => String(e)))}; errors ${JSON.stringify(await pageErrors())})`,
    });
    const done = (await setupCard())!;
    expect(done.segs.every((c) => c === 'done')).toBe(true);
    expect(done.countdown).toBe(true);
    // Still pinned through its countdown (consultant, after P12).
    expect(done.pinned).toBe(true);
    await browser.waitUntil(async () => /^\d+ systems? (is|are) ready to play\.$/.test((await setupCard())?.step ?? ''), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the ready line never showed (${JSON.stringify(await setupCard())})`,
    });
    await browser.waitUntil(async () => (await setupCard()) === null, { timeout: 15_000, timeoutMsg: 'the setup card never closed' });
  });

  it('raised no page errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
