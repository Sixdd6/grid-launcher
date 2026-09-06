/**
 * The CURRENT `grid-launcher.db` schema, for seed scripts that need a
 * registry the running app will accept as already up to date.
 *
 * `SCHEMA_SQL` is copied verbatim from `SCHEMA_SQL` in
 * `crates/grid-core/src/library/registry.rs`, and `USER_VERSION` matches that
 * file's `LATEST_USER_VERSION`. This pairing is load-bearing:
 * `Registry::open` only runs its own `CREATE TABLE` when `PRAGMA
 * user_version` is 0, and it trusts a database already at
 * `LATEST_USER_VERSION` to have the right shape — so a stale copy here
 * silently diverges from what the app expects instead of failing loudly.
 * Keep both in sync with registry.rs whenever a migration lands.
 *
 * Deliberately NOT retrofitted onto `launch-seed.mjs`, `cloud-saves-seed.mjs`
 * or `images-seed.mjs`: those seed v1 databases on purpose (images-seed
 * exercises the v1 -> v2 -> v3 migration path), and rewriting them to v6
 * would delete that coverage.
 *
 * A row inserted through `writeRegistry` gets `images_version = 0`, which the
 * replenish pass treats as "written before the current image rules" and
 * re-fetches once. Seed `images_version = 1` on a row whose spec must see NO
 * re-fetch.
 */

import { execFileSync } from 'node:child_process';

export const USER_VERSION = 6;

export const SCHEMA_SQL = `
CREATE TABLE installed_games (
    id                  INTEGER PRIMARY KEY,
    title               TEXT NOT NULL,
    platform            TEXT NOT NULL,
    title_key           TEXT NOT NULL,
    platform_key        TEXT NOT NULL,
    rom_id              INTEGER,
    rom_file_name       TEXT NOT NULL DEFAULT '',
    archive_path        TEXT NOT NULL DEFAULT '',
    extracted_path      TEXT NOT NULL DEFAULT '',
    extracted_dir       TEXT NOT NULL DEFAULT '',
    multi_file_game_dir TEXT NOT NULL DEFAULT '',
    description         TEXT NOT NULL DEFAULT '',
    rating              TEXT NOT NULL DEFAULT '',
    genres              TEXT NOT NULL DEFAULT '',
    regions             TEXT NOT NULL DEFAULT '',
    languages           TEXT NOT NULL DEFAULT '',
    tags                TEXT NOT NULL DEFAULT '',
    revision            TEXT NOT NULL DEFAULT '',
    companies           TEXT NOT NULL DEFAULT '',
    first_release_date  TEXT NOT NULL DEFAULT '',
    filesize_bytes      INTEGER NOT NULL DEFAULT 0,
    server_updated_at   TEXT NOT NULL DEFAULT '',
    cover_small_path    TEXT NOT NULL DEFAULT '',
    cover_large_path    TEXT NOT NULL DEFAULT '',
    screenshot_urls     TEXT NOT NULL DEFAULT '',
    fanart_urls         TEXT NOT NULL DEFAULT '',
    native_executable_path   TEXT NOT NULL DEFAULT '',
    native_launch_parameters TEXT NOT NULL DEFAULT '',
    native_compat_tool       TEXT NOT NULL DEFAULT '',
    native_wineprefix        TEXT NOT NULL DEFAULT '',
    native_game_dir          TEXT NOT NULL DEFAULT '',
    included_dlc             TEXT NOT NULL DEFAULT '',
    ps3_trophy_paths         TEXT NOT NULL DEFAULT '',
    ps3_game_id              TEXT NOT NULL DEFAULT '',
    ps3_iso_path             TEXT NOT NULL DEFAULT '',
    ps4_game_id              TEXT NOT NULL DEFAULT '',
    ps4_content              TEXT NOT NULL DEFAULT '',
    ra_id                    TEXT NOT NULL DEFAULT '',
    installed_at        INTEGER NOT NULL,
    last_played_at      INTEGER NOT NULL DEFAULT 0,
    images_version      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (title_key, platform_key)
);
PRAGMA user_version = ${USER_VERSION};
`;

/**
 * Creates `dbPath` with the current schema and no rows (plus any `extraSql`
 * the caller appends), using the `sqlite3` CLI the runner already requires
 * as a prerequisite (see `rewrite/scripts/e2e.sh`).
 */
export function writeRegistry(dbPath, extraSql = '') {
  execFileSync('sqlite3', [dbPath], {
    input: `${SCHEMA_SQL}${extraSql}`,
    stdio: ['pipe', 'inherit', 'inherit'],
  });
}

/** Minimal TOML basic-string escaping — enough for the paths seeds write. */
export function tomlString(value) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}
