use grid_core::images::ImageFields;
use grid_core::library::registry::{InstalledGame, Registry};
use rusqlite::Connection;

fn sample(title: &str, platform: &str) -> InstalledGame {
    InstalledGame {
        title: title.to_string(),
        platform: platform.to_string(),
        rom_id: Some(42),
        rom_file_name: "Game.zip".into(),
        archive_path: "/library/Platform/Game.zip".into(),
        extracted_path: String::new(),
        extracted_dir: String::new(),
        multi_file_game_dir: String::new(),
        description: "A game.".into(),
        rating: "4.5".into(),
        genres: "Action,Adventure".into(),
        regions: "US".into(),
        languages: "en".into(),
        tags: "favorite".into(),
        revision: "1.1".into(),
        companies: "Acme".into(),
        first_release_date: "1998-11-23".into(),
        filesize_bytes: 123_456,
        server_updated_at: "2026-01-01T00:00:00Z".into(),
        installed_at: 1_700_000_000,
        cover_small_path: "/assets/s.png".into(),
        cover_large_path: "/assets/l.png".into(),
        screenshot_urls: "https://h/a.png\nhttps://h/b.png".into(),
    }
}

#[test]
fn open_creates_file_and_sets_user_version_2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();
    assert!(path.exists());

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
    drop(registry);
}

#[test]
fn upsert_then_all_round_trips_every_field() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    let game = sample("Zelda", "SNES");
    registry.upsert(&game).unwrap();

    let all = registry.all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0], game);
}

#[test]
fn upsert_replaces_row_when_identity_matches_after_normalization() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    registry.upsert(&sample("Zelda", "SNES")).unwrap();

    let mut second = sample(" Zelda ", "SNES");
    second.rom_id = Some(43);
    second.description = "Updated description".into();
    registry.upsert(&second).unwrap();

    let all = registry.all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].rom_id, Some(43));
    assert_eq!(all[0].description, "Updated description");
}

#[test]
fn find_by_rom_id_wins_over_identity() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    let mut game = sample("Zelda", "SNES");
    game.rom_id = Some(99);
    registry.upsert(&game).unwrap();

    // Wrong title/platform, but matching rom_id should still find it.
    let found = registry
        .find(Some(99), "Not The Title", "Not The Platform")
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().rom_id, Some(99));
}

#[test]
fn find_falls_back_to_identity_when_rom_id_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    registry.upsert(&sample("Zelda", "SNES")).unwrap();

    let found = registry.find(None, " zelda ", "snes").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Zelda");
}

#[test]
fn find_falls_back_to_identity_when_rom_id_is_unmatched() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    registry.upsert(&sample("Zelda", "SNES")).unwrap();

    let found = registry.find(Some(12345), "Zelda", "SNES").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Zelda");
}

#[test]
fn find_does_not_match_blank_title_query_to_a_blank_titled_row() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    let mut game = sample("", "");
    game.rom_id = None;
    registry.upsert(&game).unwrap();

    // rom_id 7 doesn't match this row's NULL rom_id, so `find` must not
    // fall back to matching the blank (title_key, platform_key) identity —
    // that fallback exists to rescue pre-rom-id rows by real identity, not
    // to hand back an arbitrary blank-titled row for a blank query.
    let found = registry.find(Some(7), "", "").unwrap();
    assert!(found.is_none());
}

#[test]
fn find_returns_none_when_nothing_matches() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    let found = registry.find(None, "Nope", "Nope").unwrap();
    assert!(found.is_none());
}

#[test]
fn remove_returns_true_then_false() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    registry.upsert(&sample("Zelda", "SNES")).unwrap();

    assert!(registry.remove("Zelda", "SNES").unwrap());
    assert!(!registry.remove("Zelda", "SNES").unwrap());
}

#[test]
fn upsert_stores_empty_archive_path_when_extracted_path_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    let mut game = sample("Zelda", "SNES");
    game.archive_path = "/library/Platform/Game.zip".into();
    game.extracted_path = "/library/Platform/Game/rom.sfc".into();
    registry.upsert(&game).unwrap();

    let all = registry.all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].archive_path, "");
    assert_eq!(all[0].extracted_path, "/library/Platform/Game/rom.sfc");
}

#[test]
fn opening_a_newer_database_errors_mentioning_newer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
    }

    let err = match Registry::open(&path) {
        Ok(_) => panic!("expected Registry::open to error on a newer user_version"),
        Err(e) => e,
    };
    assert!(
        err.to_string().to_lowercase().contains("newer"),
        "error message should mention the DB is from a newer app version: {err}"
    );
}

const V1_SCHEMA: &str = "CREATE TABLE installed_games (
    id INTEGER PRIMARY KEY, title TEXT NOT NULL, platform TEXT NOT NULL,
    title_key TEXT NOT NULL, platform_key TEXT NOT NULL, rom_id INTEGER,
    rom_file_name TEXT NOT NULL DEFAULT '', archive_path TEXT NOT NULL DEFAULT '',
    extracted_path TEXT NOT NULL DEFAULT '', extracted_dir TEXT NOT NULL DEFAULT '',
    multi_file_game_dir TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '',
    rating TEXT NOT NULL DEFAULT '', genres TEXT NOT NULL DEFAULT '', regions TEXT NOT NULL DEFAULT '',
    languages TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '', revision TEXT NOT NULL DEFAULT '',
    companies TEXT NOT NULL DEFAULT '', first_release_date TEXT NOT NULL DEFAULT '',
    filesize_bytes INTEGER NOT NULL DEFAULT 0, server_updated_at TEXT NOT NULL DEFAULT '',
    installed_at INTEGER NOT NULL, UNIQUE (title_key, platform_key));";

#[test]
fn open_migrates_a_v1_database_and_update_images_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(V1_SCHEMA).unwrap();
        conn.execute("INSERT INTO installed_games (title, platform, title_key, platform_key, rom_id, installed_at)
                      VALUES ('Old', 'SNES', 'old', 'snes', 7, 1)", []).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
    let rows = registry.all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cover_small_path, "");
    assert_eq!(rows[0].screenshot_urls, "");

    let fields = ImageFields {
        cover_small_path: "/s.png".into(),
        cover_large_path: "/l.png".into(),
        screenshot_urls: "https://h/x.png".into(),
    };
    assert!(registry.update_images(7, &fields).unwrap());
    assert!(!registry.update_images(999, &fields).unwrap());
    let row = &registry.all().unwrap()[0];
    assert_eq!(row.cover_small_path, "/s.png");
    assert_eq!(row.cover_large_path, "/l.png");
    assert_eq!(row.screenshot_urls, "https://h/x.png");
}

#[test]
fn open_migrates_a_v1_database_that_already_has_one_v2_column() {
    // A database torn by the pre-transaction migration: the first ALTER
    // committed in autocommit, the process died before the rest, so the file
    // sits at user_version 1 with one of the three image columns already
    // present. The migration is idempotent (it skips a column PRAGMA
    // table_info already lists), so this opens and finishes at version 2
    // rather than failing forever with "duplicate column name".
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(V1_SCHEMA).unwrap();
        conn.execute_batch(
            "ALTER TABLE installed_games ADD COLUMN cover_small_path TEXT NOT NULL DEFAULT '';",
        )
        .unwrap();
        conn.execute("INSERT INTO installed_games (title, platform, title_key, platform_key, rom_id, installed_at)
                      VALUES ('Torn', 'SNES', 'torn', 'snes', 7, 1)", []).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }

    let registry = Registry::open(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);

    let mut stmt = conn.prepare("PRAGMA table_info(installed_games)").unwrap();
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for column in ["cover_small_path", "cover_large_path", "screenshot_urls"] {
        assert!(
            columns.iter().any(|c| c == column),
            "migrated schema is missing {column}: {columns:?}"
        );
    }

    // The pre-existing row survives, and the image fields still round-trip.
    let rows = registry.all().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Torn");
    let fields = ImageFields {
        cover_small_path: "/s.png".into(),
        cover_large_path: "/l.png".into(),
        screenshot_urls: "https://h/x.png".into(),
    };
    assert!(registry.update_images(7, &fields).unwrap());
    assert_eq!(registry.all().unwrap()[0].cover_large_path, "/l.png");
}

#[test]
fn open_refuses_a_newer_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    {
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
    }
    assert!(Registry::open(&path).is_err());
}
