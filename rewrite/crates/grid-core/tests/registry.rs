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
    }
}

#[test]
fn open_creates_file_and_sets_user_version_1() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grid-launcher.db");
    let registry = Registry::open(&path).unwrap();
    assert!(path.exists());

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
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
