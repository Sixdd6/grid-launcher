//! SQLite installed-games registry. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! (SQLite registry) for the schema and identity rules this implements.

use super::LibraryError;
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
    installed_at        INTEGER NOT NULL,
    UNIQUE (title_key, platform_key)
);
";

/// The schema version this build understands. Bumped when a migration adds
/// columns (see spec: later milestones add native/PS3/PS4 fields).
const LATEST_USER_VERSION: i64 = 1;

/// Every column of `installed_games`, in the order selected/inserted below.
const SELECT_COLUMNS: &str = "title, platform, rom_id, rom_file_name, archive_path, \
     extracted_path, extracted_dir, multi_file_game_dir, description, rating, genres, \
     regions, languages, tags, revision, companies, first_release_date, filesize_bytes, \
     server_updated_at, installed_at";

/// One installed game, as persisted in the SQLite registry. `title_key` and
/// `platform_key` are not part of this type: they are computed from `title`
/// and `platform` at write and lookup time (`value.trim().to_lowercase()`),
/// never stored differently from that derivation.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
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
        let conn = Connection::open(path).map_err(registry_err)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(registry_err)?;
        match version {
            0 => {
                conn.execute_batch(SCHEMA_SQL).map_err(registry_err)?;
                conn.pragma_update(None, "user_version", LATEST_USER_VERSION)
                    .map_err(registry_err)?;
            }
            v if v == LATEST_USER_VERSION => {}
            v => {
                return Err(LibraryError::Registry(format!(
                    "this database (user_version {v}) is from a newer app version; \
                     update the app to open it"
                )));
            }
        }
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

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO installed_games (
                title, platform, title_key, platform_key, rom_id, rom_file_name,
                archive_path, extracted_path, extracted_dir, multi_file_game_dir,
                description, rating, genres, regions, languages, tags, revision,
                companies, first_release_date, filesize_bytes, server_updated_at,
                installed_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22
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
                installed_at = excluded.installed_at",
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
            ],
        )
        .map_err(registry_err)?;
        Ok(())
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
    /// `None`), falls back to the `(title_key, platform_key)` identity.
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
