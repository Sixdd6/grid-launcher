use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(uri: &str) -> RommClient {
    RommClient::new(
        uri,
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn rom(id: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "name": format!("Game {id}"), "platform_id": 7,
        "path_cover_small": format!("/assets/romm/resources/roms/{id}/cover/small.png")
    })
}

#[tokio::test]
async fn platforms_skips_zero_rom_entries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/platforms"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 7, "name": "SNES", "slug": "snes", "rom_count": 12},
            {"id": 8, "name": "Empty", "slug": "empty", "rom_count": 0}
        ])))
        .mount(&server)
        .await;
    let platforms = client(&server.uri()).platforms().await.unwrap();
    assert_eq!(platforms.len(), 1);
    assert_eq!(platforms[0].slug, "snes");
}

#[tokio::test]
async fn games_paginate_until_short_page() {
    let server = MockServer::start().await;
    let page1: Vec<_> = (0..200).map(rom).collect();
    let page2: Vec<_> = (200..250).map(rom).collect();
    // The real RomM API (openapi.json 5.2.0, docs/porting/01-romm-api.md row
    // 3) filters by the plural, repeatable `platform_ids` param, not a
    // singular `platform_id` — confirmed against
    // grid_launcher/server/catalog.py:163's `{"platform_ids": [platform_id]}`.
    // These matchers pin that: a client that regresses to `platform_id`
    // would fail to match either mock and this test would fail with a 404.
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "7"))
        .and(query_param("offset", "0"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": page1, "total": 250})),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "7"))
        .and(query_param("offset", "200"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": page2, "total": 250})),
        )
        .mount(&server)
        .await;
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 250);
    assert_eq!(games[0].name, "Game 0");
    assert_eq!(games[249].name, "Game 249");
    assert!(games[0].cover_path.as_deref().unwrap().contains("/cover/"));
}

/// Regression test: unidentified/filesystem-only ROMs commonly have a null
/// `name` server-side (`SimpleRomSchema.name` is nullable). The client must
/// not fail to decode the whole page over one such rom — it should fall back
/// to `fs_name_no_ext`, mirroring the Python reference client
/// (`grid_launcher/server/catalog.py:284`).
#[tokio::test]
async fn games_falls_back_to_fs_name_when_name_is_null() {
    let server = MockServer::start().await;
    let items = serde_json::json!([
        {
            "id": 1,
            "name": serde_json::Value::Null,
            "fs_name_no_ext": "Some Game (USA)",
            "platform_id": 7,
            "path_cover_small": serde_json::Value::Null
        },
        {
            "id": 2,
            "name": "Named Game",
            "fs_name_no_ext": "named_game_usa",
            "platform_id": 7,
            "path_cover_small": "/assets/romm/resources/roms/2/cover/small.png"
        }
    ]);
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "7"))
        .and(query_param("offset", "0"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": items, "total": 2})),
        )
        .mount(&server)
        .await;
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].name, "Some Game (USA)");
    assert_eq!(games[1].name, "Named Game");
}
