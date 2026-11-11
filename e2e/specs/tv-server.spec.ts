// e2e-app-args:
import { APP_START_TIMEOUT, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { assertNoClipping, connectPad, focused, hints, key, press, waitFocused, waitTab, waitToast, XBOX_PAD } from '../helpers/tv.js';

const testId = (id: string) => `[data-testid="${id}"]`;
const SNES = 'Super Nintendo Entertainment System';

async function text(id: string): Promise<string> {
  return ((await browser.execute((s: string) => document.querySelector(s)?.textContent ?? '', testId(id))) as string).trim();
}

async function waitText(id: string, want: string | RegExp): Promise<void> {
  let last = '';
  await browser.waitUntil(
    async () => {
      last = await text(id);
      return typeof want === 'string' ? last === want : want.test(last);
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${id} never read ${String(want)}` },
  ).catch((e) => {
    throw new Error(`${(e as Error).message} (it reads "${last}")`);
  });
}

/** Types on the on-screen keyboard with the physical keyboard. */
async function typeKeys(s: string): Promise<void> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  for (const ch of s) await browser.keys(ch);
}

async function setOffline(offline: boolean): Promise<void> {
  const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ offline }),
  });
  if (!res.ok) throw new Error(`the mock refused /__e2e__/offline: ${res.status}`);
}

async function waitDiscoverFocus(): Promise<string> {
  let k: string | null = null;
  await browser.waitUntil(
    async () => {
      k = await focused();
      return k !== null && (await browser.execute((key: string) => !!document.querySelector(`#disc [data-k="${key}"]`), k));
    },
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'focus never went into Discover' },
  ).catch((e) => {
    throw new Error(`${(e as Error).message} (focused: ${k})`);
  });
  return k!;
}

/**
 * Stage `tv-server` (docs/tv-mode/plan.md P7), on `e2e/fixtures-discover`
 * with SNES's default emulator seeded: New releases is 501, 502, 505;
 * Recently added (less those) 503, 504, 601, 602. Connects on the desktop,
 * then enters TV mode with the TV button on Server › Discover.
 */
describe('tv-server: the Server tab', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the connect form never appeared' });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('discover-coverflow')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Discover never loaded on the desktop' });
    // Every page error from here on, for the last test.
    await browser.execute(() => {
      const w = window as unknown as { __errs: string[] };
      w.__errs = [];
      addEventListener('error', (e) => w.__errs.push(`error: ${e.message} ${e.error?.stack ?? ''}`));
      addEventListener('unhandledrejection', (e) => w.__errs.push(`rejection: ${String(e.reason)} ${e.reason?.stack ?? ''}`));
      const ce = console.error.bind(console);
      console.error = (...a: unknown[]) => {
        w.__errs.push(`console: ${a.map((x) => (x instanceof Error ? `${x.message} ${x.stack}` : String(x))).join(' ')}`);
        ce(...a);
      };
    });
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('server');
    await connectPad(XBOX_PAD);
  });

  it("arrives on Discover's first cover, with the hero and the rows in order (items 104, 157–164, 417)", async () => {
    await waitFocused('n501', 'arriving on Server did not focus the first Discover cover');
    await waitText('tv-hero-title', 'Starlit Run');
    await waitText('tv-hero-line', /^Released \d{1,2} [A-Z][a-z]{2} \d{4} · Not installed$/);
    await waitText('tv-hero-summary', 'A Discover fixture game.');
    expect(await text('tv-hero-platform')).toBe(SNES);
    const shelves = await browser.execute(() =>
      Array.from(document.querySelectorAll('#disc [data-shelf]')).map((s) => ({
        title: s.querySelector('h2')?.childNodes[0]?.textContent ?? '',
        extra: s.querySelector('h2 span')?.textContent ?? null,
        covers: Array.from(s.querySelectorAll('[data-k]')).map((c) => c.getAttribute('data-k')),
        empty: s.querySelector('.empty')?.textContent ?? null,
      })),
    );
    expect(shelves).toEqual([
      { title: 'Continue playing', extra: null, covers: [], empty: 'Nothing here yet.' },
      { title: 'New releases', extra: 'the last six months, on your systems', covers: ['n501', 'n502', 'n505'], empty: null },
      { title: 'Recently added', extra: 'to your server', covers: ['r503', 'r504', 'r601', 'r602'], empty: null },
      { title: 'Your library', extra: '0 games', covers: [], empty: 'Nothing here yet.' },
    ]);
    // Discover covers carry platform tags (item 171).
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-cover-r601"] .tag.plat')?.textContent)).toBe('Arcade');
    const rail = await browser.execute(() => {
      const nav = document.querySelector('[data-testid="tv-server"] [data-rail]')!;
      return Array.from(nav.children).map((c) => (c.tagName === 'BUTTON' ? c.getAttribute('data-k') : c.tagName === 'H3' ? c.textContent : c.tagName));
    });
    // The backend lists platforms by name and leaves out empty ones (N64).
    expect(rail).toEqual(['r-discover', 'HR', 'PLATFORMS', 'r-2', 'r-1']);
    expect(await text('tv-srv-rail-1')).toBe(`${SNES}5`);
    expect(await hints()).toEqual(['A Details', 'X Install', 'Y Search', 'MENU Menu']);
    await assertNoClipping();
  });

  it('Up from the top row and Down from the bottom row stay put; a shelf title scrolls to the top (items 108–109, 174)', async () => {
    const heroHeight = await browser.execute(() => document.querySelector('[data-testid="tv-hero"]')!.getBoundingClientRect().height);
    await key('ArrowUp');
    await browser.pause(300);
    expect(await focused()).toBe('n501');
    await key('ArrowDown');
    await waitFocused('r503');
    await waitText('tv-hero-title', 'Old Classic');
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const disc = document.getElementById('disc')!;
          const shelf = document.querySelector('[data-testid="tv-shelf-r"]')!;
          const want = Math.min(disc.scrollTop + shelf.getBoundingClientRect().top - disc.getBoundingClientRect().top, disc.scrollHeight - disc.clientHeight);
          return Math.abs(disc.scrollTop - want) < 2 && disc.scrollTop > 0;
        }),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the Recently added shelf's title never scrolled to the top" },
    );
    await assertNoClipping();
    await key('ArrowDown');
    await browser.pause(300);
    expect(await focused()).toBe('r503');
    // The hero keeps its height whatever is focused.
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-hero"]')!.getBoundingClientRect().height)).toBe(heroHeight);
    await key('ArrowRight');
    await waitFocused('r504');
    await waitText('tv-hero-title', 'Upcoming Thing');
    await assertNoClipping();
  });

  it('A opens Details over the tab, and B comes back to the cover (items 93, 172)', async () => {
    await key('Enter');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never opened Details' });
    await waitText('tv-crumb', /Upcoming Thing/);
    await key('Escape');
    await waitFocused('r504', 'B did not come back to the cover');
  });

  it("Left from a row's first cover goes to the rail; coming back focuses the cover last focused (items 104, 174)", async () => {
    await key('ArrowLeft');
    await waitFocused('r503');
    await key('ArrowLeft');
    await waitFocused('r-discover');
    // Right comes back to the cover left, in its own row, not the first row.
    await key('ArrowRight');
    await waitFocused('r503', 'Right from the rail did not come back to the cover left');
    await key('e');
    await waitTab('downloads');
    await key('q');
    await waitTab('server');
    await waitFocused('r503', 'coming back to Server did not focus the cover last focused');
  });

  it('a platform: counts, chips, and Y narrows the grid as you type (items 196–206)', async () => {
    await key('ArrowLeft');
    await waitFocused('r-discover');
    await key('ArrowDown');
    await waitFocused('r-2');
    await waitText('tv-srv-count', '2 games · 0 installed');
    await key('ArrowDown');
    await waitFocused('r-1');
    await waitText('tv-srv-count', '5 games · 0 installed');
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-srv-header"] h1')?.textContent)).toBe(SNES);
    expect(await text('tv-srv-emu')).toBe('Emulator: snes9x');
    expect(await text('tv-srv-backup-all')).toBe('Backup all 5…');
    expect(await hints()).toEqual(['A Open', 'Y Search', 'RT Select', 'MENU Menu']);
    await key('ArrowRight');
    await waitFocused('sg501');
    await assertNoClipping();
    // Up from the top row reaches the header, never the rail.
    await key('ArrowUp');
    await browser.waitUntil(async () => (await focused()) !== 'sg501', { timeout: TRANSITION_TIMEOUT });
    expect(await browser.execute(() => !!document.querySelector('[data-testid="tv-srv-header"] .f'))).toBe(true);
    // The emulator chip opens the system's page (item 200).
    await waitFocused('ch-emu');
    expect(await hints()).toContain('A Open in Systems');
    await key('Enter');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chip never opened the system page' });
    await waitText('tv-crumb', new RegExp(SNES));
    await key('Escape');
    await waitFocused('ch-emu');
    await key('ArrowDown');
    await waitFocused('sg501');

    await key('y');
    await typeKeys('neon');
    await waitText('tv-srv-count', '1 of 5 games match · 0 installed');
    await waitText('tv-srv-search', 'Search: neon');
    await key('Enter');
    await browser.waitUntil(async () => !(await $(testId('tv-osk')).isExisting()), { timeout: TRANSITION_TIMEOUT });
    expect(await browser.execute(() => Array.from(document.querySelectorAll('#srv-grid [data-k]')).map((c) => c.getAttribute('data-k')))).toEqual(['sg502']);
    await key('y');
    await typeKeys('\uE003\uE003\uE003\uE003zz');
    await waitText('tv-srv-nomatch', 'No games match “zz”');
    await key('Enter');
    // A new platform clears the search (item 204).
    await key('Escape');
    await waitFocused('r-1');
    await key('ArrowUp');
    await waitFocused('r-2');
    await key('ArrowDown');
    await waitFocused('r-1');
    await waitText('tv-srv-count', '5 games · 0 installed');
    await expect($(testId('tv-srv-search'))).not.toExist();
  });

  it('X installs: the notice offers Open Downloads, then every cover of the game shows it installed (items 172, 195)', async () => {
    await key('ArrowRight');
    await waitFocused('sg501');
    await key('x');
    await waitToast('Starlit Run added to download queue');
    await browser.waitUntil(async () => browser.execute(() => !!document.querySelector('[data-testid="tv-cover-sg501"] .idot')), {
      timeout: 30_000,
      timeoutMsg: 'the installed dot never came on',
    });
    await waitText('tv-srv-count', '5 games · 1 installed');
    await browser.waitUntil(async () => (await hints()).includes('X Play'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'X never became Play' });
  });

  it('Search everything: results on Done, then Y searches again live (items 353–355, ruling 17)', async () => {
    await key('Escape');
    await waitFocused('r-1');
    await key('ArrowUp');
    await waitFocused('r-2');
    await key('ArrowUp');
    await waitFocused('r-discover');
    // The install shows on Discover too.
    await browser.waitUntil(async () => browser.execute(() => !!document.querySelector('[data-testid="tv-cover-n501"] .idot')), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: "Discover's cover never showed the install",
    });
    await expect($(testId('tv-cover-l501'))).toExist();
    await key('y');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Done with nothing typed just closes the keyboard (item 354): Menu is
    // Done on the keyboard (a physical M would type).
    await press('start');
    await browser.waitUntil(async () => !(await $(testId('tv-osk')).isExisting()), { timeout: TRANSITION_TIMEOUT });
    await expect($(testId('tv-search-all'))).not.toExist();

    await key('y');
    await typeKeys('ar');
    await key('Enter');
    await $(testId('tv-search-all')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Done never pushed the results' });
    await waitText('tv-search-heading', '4 results for “ar”');
    expect(await text('tv-crumb')).toContain('Search: ar');
    expect(await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-search-all"] [data-k]')).map((c) => c.getAttribute('data-k')))).toEqual([
      'sr601',
      'sr602',
      'sr505',
      'sr501',
    ]);
    expect(await hints()).toContain('Y Search');
    await key('y');
    await typeKeys('c');
    await waitText('tv-search-heading', '2 results for “arc”');
    await typeKeys('\uE003\uE003\uE003zzz');
    await waitText('tv-search-none', 'Nothing matches. Try fewer letters.');
    await key('Enter');
    await waitText('tv-crumb', /Search: zzz/);
    await key('Escape');
    await waitTab('server');
  });

  it("offline, the tab says the server can't be reached; Try again comes back to Discover (items 207–208)", async () => {
    await setOffline(true);
    await key('m');
    await $(testId('tv-menu-reconnect')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-menu-reconnect')).click();
    await $(testId('tv-server-offline')).waitForExist({ timeout: 30_000, timeoutMsg: 'the offline panel never showed' });
    expect(await text('tv-server-offline')).toContain("Can't reach your server");
    expect(await text('tv-server-offline')).toContain(`${mockUrl()} isn't answering. Your installed games still play from the Library.`);
    await waitFocused('srv-retry');
    expect(await hints()).toEqual(['A Select', 'MENU Menu']);
    await setOffline(false);
    await key('Enter');
    await waitToast(/^Reconnecting to http.*…$/);
    await waitToast(/^Connected to http/);
    await $(testId('tv-server')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitDiscoverFocus();
  });

  it('Open Library on the offline panel shows the Library', async () => {
    await setOffline(true);
    await key('m');
    await $(testId('tv-menu-reconnect')).click();
    await $(testId('tv-server-offline')).waitForExist({ timeout: 30_000 });
    await $(testId('tv-srv-library')).click();
    await waitTab('library');
    await setOffline(false);
    await key('m');
    await $(testId('tv-menu-reconnect')).click();
    await browser.waitUntil(async () => browser.execute(() => !document.querySelector('[data-testid="tv-connection"] .dot.offline')), {
      timeout: 30_000,
      timeoutMsg: 'GRID never reconnected',
    });
  });

  it('Menu › Discover selects Discover and focuses its content (item 365)', async () => {
    await key('e');
    await waitTab('server');
    await waitDiscoverFocus();
    for (let i = 0; i < 6 && !(await focused())?.startsWith('r-'); i++) {
      await key('ArrowLeft');
      await browser.pause(150);
    }
    await waitFocused('r-discover');
    await key('ArrowDown');
    await waitFocused('r-2');
    await key('m');
    await $(testId('tv-menu-discover')).click();
    await waitDiscoverFocus();
    await expect($(testId('tv-srv-rail-discover'))).toHaveAttribute('aria-current', 'true');
    await assertNoClipping();
  });

  it('raised no errors in the page', async () => {
    expect(await browser.execute(() => (window as unknown as { __errs: string[] }).__errs)).toEqual([]);
  });
});
