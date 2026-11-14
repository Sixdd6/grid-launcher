#!/usr/bin/env node
/**
 * Seeds a `steam*` stage group's data dir before each spec (scripts/e2e.sh
 * runs `node steam-seed.mjs <data-dir> <spec name> [--nokey|--private|--noclient]`).
 * Every spec starts from nothing, on a profile used without a server
 * (`offline = true`), with the fake Steam (docs/stores-phase4-plan.md
 * package C "E2E harness"):
 *
 * - `<data>/steam/root`: the Steam root (GRID_LAUNCHER_E2E_STEAM_ROOT, e2e.sh
 *   exports it as E2E_STEAM_ROOT), copied from e2e/fixtures-steam/root:
 *   `steamapps/libraryfolders.vdf` naming two libraries (the root and
 *   `<data>/steam/library2`), Moonlit Ferry installed in the root (played
 *   two hours ago), Proton 9.0, Steam Linux Runtime 3.0 and Steamworks
 *   Common Redistributables beside it (tools that must never show), and
 *   `config/loginusers.vdf` with SamPlays (…0001, signed in now) and
 *   couchguest (…0002), each with the AccountName
 *   FAKE-ACCOUNT-NAME-not-shown, which must never show;
 * - `<data>/steam/library2`: Deep Field Radio (played a week ago) and Old
 *   Brick Road (never played);
 * - `<data>/steam/registry.vdf` (`<root>/../registry.vdf`): RunningAppID 0,
 *   and AutoLoginUser FAKE-ACCOUNT-NAME-not-shown.
 * Each game folder under `steamapps/common/` holds one file, so a spec can
 * check that GRID never touched them.
 *
 * Profiles:
 * - `steam-connect`, `steam-secrets`: Steam not connected (the specs
 *   connect through `steam_save`).
 * - every other Steam spec (package D's `steam-library`, `tv-steam`):
 *   connected as SamPlays (`[stores.steam]` in config.toml). The key is NOT
 *   written here: e2e.sh empties the run keyring's Steam slot before each
 *   spec, and a spec that needs the key calls `ensureSteamKey()`
 *   (e2e/helpers/steamSetup.ts), which saves it through the app's own
 *   `steam_save`, so no seed writes a secret to a file.
 *   `--private` (or a spec name ending in `-private`): connected as
 *   couchguest (…0002), whose game details are private.
 *   `--noclient` (or `-noclient`): no Steam root at all (Steam isn't
 *   installed).
 *   `--nokey` (or `-nokey`): the same files; it only documents that the
 *   spec never calls `ensureSteamKey()`.
 */
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dataDir = process.argv[2];
const spec = process.argv[3] ?? '';
const flag = (name) => process.argv.includes(`--${name}`) || spec.endsWith(`-${name}`);
if (!dataDir) {
  console.error('usage: steam-seed.mjs <data-dir> [spec] [--nokey|--private|--noclient]');
  process.exit(1);
}

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.join(here, '..', 'fixtures-steam');
export const SAM = '76561198000000001';
export const GUEST = '76561198000000002';

rmSync(dataDir, { recursive: true, force: true });
const library = path.join(dataDir, 'library');
mkdirSync(library, { recursive: true });
const tomlString = (s) => JSON.stringify(s);

const steam = path.join(dataDir, 'steam');
const root = path.join(steam, 'root');
const lib2 = path.join(steam, 'library2');
const now = Math.floor(Date.now() / 1000);
const fill = (text) =>
  text
    .replaceAll('{ROOT}', root)
    .replaceAll('{LIB2}', lib2)
    .replaceAll('{TODAY}', String(now - 2 * 3600))
    .replaceAll('{LAST_WEEK}', String(now - 7 * 86400));

/** Copies the template, filling the placeholders in its text files. */
function copyFilled(from, to) {
  cpSync(from, to, { recursive: true });
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      const full = path.join(dir, name);
      if (statSync(full).isDirectory()) walk(full);
      else if (/\.(vdf|acf)$/.test(name)) writeFileSync(full, fill(readFileSync(full, 'utf8')));
    }
  };
  if (statSync(to).isDirectory()) walk(to);
  else writeFileSync(to, fill(readFileSync(to, 'utf8')));
}

if (!flag('noclient')) {
  mkdirSync(steam, { recursive: true });
  copyFilled(path.join(fixtures, 'root'), root);
  copyFilled(path.join(fixtures, 'library2'), lib2);
  copyFilled(path.join(fixtures, 'registry.vdf'), path.join(steam, 'registry.vdf'));
  // The game folders, one file each.
  for (const lib of [root, lib2]) {
    const apps = path.join(lib, 'steamapps');
    for (const name of readdirSync(apps).filter((n) => n.startsWith('appmanifest_'))) {
      const dir = /"installdir"\s+"([^"]*)"/.exec(readFileSync(path.join(apps, name), 'utf8'))?.[1];
      if (!dir) continue;
      mkdirSync(path.join(apps, 'common', dir), { recursive: true });
      writeFileSync(path.join(apps, 'common', dir, 'game.bin'), `fake ${dir}\n`);
    }
  }
}

const connected = spec !== 'steam-connect' && spec !== 'steam-secrets';
const id = flag('private') ? GUEST : SAM;
const persona = flag('private') ? 'couchguest' : 'SamPlays';
writeFileSync(
  path.join(dataDir, 'config.toml'),
  `schema_version = 1
library_path = ${tomlString(library)}
library_layout_version = 2
offline = true
${
  connected
    ? `
[stores.steam]
connected = true
steam_id = ${tomlString(id)}
persona = ${tomlString(persona)}
`
    : ''
}`,
);
