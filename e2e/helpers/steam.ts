/**
 * Shared steps for the Steam specs (GameStores and TVStores cards, Phase 4
 * package D). They run against package C's harness (docs/stores-phase4-plan.md,
 * "C — E2E harness"): the mock Steam Web API (`e2e/mock-romm/mock-steam.mjs`),
 * the fake `steam` client (`e2e/fake-tools/steam.mjs`, through
 * `GRID_LAUNCHER_E2E_STEAM_EXE`), which records its argv and acts on the link
 * in the fake Steam root (`e2e/fixtures-steam/`, through
 * `GRID_LAUNCHER_E2E_STEAM_ROOT`), and the seed (`e2e/seed/steam-seed.mjs`:
 * connected as SamPlays with the key in the run's keyring; Moonlit Ferry,
 * Deep Field Radio and Old Brick Road installed; Quiet Comet, Harbor of Bells,
 * Rime Assembly and Signal Lost owned and not installed).
 *
 * App ids are package C's choice, so the specs find covers by title.
 */
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { dataDir, TRANSITION_TIMEOUT } from './env.js';
import { closeWithEscape, coverTags, detailsLine, startApp } from './epic.js';
import { testId, testIdPrefix } from './gog.js';

export { closeWithEscape, coverTags, detailsLine, startApp, testId, testIdPrefix };

/** The titles and account the seed gives (the prototype's `stores.js` s1–s7, less Signal Lost's install). */
export const STEAM = {
  installed: ['Deep Field Radio', 'Moonlit Ferry', 'Old Brick Road'],
  available: ['Harbor of Bells', 'Quiet Comet', 'Rime Assembly', 'Signal Lost'],
  /** Installed in Steam itself (the fake root's manifests GRID's watcher never saw). */
  outside: 'Deep Field Radio',
  persona: 'SamPlays',
  steamId: '76561198000000001',
  /** The guest account in `loginusers.vdf`; the mock answers `{}` for it (private). */
  guestId: '76561198000000002',
  /** The mock's only good key: a fixture marker, never a real key. */
  key: 'FAKE-STEAM-KEY-not-real',
  revokedKey: 'FAKE-STEAM-KEY-not-real-revoked',
  /** The tool rows the fake root holds and GRID must never show. */
  tools: ['Proton 9.0', 'Steam Linux Runtime 3.0', 'Steamworks Common Redistributables'],
  noBackup: 'Steam games can’t be backed up here. Steam has its own Backup and restore.',
  /** The account names in `loginusers.vdf`, which GRID never reads or shows. */
  accountName: 'FAKE-ACCOUNT-NAME-not-shown',
} as const;

/** Opens Library › Steam › <part>. */
export async function openSteam(part: 'installed' | 'available'): Promise<void> {
  const pill = await $('button=Library');
  if (await pill.isExisting()) await pill.click();
  const row = await $(testId(`library-rail-store-steam-${part}`));
  await row.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `Library › Steam › ${part} never appeared in the rail` });
  await row.click();
  await expect($(testId('library-title'))).toHaveText(expect.stringContaining('Steam'));
}

/** The Steam covers on screen, in grid order. */
export function steamCards() {
  return $$(testIdPrefix('library-store-card-steam-'));
}

/** Every Steam cover's title, in grid order. */
export async function steamTitles(): Promise<string[]> {
  const out: string[] = [];
  for (const card of await steamCards()) out.push((await card.$('.label').getText()).trim());
  return out;
}

/** The cover with `title` and its badge (`steam-<appid>`). Waits for it. */
export async function steamCard(title: string, timeout = TRANSITION_TIMEOUT): Promise<{ card: WebdriverIO.Element; badge: string }> {
  let found: WebdriverIO.Element | null = null;
  await browser.waitUntil(
    async () => {
      for (const card of await steamCards()) {
        if ((await card.$('.label').getText()).trim() === title) {
          found = card;
          return true;
        }
      }
      return false;
    },
    { timeout, timeoutMsg: `no Steam cover titled ${title}` },
  );
  const card = found as unknown as WebdriverIO.Element;
  const badge = ((await card.getAttribute('data-testid')) ?? '').replace('library-store-card-', '');
  return { card, badge };
}

/** Opens Details for the cover with `title`; returns its badge. */
export async function openSteamDetails(title: string): Promise<string> {
  const { card, badge } = await steamCard(title);
  await card.click();
  await $(testId('store-details')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  return badge;
}

/** The game key a badge names (`steam-620` → `steam:620`). */
export function keyOfBadge(badge: string): string {
  return badge.replace(/^steam-/, 'steam:');
}

// ── The fake client and the fake Steam root ─────────────────────────────

/** Where the fake `steam` records each argv (one line per launch). */
export function argvFile(): string {
  return process.env.E2E_STEAM_ARGV_FILE ?? path.join(dataDir(), 'steam-argv.log');
}

/**
 * Every argv the fake `steam` was started with, oldest first, each joined
 * by spaces (the fake writes one JSON array per line).
 */
export function steamArgv(): string[] {
  const f = argvFile();
  if (!existsSync(f)) return [];
  return readFileSync(f, 'utf8')
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l !== '')
    .map((l) => {
      try {
        return (JSON.parse(l) as string[]).join(' ');
      } catch {
        return l;
      }
    });
}

/** Waits until the fake `steam` was started with an argv matching `re`; answers it. */
export async function waitArgv(re: RegExp, timeout = TRANSITION_TIMEOUT): Promise<string> {
  let hit = '';
  await browser.waitUntil(
    async () => {
      hit = steamArgv().find((l) => re.test(l)) ?? '';
      return hit !== '';
    },
    { timeout, timeoutMsg: `the fake steam never ran with ${re} (argv: ${steamArgv().join(' | ')})` },
  );
  return hit;
}

/** This stage's fake Steam root (the copy of `e2e/fixtures-steam/` the app reads). */
export function steamRoot(): string {
  const root = process.env.E2E_STEAM_ROOT ?? process.env.GRID_LAUNCHER_E2E_STEAM_ROOT;
  if (!root || root.trim() === '') {
    throw new Error('E2E_STEAM_ROOT is not set. Run the steam stages through scripts/e2e.sh.');
  }
  return root;
}

/**
 * Drops a fully installed app manifest into the fake root's first library,
 * as Steam itself does for a game installed in Steam (GRID shows it within
 * its 15 s background scan).
 */
export function dropManifest(appid: number, name: string, sizeBytes = 1024 ** 3): void {
  const dir = path.join(steamRoot(), 'steamapps');
  mkdirSync(path.join(dir, 'common', name.replace(/[^A-Za-z0-9]+/g, '')), { recursive: true });
  const acf = [
    '"AppState"',
    '{',
    `\t"appid"\t\t"${appid}"`,
    '\t"Universe"\t\t"1"',
    `\t"name"\t\t"${name}"`,
    '\t"StateFlags"\t\t"4"',
    `\t"installdir"\t\t"${name.replace(/[^A-Za-z0-9]+/g, '')}"`,
    `\t"LastUpdated"\t\t"${Math.floor(Date.now() / 1000)}"`,
    `\t"SizeOnDisk"\t\t"${sizeBytes}"`,
    '\t"buildid"\t\t"1"',
    '\t"BytesToDownload"\t\t"0"',
    '\t"BytesDownloaded"\t\t"0"',
    '}',
    '',
  ].join('\n');
  writeFileSync(path.join(dir, `appmanifest_${appid}.acf`), acf);
}

/**
 * Puts a marker file in the fake root: `.fake-install-cancel` (Steam's
 * dialog closed, no manifest ever), `.fake-install-abort` (cancelled in
 * Steam halfway: the manifest goes), `.fake-no-run`.
 */
export function setMarker(name: '.fake-install-cancel' | '.fake-install-abort' | '.fake-no-run', on = true): void {
  const f = path.join(steamRoot(), name);
  if (on) writeFileSync(f, '');
  else rmSync(f, { force: true });
}

// ── Settings › Stores › Steam ───────────────────────────────────────────

/**
 * Fills the Steam card's key field without the key crossing the WebDriver
 * wire as text (wdio logs every command's arguments; helpers/steamSetup.ts):
 * it goes reversed and the page turns it back. The card reads the field
 * once, when Test or Save is pressed.
 */
export async function fillKey(value: string): Promise<void> {
  const change = $(testId('steam-key-change'));
  if (await change.isExisting()) await change.click();
  const field = $(testId('steam-key-field'));
  await field.waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
  await browser.execute(
    (id: string, reversed: string) => {
      const el = document.querySelector<HTMLInputElement>(`[data-testid="${id}"]`);
      if (!el) return;
      el.value = [...reversed].reverse().join('');
      el.dispatchEvent(new Event('input', { bubbles: true }));
    },
    'steam-key-field',
    [...value].reverse().join(''),
  );
}

/** Presses Test (or Save) on the Steam card and answers the result line once "Asking Steam…" is gone. */
export async function steamCardAnswer(button: 'steam-test' | 'steam-save'): Promise<string> {
  await $(testId(button)).click();
  await browser.waitUntil(async () => !(await $(testId('steam-test-busy')).isExisting()), {
    timeout: TRANSITION_TIMEOUT * 2,
    timeoutMsg: 'Asking Steam… never ended',
  });
  const line = $(testId('steam-test-result'));
  return (await line.isExisting()) ? (await line.getText()).trim() : '';
}
