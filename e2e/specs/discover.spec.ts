import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';
import { openDiscover, openPlatform } from '../helpers/server.js';

const testId = (id: string) => `[data-testid="${id}"]`;

const SNES = 'Super Nintendo Entertainment System';

/** With E2E_SCREENSHOTS set, saves the window into ctr-out/discover/. */
async function screenshot(name: string): Promise<void> {
  if (!process.env.E2E_SCREENSHOTS) return;
  const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'discover');
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(path.join(dir, `${name}.png`));
}

/**
 * Stage `discover`: Server › Discover against `e2e/fixtures-discover`, whose
 * dates are relative to the day the suite runs (`e2e_released_days_ago`,
 * `e2e_added_days_ago` in roms.json):
 *
 *   SNES  501 Starlit Run    released 10 days ago,  added 1 day ago
 *         502 Neon Drift     released 40 days ago,  added 2 days ago
 *         503 Old Classic    released years ago,    added 3 days ago
 *         504 Upcoming Thing released in 30 days,   added 4 days ago
 *         505 Mid Year       released 100 days ago, added 300 days ago
 *   Arcade 601 Arcade Fresh  released 5 days ago,   added 5 days ago
 *         602 Arcade Logo    released years ago,    added 6 days ago (server logo)
 *   N64    no games, so never a Recently added filter
 *
 * The run starts with no default emulator and no Linux platform, so New
 * releases is empty; giving SNES a default fills it with 501, 502, 505.
 */
describe('discover', () => {
  /** The ids of the elements whose test id starts with `prefix`, in order. */
  async function idsOf(prefix: string): Promise<number[]> {
    return browser.execute(
      (p: string) =>
        Array.from(document.querySelectorAll(`[data-testid^="${p}"]`)).map((el) =>
          Number((el as HTMLElement).dataset.testid!.slice(p.length)),
        ),
      prefix,
    );
  }

  /** The platform filter pills' test id suffixes, in order (`all` first). */
  async function pills(): Promise<string[]> {
    return browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid^="discover-filter-"]')).map((el) =>
        (el as HTMLElement).dataset.testid!.replace('discover-filter-', ''),
      ),
    );
  }

  /** The pills pressed (`aria-pressed="true"`). */
  async function pressed(): Promise<string[]> {
    return browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid^="discover-filter-"][aria-pressed="true"]')).map((el) =>
        (el as HTMLElement).dataset.testid!.replace('discover-filter-', ''),
      ),
    );
  }

  /** Waits until Recently added lists exactly `ids`, in order. */
  async function waitRows(ids: number[], why: string) {
    await browser.waitUntil(async () => JSON.stringify(await idsOf('discover-recent-row-')) === JSON.stringify(ids), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `${why}: Recently added lists ${JSON.stringify(await idsOf('discover-recent-row-'))}, not ${JSON.stringify(ids)}`,
    });
  }

  async function centreCover(): Promise<number | null> {
    return browser.execute(() => {
      const el = document.querySelector('[data-testid^="discover-cover-"][data-center="true"]') as HTMLElement | null;
      return el ? Number(el.dataset.testid!.replace('discover-cover-', '')) : null;
    });
  }

  async function waitCentre(id: number) {
    await browser.waitUntil(async () => (await centreCover()) === id, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the coverflow never centred rom ${id}`,
    });
  }

  /** Gives SNES a default emulator in config.toml, as Settings › Systems would. */
  function setSnesDefault() {
    const line = `"${SNES}" = "snes9x"`;
    let config = readFileSync(configPath(), 'utf-8');
    if (/^\[default_emulators\]\s*$/m.test(config)) {
      config = config.replace(/^\[default_emulators\]\s*$/m, `[default_emulators]\n${line}`);
    } else {
      config = `${config.trimEnd()}\n\n[default_emulators]\n${line}\n`;
    }
    writeFileSync(configPath(), config);
  }

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('server-rail-discover')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Server rail never appeared after connecting',
    });
  });

  it('opens Server on Discover, the first rail row, with a divider under it', async () => {
    await $(testId('discover')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // Open GRID on is Automatic and the server answers: GRID opens here.
    await expect($(testId('nav-server'))).toHaveAttribute('aria-current', 'page');
    await expect($(testId('server-rail-discover'))).toHaveAttribute('aria-current', 'page');
    const rail = await browser.execute(() => {
      const nav = document.querySelector('[data-testid="server-rail"]')!;
      const first = nav.querySelector('button');
      return {
        first: first?.dataset.testid ?? '',
        dividerNext: first?.nextElementSibling?.tagName ?? '',
      };
    });
    expect(rail).toEqual({ first: 'server-rail-discover', dividerNext: 'HR' });
    // The folding header and the grid belong to a platform, not to Discover.
    await expect($(testId('server-platform-header'))).not.toExist();
    await expect($(testId('server-search'))).not.toExist();
  });

  it('does nothing on Ctrl+F', async () => {
    await browser.keys(['Control', 'f']);
    const tag = await browser.execute(() => document.activeElement?.tagName ?? '');
    expect(tag).not.toBe('INPUT');
  });

  it('says New releases is empty and points to Settings › Systems', async () => {
    await $(testId('discover-new-empty')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('discover-open-systems')).click();
    await $(testId('systems-list')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the empty New releases link never opened Settings › Systems',
    });
    await $(testId('nav-server')).click();
    await $(testId('discover')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('lists the newest games on the server in Recently added', async () => {
    await $(testId('discover-recent-row-501')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await idsOf('discover-recent-row-')).toEqual([501, 502, 503, 504, 601, 602, 505]);
    await expect($(testId('discover-recent-row-501'))).toHaveAttribute('data-active', 'true');
    await expect($(testId('discover-recent-meta'))).toHaveText(
      expect.stringMatching(/^Super Nintendo Entertainment System · Added (yesterday|2 days ago)$/),
    );
  });

  it('filters Recently added by the platforms with recent additions', async () => {
    // SNES's newest addition is newer than Arcade's; N64 has no games.
    await $(testId('discover-filters')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await pills()).toEqual(['all', '1', '2']);
    expect(await pressed()).toEqual(['all']);
    await expect($(testId('discover-filter-1'))).toHaveText(SNES);

    await $(testId('discover-filter-2')).click();
    await waitRows([601, 602], 'the Arcade filter');
    expect(await pressed()).toEqual(['2']);
    // A new filter starts the rotation at its first row.
    await expect($(testId('discover-recent-row-601'))).toHaveAttribute('data-active', 'true');

    // The pressed pill again clears the filter.
    await $(testId('discover-filter-2')).click();
    await waitRows([501, 502, 503, 504, 601, 602, 505], 'clearing with the pressed pill');
    expect(await pressed()).toEqual(['all']);

    await $(testId('discover-filter-1')).click();
    await waitRows([501, 502, 503, 504, 505], 'the SNES filter');
    expect(await pressed()).toEqual(['1']);
    await screenshot('0-filtered');
    await $(testId('discover-filter-all')).click();
    await waitRows([501, 502, 503, 504, 601, 602, 505], 'clearing with All');
    expect(await pressed()).toEqual(['all']);
  });

  it('fills New releases from the preferred platforms, newest release first', async () => {
    setSnesDefault();
    // Discover reads the defaults each time it comes to the front.
    await openPlatform(2);
    await openDiscover();
    await $(testId('discover-coverflow')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'New releases never showed a coverflow once SNES had a default',
    });
    // 504 is upcoming, 503 too old, and Arcade has no default emulator.
    expect(await idsOf('discover-cover-')).toEqual([501, 502, 505]);
    expect(await centreCover()).toBe(501);
    await expect($(testId('discover-cf-title'))).toHaveText('Starlit Run');
    await expect($(testId('discover-cf-action'))).toHaveText('Install');
    await expect($(testId('discover-cf-caption'))).toHaveText(expect.stringContaining('Released '));
    await screenshot('1-discover');
  });

  it('leaves the coverflow games out of Recently added', async () => {
    await browser.waitUntil(
      async () => JSON.stringify(await idsOf('discover-recent-row-')) === JSON.stringify([503, 504, 601, 602]),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `Recently added still lists coverflow games: ${JSON.stringify(await idsOf('discover-recent-row-'))}`,
      },
    );
    // A platform filter leaves them out too.
    expect(await pills()).toEqual(['all', '1', '2']);
    await $(testId('discover-filter-1')).click();
    await waitRows([503, 504], 'the SNES filter beside the coverflow');
    await $(testId('discover-filter-1')).click();
    await waitRows([503, 504, 601, 602], 'clearing the SNES filter');
  });

  it('moves the coverflow with ← / →, a side-cover click, and opens Details with Enter', async () => {
    await browser.execute(() => (document.querySelector('[data-testid="discover-coverflow"]') as HTMLElement).focus());
    await browser.keys(['ArrowRight']);
    await waitCentre(502);
    await expect($(testId('discover-cf-title'))).toHaveText('Neon Drift');
    await browser.keys(['ArrowRight']);
    await waitCentre(505);
    // The last cover is the end: the coverflow does not wrap.
    await browser.keys(['ArrowRight']);
    await waitCentre(505);
    await expect($(testId('discover-cf-next'))).toBeDisabled();
    await browser.keys(['ArrowLeft']);
    await waitCentre(502);

    // A side cover moves the coverflow to it rather than opening it.
    await $(testId('discover-cover-501')).click();
    await waitCentre(501);
    await expect($(testId('details-panel'))).not.toExist();

    await browser.execute(() => (document.querySelector('[data-testid="discover-coverflow"]') as HTMLElement).focus());
    await browser.keys(['Enter']);
    await $(testId('details-panel')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Enter on the coverflow never opened Details',
    });
    await expect($(testId('details-panel'))).toHaveAttribute('aria-label', 'Starlit Run');
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('jumps Recently added to a clicked row and draws each game as a logo', async () => {
    const frontLogo = () => $('.ra-layer.on [data-testid="discover-logo"]');
    // Pointing at the section pauses the 10 s rotation.
    await $(testId('discover-recent-list')).moveTo();
    if ((await $(testId('discover-recent-row-601')).getAttribute('data-active')) !== 'true') {
      await $(testId('discover-recent-row-601')).click();
    }
    await expect($(testId('discover-recent-row-601'))).toHaveAttribute('data-active', 'true');
    // 601 has no server logo: its title is drawn as a generated logo.
    await browser.waitUntil(async () => (await frontLogo().getAttribute('aria-label')) === 'Arcade Fresh', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the screenshot never changed to the clicked row',
    });
    await expect(frontLogo()).toHaveAttribute('data-generated', 'true');

    // 602 has a ScreenScraper clear logo on the server.
    await $(testId('discover-recent-row-602')).click();
    await expect($(testId('discover-recent-row-602'))).toHaveAttribute('data-active', 'true');
    await browser.waitUntil(async () => (await frontLogo().getAttribute('aria-label')) === 'Arcade Logo', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the screenshot never changed to the clicked row',
    });
    await expect(frontLogo()).toHaveAttribute('data-generated', 'false');
    await screenshot('2-server-logo');

    // A click on the highlighted row opens its Details.
    await $(testId('discover-recent-row-602')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('details-panel'))).toHaveAttribute('aria-label', 'Arcade Logo');
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });
});
