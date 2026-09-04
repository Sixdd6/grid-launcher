//! SQLite installed-games registry. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! (SQLite registry) for the schema and identity rules this implements.

use super::LibraryError;
use crate::images::ImageFields;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_SQL: &str = "
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
    UNIQUE (title_key, platform_key)
);
";

/// The schema version this build understands. Bumped when a migration adds
/// columns (see spec: later milestones add native/PS3/PS4 fields).
const LATEST_USER_VERSION: i64 = 4;

/// The columns v1 -> v2 (milestone 7) adds to `installed_games`.
const V2_IMAGE_COLUMNS: [&str; 3] = ["cover_small_path", "cover_large_path", "screenshot_urls"];

/// The columns v2 -> v3 (native/PS3/PS4/RetroAchievements install fields)
/// adds to `installed_games`.
const V3_COLUMNS: [&str; 12] = [
    "native_executable_path",
    "native_launch_parameters",
    "native_compat_tool",
    "native_wineprefix",
    "native_game_dir",
    "included_dlc",
    "ps3_trophy_paths",
    "ps3_game_id",
    "ps3_iso_path",
    "ps4_game_id",
    "ps4_content",
    "ra_id",
];

/// The column v3 -> v4 (the redesign's Library rail) adds. An INTEGER, not
/// a TEXT like every earlier migration's columns, so it gets its own
/// `ADD COLUMN` type rather than joining a loop over string columns.
const V4_COLUMN: &str = "last_played_at";

/// The column names `installed_games` currently has.
fn installed_games_columns(conn: &Connection) -> Result<Vec<String>, LibraryError> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(installed_games)")
        .map_err(registry_err)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(registry_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(registry_err)
}

/// v1 -> v2 (milestone 7): adds the three image columns.
///
/// The `ALTER TABLE`s and the `user_version` bump run inside ONE
/// transaction, so a failure part way through leaves the database exactly as
/// it was — still at version 1, with the columns it had. The earlier version
/// of this migration ran the three `ALTER`s in autocommit and wrote
/// `user_version` only afterwards: an interruption between two of them left a
/// half-migrated database that stayed at version 1 and failed every later
/// open with "duplicate column name", with no way out but deleting the file.
///
/// Each `ADD COLUMN` is also skipped when `PRAGMA table_info` already lists
/// the column. That makes the migration idempotent, so a database already
/// torn by the old code opens and finishes migrating instead of bricking.
fn migrate_1_to_2(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    for column in V2_IMAGE_COLUMNS {
        if existing.iter().any(|name| name == column) {
            continue;
        }
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {column} TEXT NOT NULL DEFAULT '';"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 2)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}

/// v2 -> v3 (milestone 8): adds the twelve native/PS3/PS4/RetroAchievements
/// install columns. Same transaction + idempotent-`ADD COLUMN` shape as
/// [`migrate_1_to_2`], for the same reason: one commit for the schema change
/// and the `user_version` bump, and a column already present (a database torn
/// by an earlier, non-transactional version of a migration) is skipped rather
/// than erroring.
fn migrate_2_to_3(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    for column in V3_COLUMNS {
        if existing.iter().any(|name| name == column) {
            continue;
        }
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {column} TEXT NOT NULL DEFAULT '';"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 3)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}

/// v3 -> v4 (desktop UI redesign 2): adds `last_played_at`, the epoch
/// seconds of the last launch, `0` for a game never launched through GRID.
/// The Library rail's "Recent" entry and the "Recently played" sort are its
/// only readers; nothing else in the app depends on it, so a database that
/// cannot be migrated would be a far worse outcome than a column of zeroes.
///
/// Same transaction + idempotent-`ADD COLUMN` shape as [`migrate_1_to_2`]
/// and [`migrate_2_to_3`], for the same reasons.
fn migrate_3_to_4(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    if !existing.iter().any(|name| name == V4_COLUMN) {
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {V4_COLUMN} INTEGER NOT NULL DEFAULT 0;"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 4)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}

/// Every column of `installed_games`, in the order selected/inserted below.
const SELECT_COLUMNS: &str = "title, platform, rom_id, rom_file_name, archive_path, \
     extracted_path, extracted_dir, multi_file_game_dir, description, rating, genres, \
     regions, languages, tags, revision, companies, first_release_date, filesize_bytes, \
     server_updated_at, installed_at, cover_small_path, cover_large_path, screenshot_urls, \
     native_executable_path, native_launch_parameters, native_compat_tool, native_wineprefix, \
     native_game_dir, included_dlc, ps3_trophy_paths, ps3_game_id, ps3_iso_path, ps4_game_id, \
     ps4_content, ra_id, last_played_at";

/// One installed game, as persisted in the SQLite registry. `title_key` and
/// `platform_key` are not part of this type: they are computed from `title`
/// and `platform` at write and lookup time (`value.trim().to_lowercase()`),
/// never stored differently from that derivation.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct InstalledGame {
    pub title: String,
    pub platform: String,
    pub rom_id: Option<i64>,
    pub rom_file_name: String,
    pub archive_path: String,
    pub extracted_path: String,
    pub extracted_dir: String,
    pub multi_file_game_dir: String,
    pub description: String,
    pub rating: String,
    pub genres: String,
    pub regions: String,
    pub languages: String,
    pub tags: String,
    pub revision: String,
    pub companies: String,
    pub first_release_date: String,
    pub filesize_bytes: i64,
    pub server_updated_at: String,
    pub installed_at: i64,
    pub cover_small_path: String,
    pub cover_large_path: String,
    pub screenshot_urls: String,
    #[serde(default)]
    pub native_executable_path: String,
    #[serde(default)]
    pub native_launch_parameters: String,
    #[serde(default)]
    pub native_compat_tool: String,
    #[serde(default)]
    pub native_wineprefix: String,
    #[serde(default)]
    pub native_game_dir: String,
    #[serde(default)]
    pub included_dlc: String,
    #[serde(default)]
    pub ps3_trophy_paths: String,
    #[serde(default)]
    pub ps3_game_id: String,
    #[serde(default)]
    pub ps3_iso_path: String,
    #[serde(default)]
    pub ps4_game_id: String,
    #[serde(default)]
    pub ps4_content: String,
    #[serde(default)]
    pub ra_id: String,
    /// Epoch seconds of the last launch, `0` when the game has never been
    /// launched through GRID. Written ONLY by
    /// [`Registry::touch_last_played`] — never by [`Registry::upsert`], so
    /// an update or a reinstall keeps the history the Library rail reads.
    #[serde(default)]
    pub last_played_at: i64,
}

impl InstalledGame {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            title: row.get(0)?,
            platform: row.get(1)?,
            rom_id: row.get(2)?,
            rom_file_name: row.get(3)?,
            archive_path: row.get(4)?,
            extracted_path: row.get(5)?,
            extracted_dir: row.get(6)?,
            multi_file_game_dir: row.get(7)?,
            description: row.get(8)?,
            rating: row.get(9)?,
            genres: row.get(10)?,
            regions: row.get(11)?,
            languages: row.get(12)?,
            tags: row.get(13)?,
            revision: row.get(14)?,
            companies: row.get(15)?,
            first_release_date: row.get(16)?,
            filesize_bytes: row.get(17)?,
            server_updated_at: row.get(18)?,
            installed_at: row.get(19)?,
            cover_small_path: row.get(20)?,
            cover_large_path: row.get(21)?,
            screenshot_urls: row.get(22)?,
            native_executable_path: row.get(23)?,
            native_launch_parameters: row.get(24)?,
            native_compat_tool: row.get(25)?,
            native_wineprefix: row.get(26)?,
            native_game_dir: row.get(27)?,
            included_dlc: row.get(28)?,
            ps3_trophy_paths: row.get(29)?,
            ps3_game_id: row.get(30)?,
            ps3_iso_path: row.get(31)?,
            ps4_game_id: row.get(32)?,
            ps4_content: row.get(33)?,
            ra_id: row.get(34)?,
            last_played_at: row.get(35)?,
        })
    }
}

fn identity_key(value: &str) -> String {
    value.trim().to_lowercase()
}

/// Whether `row` — a hit from [`Registry::find`] — really is the install for
/// `rom_id`. `find`'s title/platform fallback can hand back a row for a
/// *different* game that merely shares a title and platform; this is the one
/// place that rule is enforced, so every caller (already-installed check,
/// uninstall, and the frontend's mirrored `matchesInstalled`) agrees:
///
/// - `row.rom_id` is `Some(other)` and `other != rom_id`: not a match, no
///   identity rescue — a different game must never be reported as installed.
/// - `row.rom_id` is `Some(rom_id)`: a match.
/// - `row.rom_id` is `None`: the row predates rom-id tracking, so the
///   title/platform identity `find` already matched on is accepted.
pub fn installed_match(row: &InstalledGame, rom_id: i64) -> bool {
    match row.rom_id {
        Some(other) => other == rom_id,
        None => true,
    }
}

fn registry_err(e: rusqlite::Error) -> LibraryError {
    LibraryError::Registry(e.to_string())
}

/// The SQLite-backed installed-games registry. Holds one connection behind a
/// mutex (rusqlite connections are not `Sync`); every method takes `&self`.
pub struct Registry {
    conn: Mutex<Connection>,
}

impl Registry {
    /// Opens (creating if absent) the registry at `path`, running the schema
    /// migration on a fresh database. Errors if the database's
    /// `PRAGMA user_version` is newer than this build understands.
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path).map_err(registry_err)?;
        let mut version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(registry_err)?;
        if version > LATEST_USER_VERSION {
            return Err(LibraryError::Registry(format!(
                "this database (user_version {version}) is from a newer app version; \
                 update the app to open it"
            )));
        }
        if version == 0 {
            conn.execute_batch(SCHEMA_SQL).map_err(registry_err)?;
            version = LATEST_USER_VERSION;
        }
        // Each step commits its own `user_version` bump with its own schema
        // change, so an interrupted upgrade never leaves the database at a
        // version that does not describe its schema.
        while version < LATEST_USER_VERSION {
            match version {
                1 => migrate_1_to_2(&mut conn)?,
                2 => migrate_2_to_3(&mut conn)?,
                3 => migrate_3_to_4(&mut conn)?,
                v => {
                    return Err(LibraryError::Registry(format!(
                        "no migration from user_version {v}"
                    )))
                }
            }
            version += 1;
        }
        conn.pragma_update(None, "user_version", LATEST_USER_VERSION)
            .map_err(registry_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Inserts or replaces the row identified by `(title_key, platform_key)`
    /// with every field from `rec`. When `rec.extracted_path` is non-empty,
    /// `archive_path` is stored as `""` regardless of `rec.archive_path`
    /// (the two are mutually exclusive on disk).
    pub fn upsert(&self, rec: &InstalledGame) -> Result<(), LibraryError> {
        let title_key = identity_key(&rec.title);
        let platform_key = identity_key(&rec.platform);
        let archive_path: &str = if rec.extracted_path.is_empty() {
            &rec.archive_path
        } else {
            ""
        };

        let ps3_game_id = rec.ps3_game_id.to_uppercase();
        let ps4_game_id = rec.ps4_game_id.to_uppercase();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO installed_games (
                title, platform, title_key, platform_key, rom_id, rom_file_name,
                archive_path, extracted_path, extracted_dir, multi_file_game_dir,
                description, rating, genres, regions, languages, tags, revision,
                companies, first_release_date, filesize_bytes, server_updated_at,
                installed_at, cover_small_path, cover_large_path, screenshot_urls,
                native_executable_path, native_launch_parameters, native_compat_tool,
                native_wineprefix, native_game_dir, included_dlc, ps3_trophy_paths,
                ps3_game_id, ps3_iso_path, ps4_game_id, ps4_content, ra_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37
            )
            ON CONFLICT(title_key, platform_key) DO UPDATE SET
                title = excluded.title,
                platform = excluded.platform,
                rom_id = excluded.rom_id,
                rom_file_name = excluded.rom_file_name,
                archive_path = excluded.archive_path,
                extracted_path = excluded.extracted_path,
                extracted_dir = excluded.extracted_dir,
                multi_file_game_dir = excluded.multi_file_game_dir,
                description = excluded.description,
                rating = excluded.rating,
                genres = excluded.genres,
                regions = excluded.regions,
                languages = excluded.languages,
                tags = excluded.tags,
                revision = excluded.revision,
                companies = excluded.companies,
                first_release_date = excluded.first_release_date,
                filesize_bytes = excluded.filesize_bytes,
                server_updated_at = excluded.server_updated_at,
                installed_at = excluded.installed_at,
                cover_small_path = excluded.cover_small_path,
                cover_large_path = excluded.cover_large_path,
                screenshot_urls = excluded.screenshot_urls,
                native_executable_path = excluded.native_executable_path,
                native_launch_parameters = excluded.native_launch_parameters,
                native_compat_tool = excluded.native_compat_tool,
                native_wineprefix = excluded.native_wineprefix,
                native_game_dir = excluded.native_game_dir,
                included_dlc = excluded.included_dlc,
                ps3_trophy_paths = excluded.ps3_trophy_paths,
                ps3_game_id = excluded.ps3_game_id,
                ps3_iso_path = excluded.ps3_iso_path,
                ps4_game_id = excluded.ps4_game_id,
                ps4_content = excluded.ps4_content,
                ra_id = excluded.ra_id",
            params![
                rec.title,
                rec.platform,
                title_key,
                platform_key,
                rec.rom_id,
                rec.rom_file_name,
                archive_path,
                rec.extracted_path,
                rec.extracted_dir,
                rec.multi_file_game_dir,
                rec.description,
                rec.rating,
                rec.genres,
                rec.regions,
                rec.languages,
                rec.tags,
                rec.revision,
                rec.companies,
                rec.first_release_date,
                rec.filesize_bytes,
                rec.server_updated_at,
                rec.installed_at,
                rec.cover_small_path,
                rec.cover_large_path,
                rec.screenshot_urls,
                rec.native_executable_path,
                rec.native_launch_parameters,
                rec.native_compat_tool,
                rec.native_wineprefix,
                rec.native_game_dir,
                rec.included_dlc,
                rec.ps3_trophy_paths,
                ps3_game_id,
                rec.ps3_iso_path,
                ps4_game_id,
                rec.ps4_content,
                rec.ra_id,
            ],
        )
        .map_err(registry_err)?;
        Ok(())
    }

    /// Sets the three image columns on the row for `rom_id`. Returns whether
    /// a row matched.
    pub fn update_images(&self, rom_id: i64, fields: &ImageFields) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET cover_small_path = ?1, cover_large_path = ?2, \
                 screenshot_urls = ?3 WHERE rom_id = ?4",
                params![
                    fields.cover_small_path,
                    fields.cover_large_path,
                    fields.screenshot_urls,
                    rom_id
                ],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }

    /// Stamps `last_played_at` on the row for `rom_id`. Returns whether a
    /// row matched — a launch of something not in the registry (there is no
    /// such path today, but `launch_game` does not require one) stamps
    /// nothing and reports `false`.
    pub fn touch_last_played(&self, rom_id: i64, at: i64) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET last_played_at = ?1 WHERE rom_id = ?2",
                params![at, rom_id],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }

    /// Sets the native-launch columns on the row for `rom_id`. Returns
    /// whether a row matched. The caller re-registers a full record through
    /// [`Registry::upsert`] for anything beyond these three fields.
    pub fn update_native_settings(
        &self,
        rom_id: i64,
        executable: &str,
        parameters: &str,
        compat_tool: &str,
    ) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET native_executable_path = ?1, \
                 native_launch_parameters = ?2, native_compat_tool = ?3 WHERE rom_id = ?4",
                params![executable, parameters, compat_tool, rom_id],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }

    /// Sets the PS4 title-id and content-manifest columns on the row for
    /// `rom_id`. Returns whether a row matched.
    pub fn update_ps4_content(
        &self,
        rom_id: i64,
        game_id: &str,
        content_json: &str,
    ) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET ps4_game_id = ?1, ps4_content = ?2 \
                 WHERE rom_id = ?3",
                params![game_id, content_json, rom_id],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }

    /// All installed games, ordered by `title_key`.
    pub fn all(&self) -> Result<Vec<InstalledGame>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("SELECT {SELECT_COLUMNS} FROM installed_games ORDER BY title_key");
        let mut stmt = conn.prepare(&sql).map_err(registry_err)?;
        let rows = stmt
            .query_map([], InstalledGame::from_row)
            .map_err(registry_err)?;
        let mut games = Vec::new();
        for row in rows {
            games.push(row.map_err(registry_err)?);
        }
        Ok(games)
    }

    /// Looks up an installed game. When `rom_id` is `Some`, a row with a
    /// matching `rom_id` is tried first; if none matches (or `rom_id` is
    /// `None`), falls back to the `(title_key, platform_key)` identity —
    /// except when `title.trim()` is empty, in which case the fallback is
    /// skipped entirely and this returns `None`. A blank title has no real
    /// identity to rescue by, so it must never match a blank-titled,
    /// null-rom_id row.
    pub fn find(
        &self,
        rom_id: Option<i64>,
        title: &str,
        platform: &str,
    ) -> Result<Option<InstalledGame>, LibraryError> {
        let conn = self.conn.lock().unwrap();

        if let Some(id) = rom_id {
            let sql = format!("SELECT {SELECT_COLUMNS} FROM installed_games WHERE rom_id = ?1");
            let found = conn
                .query_row(&sql, params![id], InstalledGame::from_row)
                .optional()
                .map_err(registry_err)?;
            if found.is_some() {
                return Ok(found);
            }
        }

        if title.trim().is_empty() {
            return Ok(None);
        }

        let title_key = identity_key(title);
        let platform_key = identity_key(platform);
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM installed_games \
             WHERE title_key = ?1 AND platform_key = ?2"
        );
        conn.query_row(
            &sql,
            params![title_key, platform_key],
            InstalledGame::from_row,
        )
        .optional()
        .map_err(registry_err)
    }

    /// Removes the row for `(title, platform)`'s identity key. Returns
    /// whether a row was removed.
    pub fn remove(&self, title: &str, platform: &str) -> Result<bool, LibraryError> {
        let title_key = identity_key(title);
        let platform_key = identity_key(platform);
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "DELETE FROM installed_games WHERE title_key = ?1 AND platform_key = ?2",
                params![title_key, platform_key],
            )
            .map_err(registry_err)?;
        Ok(affected > 0)
    }
}
