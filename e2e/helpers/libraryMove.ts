/**
 * Shared by the `library-move` stage group's specs (9b). The names and
 * folders match `e2e/seed/library-move-seed.mjs`; keep both in step.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from './env.js';

export const testId = (id: string) => `[data-testid="${id}"]`;

export const PLATFORM = 'Super Nintendo Entertainment System';
export const TITLE = 'Super Mario World';
export const PCSX2 = 'PCSX2 (Playstation 2)';
export const XEMU = 'Xemu (Xbox)';
export const MOVED_DIRS = ['games', 'emulators', 'saves'] as const;

/**
 * Upper bound for a whole library move of the seeded few-file library:
 * Copy, verify, Commit (with an emulator resync per entry) and Cleanup.
 * The work itself takes well under a second; the rest is margin for a slow
 * CI disk and the WebDriver round trips while polling.
 */
export const MOVE_TIMEOUT = 30_000;

export const argvFile = () => path.join(dataDir(), 'library-move-argv.log');
export const romPath = (root: string) => path.join(root, 'games', PLATFORM, TITLE, 'game.sfc');
export const pcsx2Ini = (root: string) => path.join(root, 'saves', PCSX2, 'inis', 'PCSX2.ini');
export const xemuToml = (root: string) => path.join(root, 'emulators', XEMU, 'xemu.toml');
export const journalFile = () => path.join(dataDir(), 'library-move.json');

/**
 * Every spelling a rewritten value may use for `root`: as typed, and as the
 * filesystem resolves it (files that stored a canonical path get the new
 * root in canonical form).
 */
export function rootForms(root: string): string[] {
  const forms = [root];
  try {
    const resolved = realpathSync(root);
    if (resolved !== root) forms.push(resolved);
  } catch {
    // Not there: only the typed form.
  }
  return forms;
}

export const readConfig = () => readFileSync(configPath(), 'utf-8');

/** One column of the single seeded registry row, read with the sqlite3 CLI. */
export function registryValue(column: 'extracted_path' | 'extracted_dir'): string {
  return execFileSync(
    'sqlite3',
    [path.join(dataDir(), 'grid-launcher.db'), `select ${column} from installed_games`],
    { encoding: 'utf-8' },
  ).trim();
}

/** The app is on Connect or already in the shell; ends in the shell. */
export async function reachShell(): Promise<void> {
  const form = $(testId('connect-server-url'));
  const shell = $(testId('platform-btn-1'));
  await browser.waitUntil(async () => (await form.isExisting()) || (await shell.isExisting()), {
    timeout: APP_START_TIMEOUT,
    timeoutMsg: 'neither the connect form nor the library appeared',
  });
  if (await form.isExisting()) {
    await form.setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
  }
  await shell.waitForExist({
    timeout: TRANSITION_TIMEOUT * 2,
    timeoutMsg: 'the library never rendered a platform button after connecting',
  });
}

/** The `Library move` row's detail line on the Downloads view, `null` when there is none. */
async function moveRowDetail(): Promise<string | null> {
  return browser.execute(() => {
    const kinds = document.querySelectorAll('[data-testid^="download-kind-"]');
    for (const kind of Array.from(kinds)) {
      if (kind.textContent?.trim() !== 'Library move') continue;
      const id = kind.getAttribute('data-testid')!.slice('download-kind-'.length);
      const detail = document.querySelector(`[data-testid="download-detail-${id}"]`);
      return detail?.textContent?.trim() ?? '';
    }
    return null;
  });
}

/**
 * Opens Downloads and waits for the `Library move` row to read Completed.
 * A row that reads Failed or Cancelled fails at once, with its text.
 */
export async function waitForMoveCompleted(): Promise<void> {
  await $(testId('nav-downloads')).click();
  await $(testId('downloads-view')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the downloads view never rendered',
  });
  const deadline = Date.now() + MOVE_TIMEOUT;
  let last: string | null = null;
  while (Date.now() < deadline) {
    last = await moveRowDetail();
    if (last !== null && last.startsWith('Completed')) return;
    if (last !== null && (last.startsWith('Failed') || last.startsWith('Cancelled'))) {
      throw new Error(`the library move row ended as: ${last}`);
    }
    await browser.pause(250);
  }
  throw new Error(`the library move row never completed (last detail: ${last ?? 'no row'})`);
}

/** The relative user-data link resolves to the folder under `root/saves/`. */
export function expectSavesLink(root: string, name: 'inis' | 'memcards') {
  const link = path.join(root, 'emulators', PCSX2, name);
  expect(lstatSync(link).isSymbolicLink()).toBe(true);
  expect(realpathSync(link)).toBe(realpathSync(path.join(root, 'saves', PCSX2, name)));
}

/** The files moved: every library folder under `newRoot`, none left at `oldRoot`. */
export function expectTreeMoved(oldRoot: string, newRoot: string) {
  for (const dir of MOVED_DIRS) {
    expect(existsSync(path.join(newRoot, dir))).toBe(true);
    expect(existsSync(path.join(oldRoot, dir))).toBe(false);
  }
  // Whole: part B seeds a cut-off copy of this file in staging.
  expect(readFileSync(romPath(newRoot), 'utf-8')).toBe('fake snes rom bytes\n');
  expect(readFileSync(path.join(newRoot, 'saves', PCSX2, 'memcards', 'Mcd001.ps2'), 'utf-8')).toBe(
    'MEMCARD1\n',
  );
  // The staging folder is gone, and the unrelated file stayed behind.
  expect(existsSync(path.join(newRoot, '.grid-library-move'))).toBe(false);
  expect(readFileSync(path.join(oldRoot, 'notes.txt'), 'utf-8')).toBe('not part of the library\n');
  expect(existsSync(path.join(newRoot, 'notes.txt'))).toBe(false);
}

/** config.toml stores `newRoot`, records `oldRoot` as former, keeps layout 2. */
export function expectConfigSwitched(oldRoot: string, newRoot: string) {
  const config = readConfig();
  expect(config).toContain(`library_path = ${JSON.stringify(newRoot)}`);
  expect(config).toContain('former_library_paths');
  expect(config).toContain(JSON.stringify(oldRoot));
  expect(config).toContain('library_layout_version = 2');
  // Every emulator entry follows the files.
  for (const file of [
    path.join('Stub Runner', 'stub-runner.sh'),
    path.join(PCSX2, 'pcsx2-qt'),
    path.join(XEMU, 'xemu'),
  ]) {
    expect(config).toContain(JSON.stringify(path.join(newRoot, 'emulators', file)));
    expect(config).not.toContain(JSON.stringify(path.join(oldRoot, 'emulators', file)));
  }
}

/** The registry row points into `newRoot`. */
export function expectRegistryMoved(newRoot: string) {
  expect(registryValue('extracted_path')).toBe(romPath(newRoot));
  expect(registryValue('extracted_dir')).toBe(path.dirname(romPath(newRoot)));
}

/**
 * The emulator files that store absolute library paths now name `newRoot`
 * (in one of its spellings) and no longer name `oldRoot`.
 */
export function expectEmulatorFilesRewritten(oldRoot: string, newRoot: string) {
  const ini = readFileSync(pcsx2Ini(newRoot), 'utf-8');
  const bios = rootForms(newRoot).map((r) => `Bios = ${path.join(r, 'saves', PCSX2, 'bios')}`);
  expect(bios.some((line) => ini.includes(line))).toBe(true);
  expect(ini).not.toContain(`${oldRoot}/`);

  const toml = readFileSync(xemuToml(newRoot), 'utf-8');
  const hdd = rootForms(newRoot).map(
    (r) => `hdd_path = '${path.join(r, 'saves', XEMU, 'xbox_hdd.qcow2')}'`,
  );
  const bootrom = rootForms(newRoot).map(
    (r) => `bootrom_path = '${path.join(r, 'emulators', XEMU, 'mcpx_1.0.bin')}'`,
  );
  expect(hdd.some((line) => toml.includes(line))).toBe(true);
  expect(bootrom.some((line) => toml.includes(line))).toBe(true);
  expect(toml).not.toContain(`${oldRoot}/`);
}
