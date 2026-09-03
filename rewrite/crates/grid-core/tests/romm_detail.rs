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
                {"id": 1, "file_name": "super_game.sfc", "file_size_bytes": 123456, "is_top_level": true}
            ],
            "metadatum": {
                "average_rating": 87.34,
                "genres": ["Platformer", "Action"],
                "companies": ["Nintendo"],
                "first_release_date": 631152000
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
    assert_eq!(detail.filesize_bytes, 123456);
    assert_eq!(detail.server_updated_at, "2026-01-01T00:00:00");
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.files[0].id, 1);
    assert_eq!(detail.files[0].file_name, "super_game.sfc");
    assert_eq!(detail.files[0].file_size_bytes, 123456);
    assert!(detail.files[0].is_top_level);
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
