import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir } from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `layout-migration`: the one-shot startup move from the flat, pre-v1
 * library root to `games/` / `emulators/` / `saves/`
 * (grid-core/src/library/layout_migration.rs, run from
 * `app/src-tauri/src/lib.rs` before any service is built).
 *
 * `e2e/seed/layout-migration-seed.mjs` writes a LEGACY library: a flat
 * platform directory, a tagged `Emulators/<Name>-latest/` install with a
 * real `memcards/` inside it, a registry row pointing into that tree, and a
 * config with NO `library_layout_version`. Everything asserted here happened
 * before the window existed, so the spec only waits for the app to render
 * and then reads the filesystem, the config and the database.
 *
 * It deliberately never connects: the migration does not need a session, and
 * a connect would only add ways for the stage to go red for an unrelated
 * reason.
 */
describe('layout-migration', () => {
  const library = () => path.join(dataDir(), 'library');
  const EMULATOR_NAME = 'PCSX2 (Playstation 2)';
  const PLATFORM = 'Super Nintendo Entertainment System';
  const installDir = () => path.join(library(), 'emulators', EMULATOR_NAME);
  const savesDir = () => path.join(library(), 'saves', EMULATOR_NAME);

  before(async () => {
    // The migration is finished before the webview exists; waiting for the
    // connect form is just "the app reached a usable state".
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
  });

  it('moves the flat platform directory under games/', () => {
    expect(existsSync(path.join(library(), 'games', PLATFORM, 'Game A', 'game.sfc'))).toBe(true);
    expect(existsSync(path.join(library(), PLATFORM))).toBe(false);
  });

  it('renames the tagged install directory to the untagged profile name', () => {
    expect(existsSync(path.join(installDir(), 'pcsx2-qt'))).toBe(true);
    expect(existsSync(path.join(library(), 'Emulators'))).toBe(false);
  });

  it('links the emulator user data into saves/', () => {
    const linked = path.join(installDir(), 'memcards');
    expect(lstatSync(linked).isSymbolicLink()).toBe(true);
    expect(realpathSync(linked)).toBe(realpathSync(path.join(savesDir(), 'memcards')));
    // The file itself survived the move and is readable through both paths.
    expect(readFileSync(path.join(linked, 'slot1.mcd'), 'utf-8')).toBe('MEMCARD1\n');
    expect(readFileSync(path.join(savesDir(), 'memcards', 'slot1.mcd'), 'utf-8')).toBe('MEMCARD1\n');
  });

  it('stamps the version and rewrites the emulator path in the config', () => {
    const config = readFileSync(configPath(), 'utf-8');
    expect(config).toContain('library_layout_version = 2');
    expect(config).toContain(path.join(installDir(), 'pcsx2-qt'));
  });

  it('rewrites the registry row onto the games/ path', () => {
    // Read with the sqlite3 CLI the seeds already depend on, rather than
    // adding a node sqlite dependency to the e2e package.
    const value = execFileSync(
      'sqlite3',
      [path.join(dataDir(), 'grid-launcher.db'), 'select extracted_path from installed_games'],
      { encoding: 'utf-8' },
    ).trim();
    expect(value).toBe(path.join(library(), 'games', PLATFORM, 'Game A', 'game.sfc'));
  });

  it('shows no failure toast', async () => {
    const toasts = await $$(testId('toast'));
    const texts: string[] = [];
    for (const toast of toasts) {
      texts.push(await toast.getText());
    }
    expect(texts.some((text) => text.includes('Library reorganization'))).toBe(false);
  });
});
