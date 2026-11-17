import { TRANSITION_TIMEOUT } from '../helpers/env.js';
import { unplug } from '../helpers/drives.js';
import { tvRailTo } from '../helpers/romm.js';
import { assertNoClipping, connectPad, focused, hints, key, toastTitles, waitFocused, waitTab, XBOX_PAD } from '../helpers/tv.js';
import { tapK } from '../helpers/tvBackups.js';

const testId = (id: string) => `[data-testid="${id}"]`;

async function text(id: string): Promise<string> {
  return ((await browser.execute((s: string) => document.querySelector(s)?.textContent ?? '', testId(id))) as string).trim();
}

async function waitText(id: string, want: string | RegExp): Promise<void> {
  let last = '';
  await browser
    .waitUntil(
      async () => {
        last = await text(id);
        return typeof want === 'string' ? last === want : want.test(last);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `${id} never read ${String(want)}` },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (it reads "${last}")`);
    });
}

/** The rail's children: rows by key, folder headers, dividers and notes. */
async function railShape(): Promise<string[]> {
  return (await browser.execute(() => {
    const nav = document.querySelector('[data-testid="tv-library"] [data-rail]')!;
    // A folder's panel (`.rgrp`, 2026-11-16) is a wrapper, not a row: its
    // heading and rows read as the rail's own, in order.
    const rows = Array.from(nav.children).flatMap((c) => (c.classList.contains('rgrp') ? Array.from(c.children) : [c]));
    return rows.map((c) => (c.tagName === 'BUTTON' ? c.getAttribute('data-k')! : c.tagName === 'HR' ? 'HR' : `note:${c.textContent?.trim()}`));
  })) as string[];
}

/** The grid's covers, in order. */
async function covers(): Promise<string[]> {
  return (await browser.execute(() => Array.from(document.querySelectorAll('#lib-grid [data-cover]')).map((c) => c.getAttribute('data-k')!))) as string[];
}

async function waitCovers(want: string[]): Promise<void> {
  let last: string[] = [];
  await browser
    .waitUntil(
      async () => {
        last = await covers();
        return JSON.stringify(last) === JSON.stringify(want);
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `the grid never showed ${want.join(', ')}` },
    )
    .catch((e) => {
      throw new Error(`${(e as Error).message} (it shows ${last.join(', ')})`);
    });
}

async function typeKeys(s: string): Promise<void> {
  await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  for (const ch of s) await browser.keys(ch);
}

async function waitOskClosed(): Promise<void> {
  await browser.waitUntil(async () => !(await $(testId('tv-osk')).isExisting()), { timeout: TRANSITION_TIMEOUT });
}

/** Moves Down through the rail until `k` is focused. */
async function railTo(k: string, dir: 'ArrowDown' | 'ArrowUp' = 'ArrowDown'): Promise<void> {
  for (let i = 0; i < 12 && (await focused()) !== k; i++) {
    await key(dir);
    await browser.pause(60);
  }
  await waitFocused(k);
}

/**
 * Stage `tv-library` (docs/tv-mode/plan.md P8), seeded used without a
 * server (seed/tv-library-seed.mjs): Alpha Quest and Bravo Run (SNES),
 * Delta Drive (Sega Genesis), and TESTDRIVE's "Travel kit" pack with Travel
 * Game (PlayStation 2) and Alpha Quest. Starts with --tv on the Library.
 */
describe('tv-library: the Library tab', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
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
    await waitTab('library');
    await connectPad(XBOX_PAD);
  });

  it('lists All games, Recent, Updates, a divider, the Backup drives folder and the RomM group, with counts (items 176–177, 183–184, 420)', async () => {
    let shape: string[] = [];
    await browser.waitUntil(
      async () => {
        shape = await railShape();
        return shape.some((k) => k.startsWith('r-drive:'));
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the drive never reached the rail' },
    ).catch(() => {
      throw new Error(`rail: ${shape.join(' | ')}`);
    });
    const drive = shape.find((k) => k.startsWith('r-drive:'))!;
    // No PLATFORMS folder: platform rows live in the RomM group, and only
    // while RomM is connected; with RomM off it has Installed alone
    // (romm-storefront-plan decision 11).
    expect(shape).toEqual(['r-all', 'r-recent', 'r-updates', 'HR', 'fold-drives', drive, 'fold-romm', 'r-romm:installed']);
    expect(await text('tv-lib-rail-all')).toBe('All games3');
    expect(await text('tv-lib-rail-recent')).toBe('Recent1');
    expect(await text('tv-lib-rail-updates')).toBe('Updates0');
    expect(await text('tv-lib-rail-romm-installed')).toBe('Installed3');
    expect(await browser.execute((k: string) => document.querySelector(`[data-k="${k}"]`)?.textContent?.trim(), drive)).toBe('Travel kit2');
    // Rows in a folder are indented under its header.
    const pad = await browser.execute((k: string) => {
      const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
      return parseFloat(getComputedStyle(document.querySelector(`[data-k="${k}"]`)!).paddingLeft) / rem;
    }, 'r-romm:installed');
    expect(pad).toBeCloseTo(2.3, 1);
    // Never connected to RomM, GRID opens on All games (decision 16), not
    // the first Backup drive.
    await waitFocused('r-all');
    await waitText('tv-lib-name', 'All games');
    await waitText('tv-lib-count', '3 games');
    // Recently played first; never played last. Platform tags in a mixed list.
    await waitCovers(['lg101', 'lg103', 'lg102']);
    expect(await browser.execute(() => document.querySelector('[data-testid="tv-cover-lg101"] .tag.plat')?.textContent)).toBe('SNES');
    expect(await browser.execute(() => !!document.querySelector('[data-testid="tv-cover-lg101"] .idot'))).toBe(true);
    expect(await hints()).toEqual(['A Open', 'Y Search', 'LT Sort', 'RT Select', 'MENU Menu']);
    expect(await text('tv-lib-sort')).toBe('Sort: Recently played');
  });

  it('moving through the rail filters at once; empty filters say why (items 178, 186, 190)', async () => {
    await key('ArrowDown');
    await waitFocused('r-recent');
    await waitText('tv-lib-name', 'Recent');
    await waitText('tv-lib-count', '1 game');
    await waitCovers(['lg101']);
    await key('ArrowDown');
    await waitFocused('r-updates');
    await waitText('tv-lib-empty', /^Everything is up to date\s*Game updates show here\.$/);
    await key('ArrowDown');
    await waitFocused('fold-drives');
    // The header still names the filter: a folder header selects nothing.
    expect(await text('tv-lib-name')).toBe('Updates');
    expect(await hints()).toContain('A Close');
    await key('ArrowDown');
    await key('ArrowDown');
    await waitFocused('fold-romm');
    await waitText('tv-lib-name', 'Travel kit');
    await key('ArrowDown');
    await waitFocused('r-romm:installed');
    await waitText('tv-lib-name', /^RomM\s*· Installed$/);
    let keys: string[] = [];
    await browser.waitUntil(async () => (keys = await covers()).length === 3, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'RomM › Installed never showed its three games' });
    // Platform tags in RomM › Installed, a mixed list (item 186).
    expect(keys.every((k) => k.startsWith('rg'))).toBe(true);
    expect(await browser.execute(() => document.querySelectorAll('#lib-grid [data-cover] .tag.plat').length)).toBe(3);
  });

  it('Backup all on RomM › Installed opens the Backup page, with RomM off too (item 455)', async () => {
    // Backups copy installed games without RomM (romm-storefront plan Step 2).
    await waitText('tv-romm-backup-all', 'Backup all 3…');
    await tapK('rh-bk');
    await $('[data-testid="tv-backup-page"]').waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('Escape');
    await $('[data-testid="tv-backup-page"]').waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
  });

  it('A or Right goes into the grid; Up from the top row stays put; Left and B go to the selected row (items 89, 179)', async () => {
    await tvRailTo('r-all');
    await waitCovers(['lg101', 'lg103', 'lg102']);
    await key('Enter');
    await waitFocused('lg101');
    await assertNoClipping();
    expect(await hints()).toEqual(['A Details', 'X Play', 'Y Search', 'LT Sort', 'RT Select', 'MENU Menu']);
    await key('ArrowUp');
    await browser.pause(300);
    expect(await focused()).toBe('lg101');
    await key('ArrowLeft');
    await waitFocused('r-all');
    await key('ArrowRight');
    await waitFocused('lg101');
    await key('ArrowRight');
    await waitFocused('lg103');
    await key('Escape');
    await waitFocused('r-all');
    await key('ArrowRight');
    await waitFocused('lg103', 'Right did not come back to the cover last focused');
  });

  it('A on a folder header closes and opens it; closed, the header still names the filter and stays closed for the session (items 180–183)', async () => {
    await key('Escape');
    await waitFocused('r-all');
    await railTo('r-romm:installed');
    await waitText('tv-lib-name', /^RomM\s*· Installed$/);
    await railTo('fold-romm', 'ArrowUp');
    await key('Enter');
    await browser.waitUntil(async () => !(await railShape()).includes('r-romm:installed'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder never closed' });
    expect(await browser.execute(() => document.querySelector('[data-k="fold-romm"] .chev')?.classList.contains('open'))).toBe(false);
    expect(await browser.execute(() => document.querySelector('[data-k="fold-romm"]')?.getAttribute('aria-expanded'))).toBe('false');
    expect(await hints()).toContain('A Open');
    expect(await text('tv-lib-name')).toMatch(/^RomM\s*· Installed$/);
    // Right still goes to the grid; B comes back to the header holding the selection.
    await key('ArrowRight');
    await browser.waitUntil(async () => ((await focused()) ?? '').startsWith('rg'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Right never reached the grid' });
    await key('Escape');
    await waitFocused('fold-romm');
    expect(await text('tv-lib-name')).toMatch(/^RomM\s*· Installed$/);
    // Kept across a tab switch.
    await key('e');
    await waitTab('downloads');
    await key('q');
    await waitTab('library');
    expect(await railShape()).not.toContain('r-romm:installed');
    await waitFocused('fold-romm');
    await key('Enter');
    await browser.waitUntil(async () => (await railShape()).includes('r-romm:installed'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the folder never opened' });
    expect(await browser.execute(() => document.querySelector('[data-k="fold-romm"] .chev')?.classList.contains('open'))).toBe(true);
  });

  it('LT sorts; Y searches live; Done keeps the search; B restores it; a new rail row clears it (items 185, 187–189, 193)', async () => {
    await railTo('r-all', 'ArrowUp');
    await waitCovers(['lg101', 'lg103', 'lg102']);
    await key('z');
    await $(testId('tv-lib-sort-played')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await browser.execute(() => document.querySelector('.f')?.getAttribute('data-testid'))).toBe('tv-lib-sort-played');
    const labels = await browser.execute(() => Array.from(document.querySelectorAll('[data-testid^="tv-lib-sort-"]')).map((e) => e.textContent?.trim()));
    expect(labels).toEqual(['Recently played', 'Recently installed', 'Title', 'Platform']);
    await key('ArrowDown');
    await key('Enter');
    await waitText('tv-lib-sort', 'Sort: Recently installed');
    await waitCovers(['lg103', 'lg102', 'lg101']);
    await key('z');
    await $(testId('tv-lib-sort-title')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await key('ArrowDown');
    await key('Enter');
    await waitText('tv-lib-sort', 'Sort: Title');
    await waitCovers(['lg101', 'lg102', 'lg103']);

    await key('y');
    await $(testId('tv-osk')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await $(testId('tv-osk')).getAttribute('aria-label')).toBe('Search your library');
    await typeKeys('ra');
    await waitCovers(['lg102']);
    await waitText('tv-lib-count', '1 game');
    await waitText('tv-lib-search', 'Search: ra');
    await key('Enter');
    await waitOskClosed();
    await waitCovers(['lg102']);
    // No match, then B: the field keeps the value it had.
    await key('y');
    await typeKeys('zz');
    await waitText('tv-lib-empty', /^No games match “razz”/);
    expect(await browser.execute(() => !!document.querySelector('[data-testid="tv-lib-empty"] .gli'))).toBe(true);
    await key('Escape');
    await waitOskClosed();
    await waitCovers(['lg102']);
    // An emptied field is no filter.
    await key('y');
    await typeKeys('');
    await waitCovers(['lg101', 'lg102', 'lg103']);
    await expect($(testId('tv-lib-search'))).not.toExist();
    await waitText('tv-lib-count', '3 games');
    await typeKeys('alp');
    await key('Enter');
    await waitOskClosed();
    await waitCovers(['lg101']);
    // Another rail row clears the search.
    await railTo('r-recent');
    await railTo('r-all', 'ArrowUp');
    await waitCovers(['lg101', 'lg102', 'lg103']);
    await expect($(testId('tv-lib-search'))).not.toExist();
  });

  it('A on a cover opens Details; B comes back to it (item 192)', async () => {
    await key('ArrowRight');
    await waitFocused('lg101');
    await key('ArrowRight');
    await waitFocused('lg102');
    await key('Enter');
    await $(testId('tv-details')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never opened Details' });
    await waitText('tv-crumb', /Bravo Run/);
    await key('Escape');
    await waitFocused('lg102', 'B did not come back to the cover');
  });

  it("a drive: header, chips, platform tags; X installs from the drive and the covers flip to installed (items 194–195, 420–424)", async () => {
    await key('Escape');
    const drive = (await railShape()).find((k) => k.startsWith('r-drive:'))!;
    await railTo(drive);
    await waitText('tv-lib-name', 'Travel kit');
    await waitText('tv-lib-count', /^2 games · 48(\.\d)? MB · on TESTDRIVE$/);
    expect(await text('tv-lib-emus')).toBe('Emulators: PCSX2 (Playstation 2)');
    expect(await text('tv-lib-fw')).toBe('Firmware: 1 system');
    let keys: string[] = [];
    await browser.waitUntil(async () => (keys = await covers()).length === 2, { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the drive's games never showed" });
    // Title order: Alpha Quest, Travel Game.
    const [alpha, travel] = keys;
    expect(alpha.startsWith('dg101-')).toBe(true);
    expect(travel.startsWith('dg777-')).toBe(true);
    expect(await browser.execute((k: string) => document.querySelector(`[data-k="${k}"] .tag.plat`)?.textContent, travel)).toBe('PS2');
    expect(await browser.execute((k: string) => !!document.querySelector(`[data-k="${k}"] .idot`), alpha)).toBe(true);
    await key('Enter');
    await waitFocused(alpha);
    expect(await hints()).toContain('X Play');
    await key('ArrowRight');
    await waitFocused(travel);
    expect(await hints()).toContain('X Install from drive');
    await assertNoClipping();
    await key('x');
    const seen = new Set<string>();
    let tagged = false;
    await browser
      .waitUntil(
        async () => {
          for (const t of await toastTitles()) seen.add(t);
          if (await browser.execute((k: string) => !!document.querySelector(`[data-k="${k}"] .tag.q`), travel)) tagged = true;
          return browser.execute((k: string) => !!document.querySelector(`[data-k="${k}"] .idot`), travel);
        },
        { timeout: 60_000, interval: 200 },
      )
      .catch(async () => {
        const errs = await browser.execute(() => (window as unknown as { __errs: string[] }).__errs);
        throw new Error(`the installed dot never came on the drive cover (focused ${await focused()}, hints ${JSON.stringify(await hints())}, cards ${JSON.stringify([...seen])}, errors ${JSON.stringify(errs)})`);
      });
    // The queue card (the rest of the Game backups card's notices is P13's,
    // items 425–429).
    console.log(`install from drive: tag shown ${tagged}, cards ${JSON.stringify([...seen])}`);
    expect(tagged).toBe(true);
    // Cards are merged for a moment before they show, so the copy may be done first.
    await browser.waitUntil(
      async () => {
        for (const t of await toastTitles()) seen.add(t);
        // A copy that finishes within the merge window shows as one card.
        return seen.has('Travel Game added to download queue') || seen.has('Travel Game installed');
      },
      { timeout: TRANSITION_TIMEOUT, interval: 200 },
    ).catch(() => {
      throw new Error(`no card for Travel Game (cards: ${JSON.stringify([...seen])})`);
    });
    expect(await browser.execute((k: string) => !!document.querySelector(`[data-k="${k}"] .tag.q`), travel)).toBe(false);
    await waitText('tv-lib-rail-all', 'All games4');
  });

  it('an unplugged drive leaves the rail, which falls back to All games (items 420, 493)', async () => {
    unplug('TESTDRIVE');
    await waitText('tv-lib-no-drives', 'No backup drives connected');
    await waitText('tv-lib-name', 'All games');
    await key('Escape');
    await waitFocused('r-all');
    // The RomM group ends the rail: its last row scrolls the rail to its very bottom.
    await railTo('r-romm:installed');
    const atEnd = await browser.execute(() => {
      const nav = document.querySelector('[data-testid="tv-library"] [data-rail]') as HTMLElement;
      return nav.scrollTop + nav.clientHeight >= nav.scrollHeight - 2;
    });
    expect(atEnd).toBe(true);
  });

  it('had no page errors', async () => {
    const errs = (await browser.execute(() => (window as unknown as { __errs: string[] }).__errs)) as string[];
    expect(errs).toEqual([]);
  });
});
