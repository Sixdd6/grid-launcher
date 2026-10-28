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

/// Mounts one `/api/roms` page for platform 7: `count` roms from `first_id`,
/// answered after `delay_ms`, carrying `total` as given (null = absent).
async fn mount_page(
    server: &MockServer,
    offset: usize,
    first_id: i64,
    count: i64,
    total: serde_json::Value,
    delay_ms: u64,
) {
    let items: Vec<_> = (first_id..first_id + count).map(rom).collect();
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "7"))
        .and(query_param("offset", offset.to_string()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"items": items, "total": total}))
                .set_delay(std::time::Duration::from_millis(delay_ms)),
        )
        .expect(1)
        .mount(server)
        .await;
}

/// A large platform: the pages after the first are fetched together, yet the
/// result keeps server order even when an early page answers last. Every
/// request turns off the rom id index; only the first asks for the total.
#[tokio::test]
async fn games_fetch_later_pages_concurrently_in_order() {
    let server = MockServer::start().await;
    let total = serde_json::json!(650);
    mount_page(&server, 0, 0, 200, total.clone(), 0).await;
    // Each later page takes 300ms, the first of them answering last of all:
    // fetched one at a time that is 900ms or more, together about 300ms.
    mount_page(&server, 200, 200, 200, serde_json::Value::Null, 350).await;
    mount_page(&server, 400, 400, 200, serde_json::Value::Null, 300).await;
    mount_page(&server, 600, 600, 50, serde_json::Value::Null, 300).await;

    let started = std::time::Instant::now();
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 650);
    assert!(games.iter().enumerate().all(|(i, g)| g.id == i as i64));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    for req in &requests {
        let q: Vec<(String, String)> = req.url.query_pairs().into_owned().collect();
        let has = |k: &str, v: &str| q.iter().any(|(a, b)| a == k && b == v);
        assert!(has("with_rom_id_index", "false"), "{}", req.url);
        let first = has("offset", "0");
        assert!(
            has("with_total", if first { "true" } else { "false" }),
            "{}",
            req.url
        );
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(800),
        "took {elapsed:?}"
    );
}

/// A server that sends no total (older RomM) still gets every page, one at a
/// time until a short page.
#[tokio::test]
async fn games_without_a_total_page_until_short_page() {
    let server = MockServer::start().await;
    mount_page(&server, 0, 0, 200, serde_json::Value::Null, 0).await;
    mount_page(&server, 200, 200, 200, serde_json::Value::Null, 0).await;
    mount_page(&server, 400, 400, 3, serde_json::Value::Null, 0).await;
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 403);
    assert_eq!(games[402].id, 402);
}

/// The platform grew between the first page and the rest: the page the
/// total called last is full, so paging carries on past it.
#[tokio::test]
async fn games_keep_paging_when_the_platform_grew() {
    let server = MockServer::start().await;
    mount_page(&server, 0, 0, 200, serde_json::json!(400), 0).await;
    mount_page(&server, 200, 200, 200, serde_json::Value::Null, 0).await;
    mount_page(&server, 400, 400, 10, serde_json::Value::Null, 0).await;
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 410);
}
