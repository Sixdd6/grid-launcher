use grid_core::romm::{RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

#[tokio::test]
async fn rom_detail_maps_full_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 42,
            "name": "Super Game",
            "fs_name_no_ext": "super_game",
            "platform_id": 7,
            "platform_display_name": "SNES",
            "fs_name": "super_game.zip",
            "summary": "A great game.",
            "regions": ["USA", "Europe"],
            "languages": ["en", "fr"],
            "tags": ["classic", "platformer"],
            "revision": "v1.1",
            "fs_size_bytes": 123456,
            "updated_at": "2026-01-01T00:00:00",
            "files": [
                {"id": 1, "file_name": "super_game.sfc", "file_size_bytes": 123456, "is_top_level": true, "last_modified": "2026-01-02T03:04:05"}
            ],
            "metadatum": {
                "average_rating": 87.34,
                "genres": ["Platformer", "Action"],
                "companies": ["Nintendo"],
                "first_release_date": 631152000,
                "franchises": ["Mario"],
                "game_modes": ["Single player", "Co-op"],
                "player_count": "1-2"
            },
            "is_identified": true,
            "youtube_video_id": "abc123",
            "path_video": "/assets/romm/resources/roms/42/video.mp4",
            "igdb_metadata": {
                "similar_games": [{"id": 1, "name": "Sim One", "slug": "s1", "type": "game", "cover_url": ""}]
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(42).await.unwrap();

    assert_eq!(detail.id, 42);
    assert_eq!(detail.name, "Super Game");
    assert_eq!(detail.platform_id, 7);
    assert_eq!(detail.platform_name, "SNES");
    assert_eq!(detail.fs_name, "super_game.zip");
    assert_eq!(detail.description, "A great game.");
    assert_eq!(detail.regions, "USA, Europe");
    assert_eq!(detail.languages, "en, fr");
    assert_eq!(detail.tags, "classic, platformer");
    assert_eq!(detail.revision, "v1.1");
    assert_eq!(detail.rating, "87.3");
    assert_eq!(detail.genres, "Platformer, Action");
    assert_eq!(detail.companies, "Nintendo");
    assert_eq!(detail.first_release_date, "631152000");
    assert_eq!(detail.franchises, "Mario");
    assert_eq!(detail.game_modes, "Single player, Co-op");
    assert_eq!(detail.player_count, "1-2");
    assert_eq!(detail.youtube_video_id, "abc123");
    assert_eq!(
        detail.video_path,
        "/assets/romm/resources/roms/42/video.mp4"
    );
    assert!(detail.is_identified);
    assert_eq!(
        detail.related,
        vec![grid_core::romm::RelatedGame {
            name: "Sim One".to_string(),
            kind: "similar".to_string()
        }]
    );
    assert_eq!(detail.filesize_bytes, 123456);
    assert_eq!(detail.server_updated_at, "2026-01-01T00:00:00");
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.files[0].id, 1);
    assert_eq!(detail.files[0].file_name, "super_game.sfc");
    assert_eq!(detail.files[0].file_size_bytes, 123456);
    assert!(detail.files[0].is_top_level);
    assert_eq!(detail.files[0].last_modified, "2026-01-02T03:04:05");
}

#[tokio::test]
async fn rom_detail_normalises_millisecond_release_dates_to_seconds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/43"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 43,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "metadatum": {
                "first_release_date": 653529600000i64
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(43).await.unwrap();

    assert_eq!(detail.first_release_date, "653529600");
}

#[tokio::test]
async fn rom_detail_leaves_second_precision_release_dates_unchanged() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/44"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 44,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "metadatum": {
                "first_release_date": 631152000
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(44).await.unwrap();

    assert_eq!(detail.first_release_date, "631152000");
}

#[tokio::test]
async fn rom_detail_minimal_payload_decodes_with_empty_strings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "name": null,
            "summary": null,
            "revision": null,
            "metadatum": null
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(1).await.unwrap();

    assert_eq!(detail.id, 1);
    assert_eq!(detail.name, "g");
    assert_eq!(detail.description, "");
    assert_eq!(detail.regions, "");
    assert_eq!(detail.languages, "");
    assert_eq!(detail.tags, "");
    assert_eq!(detail.revision, "");
    assert_eq!(detail.rating, "");
    assert_eq!(detail.genres, "");
    assert_eq!(detail.companies, "");
    assert_eq!(detail.first_release_date, "");
    assert_eq!(detail.franchises, "");
    assert_eq!(detail.game_modes, "");
    assert_eq!(detail.player_count, "");
    assert_eq!(detail.youtube_video_id, "");
    assert_eq!(detail.video_path, "");
    assert!(!detail.is_identified);
    assert!(detail.related.is_empty());
    assert_eq!(detail.filesize_bytes, 0);
    assert_eq!(detail.server_updated_at, "");
    assert!(detail.files.is_empty());
    assert_eq!(detail.cover_small_path, "");
    assert_eq!(detail.cover_large_path, "");
    assert!(detail.screenshot_urls.is_empty());
}

#[tokio::test]
async fn rom_detail_maps_image_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "path_cover_small": "/assets/s.png",
            "path_cover_large": "/assets/l.png",
            "merged_screenshots": [
                "/assets/roms/1/screenshots/a.png",
                "https://other/b.png"
            ],
            "launchbox_metadata": {
                "images": [
                    {"type": "Box - Front", "url": "/box.png"}
                ]
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(1).await.unwrap();

    assert_eq!(detail.cover_small_path, "/assets/s.png");
    assert_eq!(detail.cover_large_path, "/assets/l.png");
    assert_eq!(
        detail.screenshot_urls,
        vec![format!("{}/assets/roms/1/screenshots/a.png", server.uri())]
    );
}

#[tokio::test]
async fn rom_detail_name_falls_back_to_fs_name_no_ext() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 9,
            "name": null,
            "fs_name_no_ext": "unmatched_rom",
            "platform_id": 3,
            "platform_display_name": "NES",
            "fs_name": "unmatched_rom.zip",
            "fs_size_bytes": 10,
            "updated_at": "2026-01-01T00:00:00",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": []
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(9).await.unwrap();

    assert_eq!(detail.name, "unmatched_rom");
}

#[tokio::test]
async fn rom_detail_404_maps_to_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/404"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    match client.rom_detail(404).await {
        Err(RommError::Http { status, .. }) => assert_eq!(status, 404),
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[tokio::test]
async fn rom_detail_maps_the_igdb_block_and_the_media_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/77"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 77,
            "name": "Super Game",
            "fs_name_no_ext": "super_game",
            "platform_id": 7,
            "platform_display_name": "SNES",
            "fs_name": "super_game.zip",
            "summary": "A great game.",
            "regions": ["USA"],
            "languages": ["en"],
            "tags": [],
            "revision": null,
            "fs_size_bytes": 0,
            "updated_at": "2026-01-01T00:00:00",
            "is_identified": true,
            "youtube_video_id": "dQw4w9WgXcQ",
            "path_video": "/assets/romm/resources/roms/77/video.mp4",
            "files": [{
                "id": 1,
                "file_name": "super_game.sfc",
                "file_size_bytes": 10,
                "is_top_level": true,
                "last_modified": "2026-02-03T11:22:33"
            }],
            "metadatum": {
                "average_rating": 87.34,
                "genres": ["Platformer"],
                "companies": ["Nintendo"],
                "first_release_date": 631152000,
                "franchises": ["Mario", "Super Mario"],
                "game_modes": ["Single player"],
                "player_count": "1"
            },
            "igdb_metadata": {
                "similar_games": [{"id": 1, "name": "Sim One", "slug": "s1", "type": "game", "cover_url": "https://images.igdb.com/a.jpg"}],
                "remakes": [{"id": 2, "name": "Remake One", "slug": "r1", "type": "game", "cover_url": ""}],
                "remasters": [{"id": 3, "name": "Remaster One", "slug": "rr1", "type": "game", "cover_url": ""}],
                "dlcs": [{"id": 4, "name": "DLC One", "slug": "d1", "type": "dlc", "cover_url": ""}],
                "expansions": [{"id": 5, "name": "Expansion One", "slug": "e1", "type": "expansion", "cover_url": ""}]
            }
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(77).await.unwrap();

    assert_eq!(detail.franchises, "Mario, Super Mario");
    assert_eq!(detail.game_modes, "Single player");
    assert_eq!(detail.player_count, "1");
    assert_eq!(detail.youtube_video_id, "dQw4w9WgXcQ");
    assert_eq!(
        detail.video_path,
        "/assets/romm/resources/roms/77/video.mp4"
    );
    assert!(detail.is_identified);
    assert_eq!(detail.files[0].last_modified, "2026-02-03T11:22:33");

    // Source order is fixed by `into_detail`: similar, remake, remaster,
    // dlc, expansion — so the Overview row cannot reshuffle between builds.
    let related: Vec<(&str, &str)> = detail
        .related
        .iter()
        .map(|r| (r.name.as_str(), r.kind.as_str()))
        .collect();
    assert_eq!(
        related,
        vec![
            ("Sim One", "similar"),
            ("Remake One", "remake"),
            ("Remaster One", "remaster"),
            ("DLC One", "dlc"),
            ("Expansion One", "expansion"),
        ]
    );
}

#[tokio::test]
async fn rom_detail_without_an_igdb_block_reports_empty_media_and_no_related() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/78"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 78,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [{"id": 1, "file_name": "g.sfc", "file_size_bytes": 0, "is_top_level": true}],
            "name": null,
            "summary": null,
            "revision": null,
            "metadatum": null,
            "igdb_metadata": null,
            "youtube_video_id": null,
            "path_video": null
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(78).await.unwrap();

    assert_eq!(detail.franchises, "");
    assert_eq!(detail.game_modes, "");
    assert_eq!(detail.player_count, "");
    assert_eq!(detail.youtube_video_id, "");
    assert_eq!(detail.video_path, "");
    // `is_identified` is absent from this payload entirely: a server that
    // never sends the flag must read as "not identified", not fail the decode.
    assert!(!detail.is_identified);
    assert!(detail.related.is_empty());
    assert_eq!(detail.files[0].last_modified, "");
}

#[tokio::test]
async fn rom_detail_still_reads_merged_screenshots_now_that_igdb_is_a_named_field() {
    // Regression guard: `merged_screenshots` is read out of RawRomDetail's
    // `#[serde(flatten)] extra` map. Naming any new field removes it from
    // that map, so this pins that the screenshot source survived Task 1.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/79"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 79,
            "fs_name": "g.zip",
            "fs_name_no_ext": "g",
            "platform_id": 2,
            "platform_display_name": "SNES",
            "fs_size_bytes": 0,
            "updated_at": "",
            "regions": [],
            "languages": [],
            "tags": [],
            "files": [],
            "name": null,
            "summary": null,
            "revision": null,
            "metadatum": null,
            "igdb_metadata": {"similar_games": []},
            "merged_screenshots": ["/assets/romm/resources/roms/79/screenshots/1.png"]
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let detail = client.rom_detail(79).await.unwrap();
    assert_eq!(
        detail.screenshot_urls,
        vec![format!(
            "{}/assets/romm/resources/roms/79/screenshots/1.png",
            server.uri()
        )]
    );
}
