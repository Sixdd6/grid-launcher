import { readFileSync } from 'node:fs';
import { APP_START_TIMEOUT, configPath, TRANSITION_TIMEOUT } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** The `[ui] rail_closed` list as written, on one line (`rail_closed =
 *  ["romm", "drives"]`; the writer breaks a longer list over lines), or null
 *  when the key is absent. */
function railClosedLine(): string | null {
  const m = /^rail_closed = \[([^\]]*)\]/m.exec(readFileSync(configPath(), 'utf-8'));
  if (!m) return null;
  const ids = m[1]
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s !== '');
  return `rail_closed = [${ids.join(', ')}]`;
}

async function waitConfig(want: string | null): Promise<void> {
  await browser.waitUntil(async () => railClosedLine() === want, {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `config.toml never had ${want ?? 'no rail_closed'} (has ${railClosedLine()})`,
  });
}

async function expanded(group: string): Promise<string | null> {
  return $(testId(`library-rail-group-${group}`)).getAttribute('aria-expanded');
}

/**
 * Stage `rail-fold`, part A (RommStorefront README § Library rail › Folding
 * groups), a desktop start on the tv-library profile: RomM not connected
 * with three installed RomM games, and the TESTDRIVE Backup drive. Every
 * group starts open; RomM folds from its heading, keeps its flag, shows its
 * Installed count and holds the selection; Enter and Space toggle Backup
 * drives; each toggle is written to `[ui] rail_closed`. Ends with RomM
 * closed, for part B's restart.
 */
describe('rail-fold (a): folding the Library rail on the desktop', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the Library never appeared' });
    await $(testId('library-rail-group-drives')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no Backup drives group' });
  });

  it('starts with every group open, in its own panel, sentence-case headings and no counts', async () => {
    expect(await expanded('drives')).toBe('true');
    expect(await expanded('romm')).toBe('true');
    expect((await $(testId('library-rail-group-drives')).getText()).trim()).toBe('Backup drives');
    expect(await $(testId('library-rail-group-romm')).getText()).toMatch(/^\s*R?\s*RomM\s*off\s*$/);
    await expect($(testId('library-rail-group-romm-count'))).not.toExist();
    await expect($(testId('library-rail-romm-installed'))).toExist();
    expect(railClosedLine()).toBeNull();
  });

  it('closing RomM while RomM › Installed shows keeps the view; the heading holds it, with the flag and the count', async () => {
    await $(testId('library-rail-romm-installed')).click();
    await expect($(testId('library-rail-romm-installed'))).toHaveAttribute('aria-current', 'page');
    await $(testId('library-rail-group-romm')).click();
    await browser.waitUntil(async () => (await expanded('romm')) === 'false', { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'RomM never closed' });
    await expect($(testId('library-rail-romm-installed'))).not.toExist();
    await expect($(testId('library-rail-romm-installed-flag'))).toHaveText('off');
    await expect($(testId('library-rail-group-romm-count'))).toHaveText('3');
    await expect($(testId('library-rail-group-romm-count'))).toHaveAttribute('aria-label', '3 installed');
    await expect($(testId('library-rail-group-romm'))).toHaveAttribute('title', 'Showing RomM › Installed');
    expect(await browser.execute(() => document.querySelector('[data-rail-group="romm"]')?.classList.contains('holds'))).toBe(true);
    // The grid still shows RomM › Installed.
    await expect($(testId('library-head'))).toHaveText(expect.stringContaining('Installed'));
    await waitConfig('rail_closed = ["romm"]');
  });

  it('Enter and Space on a heading toggle it, focus staying on the heading', async () => {
    await browser.execute(() => (document.querySelector('[data-testid="library-rail-group-drives"]') as HTMLElement).focus());
    await browser.keys('Enter');
    await browser.waitUntil(async () => (await expanded('drives')) === 'false', { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Enter never closed Backup drives' });
    await expect($(testId('library-rail-group-drives-count'))).toHaveAttribute('aria-label', '1 drive');
    await waitConfig('rail_closed = ["romm", "drives"]');
    expect(await browser.execute(() => document.activeElement?.getAttribute('data-testid'))).toBe('library-rail-group-drives');
    await browser.keys(' ');
    await browser.waitUntil(async () => (await expanded('drives')) === 'true', { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'Space never opened Backup drives' });
    await expect($(testId('library-rail-group-drives-count'))).not.toExist();
    expect(await browser.execute(() => document.activeElement?.getAttribute('data-testid'))).toBe('library-rail-group-drives');
    await waitConfig('rail_closed = ["romm"]');
  });

  it('stays closed across a view switch', async () => {
    await $(testId('nav-downloads')).click();
    await $(testId('nav-library')).click();
    await $(testId('library-rail')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    expect(await expanded('romm')).toBe('false');
    expect(await expanded('drives')).toBe('true');
  });
});
