import { readFileSync } from 'node:fs';
import { APP_START_TIMEOUT, configPath, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { waitTab } from '../helpers/tv.js';
import { tap } from '../helpers/tvBackups.js';

const testId = (id: string) => `[data-testid="${id}"]`;

function railClosedLine(): string | null {
  return /^rail_closed = .*$/m.exec(readFileSync(configPath(), 'utf-8'))?.[0] ?? null;
}

async function tvRailKeys(): Promise<string[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="tv-library"] [data-rail] [data-k]')).map((e) => e.getAttribute('data-k') ?? ''),
  )) as string[];
}

/**
 * Stage `rail-fold`, part B: a relaunch after part A closed RomM. The
 * desktop rail opens with RomM still closed (`[ui] rail_closed`), and TV
 * mode, entered from the TV button, shows the same folder closed with its
 * Installed count (one list for both modes). A on the folder in TV mode
 * opens it, in both modes, and the list is written back.
 */
describe('rail-fold (b): remembered across a restart and in TV mode', () => {
  before(async () => {
    await $(testId('library-rail')).waitForDisplayed({ timeout: APP_START_TIMEOUT, timeoutMsg: 'the Library never appeared' });
    await $(testId('library-rail-group-romm')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no RomM group' });
  });

  it('opens with RomM still closed and Backup drives open', async () => {
    expect(railClosedLine()).toBe('rail_closed = ["romm"]');
    await browser.waitUntil(async () => (await $(testId('library-rail-group-romm')).getAttribute('aria-expanded')) === 'false', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'RomM came back open after the restart',
    });
    await expect($(testId('library-rail-romm-installed'))).not.toExist();
    await expect($(testId('library-rail-group-romm-count'))).toHaveText('3');
    expect(await $(testId('library-rail-group-drives')).getAttribute('aria-expanded')).toBe('true');
    // Startup never opens a group, and All games holds the selection.
    await expect($(testId('library-rail-all'))).toHaveAttribute('aria-current', 'page');
  });

  it('TV mode shows the same folder closed, with its count', async () => {
    await $(testId('tv-mode-button')).click();
    await $(testId('tv-shell')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'TV mode never opened' });
    await waitTab('library');
    await $(testId('tv-lib-fold-romm')).waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'no RomM folder in TV mode' });
    await expect($(testId('tv-lib-fold-romm'))).toHaveAttribute('aria-expanded', 'false');
    await expect($(testId('tv-lib-fold-romm-count'))).toHaveText('3');
    await expect($(testId('tv-lib-fold-romm-count'))).toHaveAttribute('aria-label', '3 installed');
    expect(await tvRailKeys()).not.toContain('r-romm:installed');
    await expect($(testId('tv-lib-fold-drives'))).toHaveAttribute('aria-expanded', 'true');
    // Each folder sits in its own panel.
    expect(await browser.execute(() => Array.from(document.querySelectorAll('[data-testid="tv-library"] .rgrp')).map((g) => g.getAttribute('data-grp')))).toEqual([
      'drives',
      'romm',
    ]);
  });

  it('A on the folder opens it, and the list is written back', async () => {
    await tap(testId('tv-lib-fold-romm'));
    await browser.waitUntil(async () => (await tvRailKeys()).includes('r-romm:installed'), { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'A never opened RomM' });
    await expect($(testId('tv-lib-fold-romm'))).toHaveAttribute('aria-expanded', 'true');
    await expect($(testId('tv-lib-fold-romm-count'))).not.toExist();
    await browser.waitUntil(async () => railClosedLine() === null, { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'rail_closed never emptied' });
  });
});
