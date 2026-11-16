import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { connectPad, key, waitFocused, waitToast, XBOX_PAD } from '../helpers/tv.js';
import { moveTo, testId, text, waitText } from '../helpers/tvDetails.js';
import { tap } from '../helpers/tvBackups.js';

const US = 'FAKE-PS1-BIOS-US-not-real\n'.repeat(4);
const JP = 'FAKE-PS1-BIOS-JP-not-real\n'.repeat(4);
const picks = () => path.join(dataDir(), 'picks');

/** With E2E_TV_SCREENS=1, screens are saved under ctr-out/tv-screens/systems-local/. */
async function shot(name: string): Promise<void> {
  if (process.env.E2E_TV_SCREENS !== '1') return;
  const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'tv-screens', 'systems-local');
  await browser.pause(400);
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(path.join(dir, `${name}.png`));
}
const bios = (name: string) => path.join(dataDir(), 'stubs', 'duckstation', 'bios', name);

/** A settings row's words: its label, the line under it, its value. */
async function rowText(id: string): Promise<string> {
  return (await browser.execute((sel: string) => {
    const r = document.querySelector(sel);
    if (!r) return '';
    return ['.t', '.s', '.val']
      .map((c) => (r.querySelector(c)?.textContent ?? '').replace(/\s+/g, ' ').trim())
      .filter((t) => t !== '')
      .join(' ');
  }, testId(id))) as string;
}

/** The file picker: up to the root, then down into `target`. */
async function browseTo(target: string): Promise<void> {
  await $(testId('tv-fp-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  for (let i = 0; i < 30; i++) {
    const at = await text('tv-fp-path');
    if (target === at || target.startsWith(at.endsWith('/') ? at : `${at}/`)) break;
    await tap('#fp-list [data-k="up"]');
    await browser.waitUntil(async () => (await text('tv-fp-path')) !== at, { timeout: TRANSITION_TIMEOUT });
  }
  for (let i = 0; i < 30; i++) {
    const at = await text('tv-fp-path');
    if (at === target) return;
    const seg = target.slice(at.length).replace(/^\/+/, '').split('/')[0];
    await tap(`#fp-list [data-k="d-${seg}"]`);
    await browser.waitUntil(async () => (await text('tv-fp-path')) !== at, { timeout: TRANSITION_TIMEOUT });
  }
  await waitText('tv-fp-path', target);
}

/** The report page's rows in one group: "<label> <sub> <pill>". */
async function groupRows(kind: 'ok' | 'nv' | 'bad'): Promise<string[]> {
  return (await browser.execute((k: string) => {
    const head = document.querySelector(`[data-testid="tv-locate-group-${k}"]`);
    const out: string[] = [];
    let el = head?.nextElementSibling ?? null;
    while (el && el.classList.contains('srow')) {
      out.push(
        ['.t', '.s', '.val']
          .map((c) => (el!.querySelector(c)?.textContent ?? '').replace(/\s+/g, ' ').trim())
          .filter((t) => t !== '')
          .join(' '),
      );
      el = el.nextElementSibling;
    }
    return out;
  }, kind)) as string[];
}

/**
 * Stage `tv-systems-local` (TVRommStorefront README, "Settings › Systems
 * without RomM", "The firmware block", "The TV file browser in file mode",
 * "The Locate report"): the systems-local profile in TV mode. Systems
 * without RomM, PlayStation's firmware block, Locate files… through the
 * file browser's file mode with ticks, the check and the report.
 */
describe('tv-systems-local: Systems and Locate files in TV mode', () => {
  before(async () => {
    await $(testId('tv-shell')).waitForExist({ timeout: 60_000, timeoutMsg: 'TV mode never opened' });
    await connectPad(XBOX_PAD);
    await tap('[data-testid="tv-tab-settings"]');
    await $(testId('tv-settings')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitFocused('r-appearance');
    await moveTo('r-systems', 'ArrowDown', 12);
    await waitText('tv-pane-title', 'Systems');
  });

  it('lists your systems with their games, the no-RomM line and More systems', async () => {
    await $(testId('tv-sys-row-psx')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-sys-no-romm', 'Systems works without RomM. With RomM connected, its platforms join this list.');
    expect(await text('tv-sys-sub-psx')).toBe('DuckStation (Playstation 1) · 1 installed');
    await waitText('tv-sys-status-psx', 'Firmware needed');
    await expect($(testId('tv-sys-more'))).toBeExisting();
    expect(await rowText('tv-sys-more')).toMatch(/^More systems \d+ systems GRID can set up \d+$/);
    await shot('01-systems-pane');
  });

  it('the System page shows the firmware block; Install from RomM needs RomM', async () => {
    await tap('[data-k="sy-psx"]');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('tv-system-firmware')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    expect(await rowText('tv-row-fw-file-scph5501.bin')).toBe('scph5501.bin 1 KB · required Missing · required');
    expect(await rowText('tv-row-fw-romm')).toContain('Needs RomM');
    expect(await $(testId('tv-row-fw-romm')).getAttribute('aria-disabled')).toBe('true');
    expect(await rowText('tv-row-fw-locate')).toBe('Locate files… Pick files on this computer or a drive. GRID installs the ones it knows.');
    await $(testId('tv-system-firmware')).scrollIntoView();
    await shot('02-system-page-firmware');
  });

  it('Locate files: picks with ticks across the folder, Start checks them, the report', async () => {
    await tap('[data-k="s-fw-locate"]');
    await $(testId('tv-file-picker')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await waitText('tv-crumb', /Locate PlayStation firmware/);
    await browseTo(picks());
    await waitText('tv-fp-picks', 'No files picked. Pick files with (A), or check this whole folder with (Y).');
    // Each file shows its size (a 104-byte fake BIOS rounds up to 1 KB); a folder shows none.
    expect(await text('tv-fp-size-scph5501 (1).bin')).toBe('1 KB');
    expect(await $(testId('tv-fp-size-more')).isExisting()).toBe(false);
    for (const f of ['scph5501 (1).bin', 'scph5502.bin', 'bios-pack.zip', 'readme.txt']) {
      await tap(`#fp-list [data-k="f-${f}"]`);
    }
    await waitText('tv-fp-picks', '4 files picked: scph5501 (1).bin, scph5502.bin, bios-pack.zip, readme.txt');
    await shot('03-file-picker-picks');
    await key('m');
    await $(testId('tv-locate-report')).waitForExist({ timeout: 20_000, timeoutMsg: 'the report page never opened' });
    await shot('04-locate-report');
    await waitToast('2 firmware files installed for DuckStation');
    expect(await text('tv-locate-report')).toContain('Checked 4 files you picked for PlayStation · DuckStation');
    expect(await groupRows('ok')).toEqual([
      'scph5501.bin From scph5501 (1).bin · size and MD5 match Installed',
      'scph5500.bin From bios-pack.zip · size and MD5 match Installed',
    ]);
    expect(await groupRows('bad')).toEqual([
      "scph5502.bin This isn't the scph5502.bin GRID knows (size or MD5 differs) Refused",
      "readme.txt GRID doesn't know this file for PlayStation Refused",
    ]);
    expect(readFileSync(bios('scph5501.bin'), 'utf-8')).toBe(US);
    expect(readFileSync(bios('scph5500.bin'), 'utf-8')).toBe(JP);
  });

  it('Done goes back to the System page with the files marked', async () => {
    await key('m');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await rowText('tv-row-fw-file-scph5501.bin')).endsWith(' Installed'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `scph5501.bin never read Installed (${await rowText('tv-row-fw-file-scph5501.bin')})`,
    });
    expect(await rowText('tv-row-fw-file-scph5501.bin')).toContain('checked by GRID');
  });

  it('Y checks the whole folder and counts the files that match nothing', async () => {
    await tap('[data-k="s-fw-locate"]');
    await $(testId('tv-file-picker')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await browseTo(picks());
    await key('y');
    await $(testId('tv-locate-report')).waitForExist({ timeout: 20_000 });
    expect(await text('tv-locate-report')).toContain(`Checked ${picks()} for PlayStation · DuckStation`);
    await waitText('tv-locate-unmatched', '2 other files matched nothing GRID knows for PlayStation.');
    await key('m');
    await $(testId('tv-system-page')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  });
});
