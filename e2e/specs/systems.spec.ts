import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  forgeUrl,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** One catalog install from the mock forge. */
const INSTALL_WAIT = 40_000;

const config = () => readFileSync(configPath(), 'utf-8');

async function waitForConfig(check: (text: string) => boolean, timeoutMsg: string, timeout = INSTALL_WAIT) {
  await browser.waitUntil(() => check(config()), { timeout, timeoutMsg });
}

/** The row test keys, top to bottom. */
async function rowOrder(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="systems-list"] > [data-testid^="sys-row-"]')).map((r) =>
      (r as HTMLElement).dataset.testid!.replace('sys-row-', ''),
    ),
  );
}

async function statusOf(key: string): Promise<string> {
  return (await $(testId(`sys-status-${key}`)).getAttribute('data-status')) ?? '';
}

async function waitForStatus(key: string, status: string, timeout = INSTALL_WAIT) {
  await browser.waitUntil(async () => (await statusOf(key)) === status, {
    timeout,
    timeoutMsg: `${key} never showed status ${status} (it shows ${await statusOf(key)})`,
  });
}

/** A real pick fires `change`; WebKitGTK's WebDriver select does not, so
 *  make the pick the way the browser reports it. */
async function pick(key: string, value: string) {
  await browser.execute(
    (id, v) => {
      const select = document.querySelector(`[data-testid="${id}"]`) as HTMLSelectElement;
      select.value = v;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    },
    `sys-pick-${key}`,
    value,
  );
}

/** Each picker group's option texts. */
async function groups(key: string): Promise<Record<string, string[]>> {
  return browser.execute((id) => {
    const select = document.querySelector(`[data-testid="${id}"]`) as HTMLSelectElement;
    const out: Record<string, string[]> = {};
    for (const g of Array.from(select.querySelectorAll('optgroup'))) {
      out[g.label] = Array.from(g.querySelectorAll('option')).map((o) => `${o.value}=${o.textContent?.trim()}`);
    }
    return out;
  }, `sys-pick-${key}`);
}

const PS2 = 'Sony PlayStation 2';
const DC = 'Sega Dreamcast';
const PCSX2 = 'PCSX2 (Playstation 2)';
const REDREAM = 'Redream (Sega Dreamcast)';

/**
 * Stage `systems`: Settings › Systems on a fresh profile, against the
 * first-run-setup fixtures (PS2 and Dreamcast, whose emulators the mock forge
 * serves; SNES, which only RetroArch serves and the mock forge cannot; Zeebo,
 * which nothing installable serves) and the mock forge.
 */
describe('Settings › Systems', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(path.join(dataDir(), 'library'));
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('nav-settings')).click();
    await $(testId('settings-nav-systems')).click();
    await $(testId('systems-list')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  });

  it('lists one row per system, those with no emulator first, then Windows', async () => {
    await browser.waitUntil(async () => (await rowOrder()).length === 5, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the system rows never appeared',
    });
    // The Emulators shelf holds emulators, not games: never a row.
    expect(await rowOrder()).toEqual(['dc', 'ps2', 'snes', 'zeebo', 'windows']);
    for (const key of ['dc', 'ps2', 'snes', 'zeebo']) expect(await statusOf(key)).toBe('none');
    // Open, with the reason; Zeebo points to Add an emulator manually.
    await expect($(testId('sys-missing-zeebo'))).toHaveText(expect.stringContaining('Add an emulator manually'));
    await expect($(testId('sys-install-zeebo'))).not.toExist();
    await expect($(testId('sys-install-ps2'))).toHaveText('Install PCSX2');
    await expect($(testId('systems-install-recommended'))).toHaveText('Install recommended (3)');
  });

  it('groups the picker into Installed and Download, recommended first', async () => {
    const ps2 = await groups('ps2');
    expect(ps2.Installed).toBeUndefined();
    expect(ps2.Download[0]).toBe('install:PCSX2/pcsx2=Install PCSX2 (recommended)');
    expect(ps2.Download.slice(1).every((o) => !o.includes('(recommended)'))).toBe(true);
    // The Proton catalog loads on its own.
    await browser.waitUntil(async () => ((await groups('windows')).Download ?? []).length > 0, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Windows picker never offered a Proton download',
    });
    const windows = await groups('windows');
    expect(windows.Download[0]).toMatch(/=Install GE-Proton \(latest, recommended\)$/);
  });

  it('installs the recommended emulator with one click and makes it the system’s emulator', async () => {
    await $(testId('sys-install-dc')).click();
    await waitForConfig((c) => c.includes(`"${DC}" = "${REDREAM}"`), 'Redream never became Dreamcast’s emulator');
    await waitForStatus('dc', 'ready');
    await expect($(testId('systems-install-recommended'))).toHaveText('Install recommended (2)');
  });

  it('a Download pick installs the emulator and makes it the system’s emulator', async () => {
    await pick('ps2', 'install:PCSX2/pcsx2');
    await waitForConfig((c) => c.includes(`"${PS2}" = "${PCSX2}"`), 'PCSX2 never became PS2’s emulator');
    await waitForStatus('ps2', 'ready');
    expect((await groups('ps2')).Installed).toEqual([`use:${PCSX2}=${PCSX2} (recommended)`]);
    // Systems with an emulator move below the ones without.
    expect(await rowOrder()).toEqual(['snes', 'zeebo', 'dc', 'ps2', 'windows']);
  });

  it('None keeps the explicit-none marker, and an Installed pick sets the default', async () => {
    await pick('ps2', '');
    await waitForConfig((c) => c.includes(`"${PS2}" = "<none>"`), 'None never reached config.toml', TRANSITION_TIMEOUT);
    await waitForStatus('ps2', 'none', TRANSITION_TIMEOUT);
    await pick('ps2', `use:${PCSX2}`);
    await waitForConfig((c) => c.includes(`"${PS2}" = "${PCSX2}"`), 'the Installed pick never reached config.toml', TRANSITION_TIMEOUT);
    await waitForStatus('ps2', 'ready', TRANSITION_TIMEOUT);
  });

  it('says when an update is out, after Check for updates', async () => {
    await fetch(`${forgeUrl()}/__e2e__/pcsx2-release/updated`, { method: 'POST' });
    await $(testId('systems-check-updates')).click();
    await waitForStatus('ps2', 'update', TRANSITION_TIMEOUT * 2);
    await $(testId('sys-update-sony-playstation-2')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('sys-update-sony-playstation-2'))).toHaveText(expect.stringContaining('Update to'));
  });

  it('opens the ⋯ menu beside its button near the bottom edge, and closes it on scroll', async () => {
    await $(testId('sys-toggle-dc')).click();
    const more = testId('sys-more-sega-dreamcast');
    await $(more).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // Push the button to just above the window's bottom edge.
    await browser.execute((sel) => {
      const button = document.querySelector(sel) as HTMLElement;
      const page = document.querySelector('[data-testid="systems-page"]') as HTMLElement;
      page.style.paddingTop = '0px';
      const room = window.innerHeight - button.getBoundingClientRect().bottom - 24;
      if (room > 0) page.style.paddingTop = `${room}px`;
      else button.scrollIntoView({ block: 'end' });
    }, more);
    // A script click: WebDriver's own click scrolls the button into view
    // first, and any scroll closes the menu by design.
    await browser.pause(300);
    await browser.execute((sel) => (document.querySelector(sel) as HTMLElement).click(), more);
    await $(testId('sys-menu')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const geometry = await browser.execute((sel) => {
      const b = (document.querySelector(sel) as HTMLElement).getBoundingClientRect();
      const m = document.querySelector('[data-testid="sys-menu"]') as HTMLElement;
      const r = m.getBoundingClientRect();
      return {
        side: m.dataset.side,
        menu: { top: r.top, left: r.left, right: r.right, bottom: r.bottom },
        button: { top: b.top, left: b.left, right: b.right, bottom: b.bottom },
        height: window.innerHeight,
      };
    }, more);
    expect(['left', 'right']).toContain(geometry.side);
    const { menu, button } = geometry;
    expect(menu.bottom).toBeLessThanOrEqual(geometry.height - 8 + 0.5);
    expect(menu.top).toBeGreaterThanOrEqual(8 - 0.5);
    const overlaps = menu.left < button.right && button.left < menu.right && menu.top < button.bottom && button.top < menu.bottom;
    expect(overlaps).toBe(false);
    // A fixed menu would drift from its button: a scroll closes it.
    await browser.execute(() => {
      const pane = document.querySelector('[data-testid="settings-page-systems"]') as HTMLElement;
      pane.scrollTop += 1;
      pane.dispatchEvent(new Event('scroll'));
    });
    await $(testId('sys-menu')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await browser.execute(() => {
      (document.querySelector('[data-testid="systems-page"]') as HTMLElement).style.paddingTop = '';
    });
  });

  it('removes an emulator only on the second click', async () => {
    await browser.execute(() => {
      (document.querySelector('[data-testid="sys-more-sega-dreamcast"]') as HTMLElement).click();
    });
    await $(testId('sys-menu-remove')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await $(testId('sys-menu-remove')).click();
    await expect($(testId('sys-menu-remove'))).toHaveText(`Remove ${REDREAM}? Games and saves stay`);
    expect(config()).toContain(`name = "${REDREAM}"`);
    await $(testId('sys-menu-remove')).click();
    await waitForConfig((c) => !c.includes(`name = "${REDREAM}"`), 'Redream was never removed', TRANSITION_TIMEOUT * 2);
    await waitForStatus('dc', 'none', TRANSITION_TIMEOUT);
  });

  it('Install recommended (N) installs for every system that has none', async () => {
    await expect($(testId('systems-install-recommended'))).toHaveText('Install recommended (2)');
    await $(testId('systems-install-recommended')).click();
    // SNES's RetroArch is queued too; the mock forge cannot serve it, but its
    // default waits in config.toml all the same.
    await waitForConfig(
      (c) => c.includes('[setup.pending."Super Nintendo Entertainment System"]'),
      'SNES was never queued',
      TRANSITION_TIMEOUT,
    );
    await waitForConfig((c) => c.includes(`"${DC}" = "${REDREAM}"`), 'Redream never came back as Dreamcast’s emulator');
    await waitForStatus('dc', 'ready');
  });

  it('adds an emulator by hand without making it any system’s emulator', async () => {
    const before = config().match(/\[default_emulators\][^[]*/)?.[0] ?? '';
    const program = path.join(dataDir(), 'stub-dolphin');
    writeFileSync(program, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
    await $(testId('systems-add-manual')).click();
    await $(testId('systems-sheet')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const dolphin = await browser.execute(() => {
      const select = document.querySelector('[data-testid="emu-form-which"]') as HTMLSelectElement;
      const option = Array.from(select.options).find((o) => o.value.startsWith('Dolphin'));
      if (!option) return '';
      select.value = option.value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
      return option.value;
    });
    expect(dolphin).not.toBe('');
    await $(testId('emu-form-path')).setValue(program);
    await $(testId('emu-form-save')).click();
    await $(testId('systems-sheet')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await waitForConfig((c) => c.includes(`name = "${dolphin}"`), 'the manual entry never reached config.toml', TRANSITION_TIMEOUT);
    // No fixture system is a GameCube or Wii: it is nobody's emulator.
    await $(testId('systems-other-list')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    const after = config().match(/\[default_emulators\][^[]*/)?.[0] ?? '';
    expect(after).toBe(before);
    expect(after).not.toContain('Dolphin');
  });
});
