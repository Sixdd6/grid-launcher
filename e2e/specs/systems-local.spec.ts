import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, dataDir, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { openSystems, statusOf } from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** With E2E_SCREENSHOTS=1, screens are saved under ctr-out/systems-local/
 *  (git-ignored) for the design parity check. */
async function shot(name: string): Promise<void> {
  if (!process.env.E2E_SCREENSHOTS) return;
  const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'systems-local');
  mkdirSync(dir, { recursive: true });
  await browser.pause(200);
  await browser.saveScreenshot(path.join(dir, `${name}.png`));
}

const US = 'FAKE-PS1-BIOS-US-not-real\n'.repeat(4);
const JP = 'FAKE-PS1-BIOS-JP-not-real\n'.repeat(4);

const picks = () => path.join(dataDir(), 'picks');
const bios = (name: string) => path.join(dataDir(), 'stubs', 'duckstation', 'bios', name);

async function textOf(id: string): Promise<string> {
  return ((await $(testId(id)).getText()) ?? '').replace(/\s+/g, ' ').trim();
}

/** The report's lines in one section: "<file …> | <note>". */
async function reportItems(kind: 'ok' | 'nv' | 'bad'): Promise<string[]> {
  return (await browser.execute((k: string) => {
    const sec = document.querySelector(`[data-testid="locate-report-${k}"]`);
    return Array.from(sec?.querySelectorAll('li') ?? []).map((li) =>
      Array.from(li.children)
        .map((c) => (c.textContent ?? '').replace(/\s+/g, ' ').trim())
        .join(' | '),
    );
  }, kind)) as string[];
}

/** Opens Locate files for PlayStation and lists the picks folder. */
async function openLocate(): Promise<void> {
  await $(testId('sys-fw-locate-psx')).click();
  await $(testId('locate-dialog')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  // WebKitGTK's WebDriver typing fires no `change`: set the field the way
  // the browser reports a typed path.
  await browser.execute(
    (id: string, v: string) => {
      const input = document.querySelector(`[data-testid="${id}"]`) as HTMLInputElement;
      input.value = v;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    },
    'locate-path',
    picks(),
  );
  await $(testId('locate-file-readme.txt')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the picks folder was never listed' });
}

/**
 * Stage `systems-local` (romm-storefront-plan Step 7, RommStorefront README
 * "Systems (works without RomM)"): a profile that never connected RomM.
 * Settings › Systems lists GRID's own catalog — PlayStation (an emulator and
 * an installed game) in the main list, the catalog's other systems under
 * More systems — and the firmware block's Locate files installs fake
 * firmware checked against the seed's test table.
 */
describe('systems-local: Systems and firmware without RomM', () => {
  before(async () => {
    await $(testId('nav-settings')).waitForExist({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the shell never opened' });
    await openSystems();
  });

  it('lists the catalog without RomM: your systems, then More systems', async () => {
    await expect($(testId('systems-lede'))).toHaveText(
      'Pick an emulator for each system you play. GRID installs, updates and sets it up, with or without RomM.',
    );
    await expect($(testId('sys-row-psx'))).toBeExisting();
    expect(await textOf('sys-toggle-psx')).toContain('1 installed');
    expect(await statusOf('psx')).not.toBe('none');
    await $(testId('systems-more')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await expect($(`${testId('systems-more-list')} ${testId('sys-row-ps2')}`)).toBeExisting();
    expect(await statusOf('ps2')).toBe('unset');
    expect(await textOf('sys-status-ps2')).toBe('Not set up');
    expect(await textOf('sys-toggle-ps2')).toContain('No games yet');
    await shot('01-systems-without-romm');
    await $(testId('systems-more')).scrollIntoView();
    await $(testId('sys-toggle-ps2')).click();
    await $(testId('sys-recommended-ps2')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await textOf('sys-recommended-ps2')).toBe('PCSX2 is recommended for PlayStation 2.');
    await shot('02-more-systems-open');
    await $(testId('sys-toggle-ps2')).click();
    await $(testId('systems-lede')).scrollIntoView();
  });

  it('shows the firmware block: Install from RomM needs RomM, Locate files…', async () => {
    await $(testId('sys-toggle-psx')).click();
    await $(testId('sys-fw-psx')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    await expect($(testId('sys-fw-count-psx'))).toHaveText('3 files · 0 installed');
    await expect($(testId('sys-fw-romm-psx'))).toBeDisabled();
    expect(await textOf('sys-fw-why-psx')).toBe('Install from RomM needs RomM · Connect RomM… · or locate files you have.');
    await expect($(testId('sys-fw-locate-psx'))).toHaveText('Locate files…');
    expect(await $(testId('sys-fw-file-psx-scph5501.bin')).getAttribute('data-state')).toBe('missing');
    // The file line names the BIOS's region (RommStorefront README, "Required · Japan").
    expect(await textOf('sys-fw-file-psx-scph5501.bin')).toContain('Required · North America');
    expect(await statusOf('psx')).toBe('firmware');
    await shot('03-firmware-block-needs-romm');
  });

  it('Locate files installs a renamed dump and a zip member, and refuses the rest', async () => {
    await openLocate();
    for (const f of ['scph5501 (1).bin', 'scph5502.bin', 'bios-pack.zip', 'readme.txt']) {
      await $(testId(`locate-file-${f}`)).click();
    }
    await shot('04-locate-dialog');
    await $(testId('locate-open')).click();
    await $(testId('locate-report-title')).waitForDisplayed({ timeout: 20_000, timeoutMsg: 'the report never showed' });
    await shot('05-locate-report');
    await expect($(testId('locate-report-title'))).toHaveText('PlayStation firmware');
    expect(await textOf('locate-report-lede')).toBe(`4 files from ${picks()}. Installed into DuckStation’s folders.`);
    expect(await reportItems('ok')).toEqual([
      'scph5501 (1).bin → scph5501.bin | Size and MD5 match; installed under the name DuckStation looks for',
      'scph5500.bin from bios-pack.zip | Size and MD5 match',
    ]);
    expect(await reportItems('bad')).toEqual([
      "scph5502.bin | This isn't the scph5502.bin GRID knows (size or MD5 differs)",
      "readme.txt | GRID doesn't know this file for PlayStation",
    ]);
    expect(readFileSync(bios('scph5501.bin'), 'utf-8')).toBe(US);
    expect(readFileSync(bios('scph5500.bin'), 'utf-8')).toBe(JP);
    expect(existsSync(bios('scph5502.bin'))).toBe(false);
    await $(testId('locate-done')).click();
    await $(testId('locate-dialog')).waitForExist({ reverse: true, timeout: TRANSITION_TIMEOUT });
    await browser.waitUntil(async () => (await textOf('sys-fw-count-psx')) === '3 files · 2 installed', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the firmware block never marked the files installed',
    });
    expect(await $(testId('sys-fw-file-psx-scph5501.bin')).getAttribute('data-state')).toBe('verified');
    expect(await $(testId('sys-fw-file-psx-scph5500.bin')).getAttribute('data-state')).toBe('verified');
    // One BIOS of the group is in place: nothing more is needed.
    await browser.waitUntil(async () => (await statusOf('psx')) === 'ready', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `PlayStation never read Ready (it reads ${await statusOf('psx')})`,
    });
    await shot('06-firmware-installed');
  });

  it('Use the whole folder counts the files that match nothing', async () => {
    await openLocate();
    await $(testId('locate-folder')).click();
    await $(testId('locate-report-title')).waitForDisplayed({ timeout: 20_000 });
    expect(await textOf('locate-report-lede')).toBe(`Searched ${picks()} and the folders in it. Installed into DuckStation’s folders.`);
    expect(await reportItems('ok')).toHaveLength(2);
    expect(await reportItems('bad')).toEqual(["scph5502.bin | This isn't the scph5502.bin GRID knows (size or MD5 differs)"]);
    await expect($(testId('locate-report-unmatched'))).toHaveText('2 other files in the folder matched nothing.');
    await shot('07-locate-folder-report');
    await $(testId('locate-done')).click();
  });
});
