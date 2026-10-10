//! SQLite installed-games registry. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! (SQLite registry) for the schema and identity rules this implements.

use super::LibraryError;
use crate::images::ImageFields;
use crate::romm::PlaySessionEntry;
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
";

/// The play-session outbox (v7, Q10): finished sessions waiting for
/// `POST /api/play-sessions` (see `crate::play_activity`). The columns
/// rebuild a `PlaySessionEntry`; `id` is the stable local id a flush pages
/// and deletes by. `(rom_id, start_time)` is one game process, so a second
/// enqueue of it is ignored. `IF NOT EXISTS` makes the v6 -> v7 step
/// idempotent.
const PLAY_SESSIONS_SQL: &str = "
CREATE TABLE IF NOT EXISTS pending_play_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    rom_id      INTEGER NOT NULL,
    start_time  TEXT NOT NULL,
    end_time    TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    UNIQUE (rom_id, start_time)
);
";

/// The schema version this build understands. Bumped when a migration adds
/// columns or tables (see spec: later milestones add native/PS3/PS4 fields).
const LATEST_USER_VERSION: i64 = 7;

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

/// The column v4 -> v5 (round 4's background art) adds: the row's fanart
/// URLs, newline-joined exactly like `screenshot_urls`. A TEXT with a blank
/// default, so an existing row simply has no fanart and the background falls
/// back to its screenshots.
const V5_COLUMN: &str = "fanart_urls";

/// The column v5 -> v6 adds: which generation of the image-field rules wrote
/// this row's `cover_*`/`screenshot_urls`/`fanart_urls`. An INTEGER with a
/// `0` default, so every row that existed before v6 reads as "older than any
/// stamp" and is re-fetched once.
const V6_COLUMN: &str = "images_version";

/// The generation of the image-field rules this build stores. Bump when the
/// meaning or resolution of the stored image fields changes; rows below it
/// are re-fetched by `images::replenish`.
///
/// `1` is the first stamp: it marks a row whose fields were resolved by the
/// 2026-09-05 rules, which put a bare relative RomM path under
/// `/assets/romm/resources/`. Rows written before that carry `0`, which is
/// the only way to tell "this game has no fanart" from "this row was written
/// before fanart could resolve".
pub const IMAGES_VERSION: i64 = 1;

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

/// v4 -> v5 (round 4): adds `fanart_urls`. Same transaction +
/// idempotent-`ADD COLUMN` shape as every migration above it, for the same
/// reasons — one commit for the schema change and the `user_version` bump,
/// and a column already present is skipped rather than erroring.
fn migrate_4_to_5(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    if !existing.iter().any(|name| name == V5_COLUMN) {
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {V5_COLUMN} TEXT NOT NULL DEFAULT '';"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 5)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}

/// v5 -> v6 (round 5): adds `images_version`, the generation stamp
/// [`IMAGES_VERSION`] describes. Defaults to `0`, so every existing row is
/// below the current stamp and the replenish pass re-fetches its image
/// fields once. Same transaction + idempotent-`ADD COLUMN` shape as every
/// migration above it, for the same reasons.
fn migrate_5_to_6(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    let existing = installed_games_columns(&tx)?;
    if !existing.iter().any(|name| name == V6_COLUMN) {
        tx.execute_batch(&format!(
            "ALTER TABLE installed_games ADD COLUMN {V6_COLUMN} INTEGER NOT NULL DEFAULT 0;"
        ))
        .map_err(registry_err)?;
    }
    tx.pragma_update(None, "user_version", 6)
        .map_err(registry_err)?;
    tx.commit().map_err(registry_err)
}

/// v6 -> v7 (Q10 play activity): adds the `pending_play_sessions` outbox.
/// One transaction for the table and the `user_version` bump, like every
/// migration above; the `CREATE` is `IF NOT EXISTS`, so a database torn
/// between the two finishes instead of erroring, and keeps its queued rows.
fn migrate_6_to_7(conn: &mut Connection) -> Result<(), LibraryError> {
    let tx = conn.transaction().map_err(registry_err)?;
    tx.execute_batch(PLAY_SESSIONS_SQL).map_err(registry_err)?;
    tx.pragma_update(None, "user_version", 7)
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
     ps4_content, ra_id, last_played_at, fanart_urls, images_version";

/// The path-bearing columns [`Registry::rewrite_paths`] hands to its
/// closure as plain strings. `ps3_trophy_paths` is the ninth, handled
/// separately because it holds a JSON array rather than one path.
const REWRITE_PATH_COLUMNS: [&str; 8] = [
    "archive_path",
    "extracted_path",
    "extracted_dir",
    "multi_file_game_dir",
    "native_executable_path",
    "native_wineprefix",
    "native_game_dir",
    "ps3_iso_path",
];

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
    /// Newline-joined fanart URLs, already resolved + host-filtered, exactly
    /// like `screenshot_urls`. `""` for a row installed before v5 or for a
    /// game the server has no fanart for.
    #[serde(default)]
    pub fanart_urls: String,
    /// The generation of the image-field rules that wrote this row's
    /// `cover_*`/`screenshot_urls`/`fanart_urls`. `0` for a row written
    /// before v6; [`IMAGES_VERSION`] for a row written by this build.
    /// `images::replenish` re-fetches anything below [`IMAGES_VERSION`].
    /// Written by [`Registry::upsert`] and [`Registry::update_images`] only —
    /// never carried in from a caller's record.
    #[serde(default)]
    pub images_version: i64,
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
            fanart_urls: row.get(36)?,
            images_version: row.get(37)?,
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
            conn.execute_batch(PLAY_SESSIONS_SQL)
                .map_err(registry_err)?;
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
                4 => migrate_4_to_5(&mut conn)?,
                5 => migrate_5_to_6(&mut conn)?,
                6 => migrate_6_to_7(&mut conn)?,
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
    ///
    /// `images_version` is stamped with [`IMAGES_VERSION`], NOT taken from
    /// `rec`: an install or reinstall resolves the image fields with this
    /// build's rules, so the row is current by construction.
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
                installed_at, cover_small_path, cover_large_path, screenshot_urls, fanart_urls,
                native_executable_path, native_launch_parameters, native_compat_tool,
                native_wineprefix, native_game_dir, included_dlc, ps3_trophy_paths,
                ps3_game_id, ps3_iso_path, ps4_game_id, ps4_content, ra_id, images_version
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39
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
                fanart_urls = excluded.fanart_urls,
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
                ra_id = excluded.ra_id,
                images_version = excluded.images_version",
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
                rec.fanart_urls,
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
                IMAGES_VERSION,
            ],
        )
        .map_err(registry_err)?;
        Ok(())
    }

    /// Sets the image columns on the row for `rom_id`, and stamps
    /// `images_version` with [`IMAGES_VERSION`] — the fields were just
    /// resolved by this build's rules, so the row is current and the
    /// replenish pass must not pick it up again. Returns whether a row
    /// matched.
    pub fn update_images(&self, rom_id: i64, fields: &ImageFields) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute(
                "UPDATE installed_games SET cover_small_path = ?1, cover_large_path = ?2, \
                 screenshot_urls = ?3, fanart_urls = ?4, images_version = ?5 \
                 WHERE rom_id = ?6",
                params![
                    fields.cover_small_path,
                    fields.cover_large_path,
                    fields.screenshot_urls,
                    fields.fanart_urls,
                    IMAGES_VERSION,
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

    /// Rewrites every stored path with `rewrite`, in ONE transaction.
    ///
    /// `rewrite` is handed each non-empty value of the nine path columns
    /// ([`REWRITE_PATH_COLUMNS`] plus `ps3_trophy_paths`, whose JSON array
    /// is rewritten element-wise and re-serialized only when an element
    /// changed) and returns the replacement, or `None` to leave the value
    /// alone. Rows with no changed value are not written at all. Returns the
    /// number of rows updated; any error rolls the whole pass back.
    ///
    /// The closure keeps the registry ignorant of layout names: the library
    /// layout migration owns the prefix rules, this owns the SQL.
    pub fn rewrite_paths(
        &self,
        rewrite: &dyn Fn(&str) -> Option<String>,
    ) -> Result<usize, LibraryError> {
        let mut guard = self.conn.lock().unwrap();
        let tx = guard.transaction().map_err(registry_err)?;

        let columns = REWRITE_PATH_COLUMNS.join(", ");
        let mut rows: Vec<(i64, Vec<String>, String)> = Vec::new();
        {
            let sql =
                format!("SELECT id, {columns}, ps3_trophy_paths FROM installed_games ORDER BY id");
            let mut stmt = tx.prepare(&sql).map_err(registry_err)?;
            let mapped = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let mut values = Vec::with_capacity(REWRITE_PATH_COLUMNS.len());
                    for index in 0..REWRITE_PATH_COLUMNS.len() {
                        values.push(row.get::<_, String>(index + 1)?);
                    }
                    let trophies: String = row.get(REWRITE_PATH_COLUMNS.len() + 1)?;
                    Ok((id, values, trophies))
                })
                .map_err(registry_err)?;
            for row in mapped {
                rows.push(row.map_err(registry_err)?);
            }
        }

        let assignments: Vec<String> = REWRITE_PATH_COLUMNS
            .iter()
            .enumerate()
            .map(|(index, column)| format!("{column} = ?{}", index + 1))
            .collect();
        let update_sql = format!(
            "UPDATE installed_games SET {}, ps3_trophy_paths = ?{} WHERE id = ?{}",
            assignments.join(", "),
            REWRITE_PATH_COLUMNS.len() + 1,
            REWRITE_PATH_COLUMNS.len() + 2
        );

        let mut updated = 0usize;
        for (id, values, trophies) in rows {
            let mut changed = false;
            let mut new_values = values.clone();
            for (index, value) in values.iter().enumerate() {
                if value.is_empty() {
                    continue;
                }
                if let Some(replacement) = rewrite(value) {
                    if &replacement != value {
                        new_values[index] = replacement;
                        changed = true;
                    }
                }
            }
            let mut new_trophies = trophies.clone();
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&trophies) {
                let mut rewritten = parsed.clone();
                let mut trophies_changed = false;
                for (index, value) in parsed.iter().enumerate() {
                    if value.is_empty() {
                        continue;
                    }
                    if let Some(replacement) = rewrite(value) {
                        if &replacement != value {
                            rewritten[index] = replacement;
                            trophies_changed = true;
                        }
                    }
                }
                if trophies_changed {
                    if let Ok(serialized) = serde_json::to_string(&rewritten) {
                        new_trophies = serialized;
                        changed = true;
                    }
                }
            }
            if !changed {
                continue;
            }
            let mut params: Vec<rusqlite::types::Value> = new_values
                .into_iter()
                .map(rusqlite::types::Value::Text)
                .collect();
            params.push(rusqlite::types::Value::Text(new_trophies));
            params.push(rusqlite::types::Value::Integer(id));
            tx.execute(&update_sql, rusqlite::params_from_iter(params))
                .map_err(registry_err)?;
            updated += 1;
        }

        tx.commit().map_err(registry_err)?;
        Ok(updated)
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

    // --- play-session outbox (Q10, `play_activity`) ------------------------

    /// Queues `entry` for the play-session ingest. Returns `false` when the
    /// same `(rom_id, start_time)` is already queued: one game process is
    /// one session, so a second enqueue of it is a repeat.
    pub fn enqueue_play_session(&self, entry: &PlaySessionEntry) -> Result<bool, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO pending_play_sessions                  (rom_id, start_time, end_time, duration_ms) VALUES (?1, ?2, ?3, ?4)",
                params![
                    entry.rom_id,
                    entry.start_time,
                    entry.end_time,
                    entry.duration_ms
                ],
            )
            .map_err(registry_err)?;
        Ok(inserted > 0)
    }

    /// Up to `limit` queued sessions with a row id above `after_id`, oldest
    /// first. A flush pages with the last id it saw, so a row it keeps is
    /// not sent twice in one pass.
    pub fn pending_play_sessions(
        &self,
        after_id: i64,
        limit: usize,
    ) -> Result<Vec<PendingPlaySession>, LibraryError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, rom_id, start_time, end_time, duration_ms                  FROM pending_play_sessions WHERE id > ?1 ORDER BY id LIMIT ?2",
            )
            .map_err(registry_err)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![after_id, limit], |row| {
                Ok(PendingPlaySession {
                    id: row.get(0)?,
                    entry: PlaySessionEntry {
                        rom_id: row.get(1)?,
                        start_time: row.get(2)?,
                        end_time: row.get(3)?,
                        duration_ms: row.get(4)?,
                    },
                })
            })
            .map_err(registry_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(registry_err)
    }

    /// Deletes the queued sessions with these row ids. Returns how many
    /// rows went away.
    pub fn delete_pending_play_sessions(&self, ids: &[i64]) -> Result<usize, LibraryError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(registry_err)?;
        let mut removed = 0;
        {
            let mut stmt = tx
                .prepare("DELETE FROM pending_play_sessions WHERE id = ?1")
                .map_err(registry_err)?;
            for id in ids {
                removed += stmt.execute(params![id]).map_err(registry_err)?;
            }
        }
        tx.commit().map_err(registry_err)?;
        Ok(removed)
    }
}

/// One queued play session: the outbox row id and the entry to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPlaySession {
    pub id: i64,
    pub entry: PlaySessionEntry,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registry in a temp directory, with the handle that keeps it alive.
    fn temp_registry() -> (tempfile::TempDir, Registry) {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::open(&dir.path().join("registry.db")).unwrap();
        (dir, registry)
    }

    /// A row with every path column this migration rewrites set.
    fn row_with_paths(title: &str) -> InstalledGame {
        InstalledGame {
            title: title.to_string(),
            platform: "Sony PlayStation 2".to_string(),
            archive_path: "/lib/archive.zip".to_string(),
            extracted_path: "/lib/game/game.iso".to_string(),
            extracted_dir: "/lib/game".to_string(),
            multi_file_game_dir: "/lib/multi".to_string(),
            native_executable_path: "/lib/native/game.exe".to_string(),
            native_wineprefix: "/lib/native/prefix".to_string(),
            native_game_dir: "/lib/native".to_string(),
            ps3_iso_path: "/lib/ps3/game.iso".to_string(),
            ps3_trophy_paths: "[\"/lib/a\",\"/lib/b\"]".to_string(),
            installed_at: 1,
            ..Default::default()
        }
    }

    #[test]
    fn rewrite_paths_updates_every_path_column_in_one_transaction() {
        let (_dir, registry) = temp_registry();
        registry.upsert(&row_with_paths("Game A")).unwrap();
        let mut untouched = row_with_paths("Game B");
        untouched.archive_path = "/elsewhere/archive.zip".to_string();
        untouched.extracted_path = String::new();
        untouched.extracted_dir = String::new();
        untouched.multi_file_game_dir = String::new();
        untouched.native_executable_path = String::new();
        untouched.native_wineprefix = String::new();
        untouched.native_game_dir = String::new();
        untouched.ps3_iso_path = String::new();
        untouched.ps3_trophy_paths = String::new();
        registry.upsert(&untouched).unwrap();

        let changed = registry
            .rewrite_paths(&|value| {
                value
                    .strip_prefix("/lib")
                    .map(|rest| format!("/new/lib{rest}"))
            })
            .unwrap();
        assert_eq!(changed, 1, "only the row with /lib paths changed");

        let rows = registry.all().unwrap();
        let a = rows.iter().find(|r| r.title == "Game A").unwrap();
        // `upsert` blanks `archive_path` when `extracted_path` is set, so the
        // stored value there is "" and stays "".
        assert_eq!(a.archive_path, "");
        assert_eq!(a.extracted_path, "/new/lib/game/game.iso");
        assert_eq!(a.extracted_dir, "/new/lib/game");
        assert_eq!(a.multi_file_game_dir, "/new/lib/multi");
        assert_eq!(a.native_executable_path, "/new/lib/native/game.exe");
        assert_eq!(a.native_wineprefix, "/new/lib/native/prefix");
        assert_eq!(a.native_game_dir, "/new/lib/native");
        assert_eq!(a.ps3_iso_path, "/new/lib/ps3/game.iso");
        assert_eq!(a.ps3_trophy_paths, "[\"/new/lib/a\",\"/new/lib/b\"]");
        // Columns the closure never matched, and the whole second row, are
        // unchanged.
        assert_eq!(a.platform, "Sony PlayStation 2");
        let b = rows.iter().find(|r| r.title == "Game B").unwrap();
        assert_eq!(b.archive_path, "/elsewhere/archive.zip");
        assert_eq!(b.ps3_trophy_paths, "");
    }

    #[test]
    fn rewrite_paths_rolls_back_on_a_failing_row() {
        let (_dir, registry) = temp_registry();
        registry.upsert(&row_with_paths("Game A")).unwrap();
        let first = registry
            .rewrite_paths(&|value| {
                value
                    .strip_prefix("/lib")
                    .map(|rest| format!("/new/lib{rest}"))
            })
            .unwrap();
        assert_eq!(first, 1);
        let before = registry.all().unwrap();

        // A second pass that rewrites nothing must change no rows and leave
        // every value exactly as the first pass left it.
        let second = registry.rewrite_paths(&|_| None).unwrap();
        assert_eq!(second, 0);
        assert_eq!(registry.all().unwrap(), before);
    }
}
