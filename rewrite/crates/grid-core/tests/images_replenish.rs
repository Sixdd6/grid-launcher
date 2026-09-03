use grid_core::images::cache::{image_key, ImageCache};
use grid_core::images::replenish::{plan, run, ReplenishItem, ReplenishReport};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn row(
    rom_id: Option<i64>,
    cover_small: &str,
    cover_large: &str,
    screenshots: &str,
) -> InstalledGame {
    let label = rom_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string());
    InstalledGame {
        title: format!("G{label}"),
        platform: "SNES".to_string(),
        rom_id,
        cover_small_path: cover_small.to_string(),
        cover_large_path: cover_large.to_string(),
        screenshot_urls: screenshots.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn plan_classifies_rows() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().to_path_buf());
    let base = "https://h";
    // rom 3 already has its file on disk
    let key = image_key("https://h/assets/3.png");
    std::fs::write(dir.path().join(format!("{key}.png")), b"\x89PNG\r\n\x1a\n").unwrap();
    let rows = vec![
        row(Some(1), "", "", ""),
        row(Some(2), "/assets/2.png", "", ""),
        row(Some(3), "/assets/3.png", "/assets/3l.png", ""),
        row(None, "", "", ""),
        row(Some(5), "https://other/5.png", "", ""), // foreign host: no fetch target
    ];
    assert_eq!(
        plan(&rows, &cache, base),
        vec![
            ReplenishItem::NeedsFields { rom_id: 1 },
            ReplenishItem::NeedsFile {
                rom_id: 2,
                url: "https://h/assets/2.png".into()
            },
        ]
    );
}

#[tokio::test]
async fn run_backfills_fields_fetches_files_and_counts_skips() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "name": "G1", "platform_id": 1, "path_cover_small": "/assets/1.png",
            "path_cover_large": "/assets/1l.png",
            "merged_screenshots": ["/assets/roms/1/screenshots/a.png"]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/assets/1.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/assets/2.png"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/roms/9"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(dir.path().join("covers"));
    let registry = Registry::open(&dir.path().join("db.sqlite")).unwrap();
    registry.upsert(&row(Some(1), "", "", "")).unwrap();
    registry
        .upsert(&row(Some(2), "/assets/2.png", "", ""))
        .unwrap();
    registry.upsert(&row(Some(9), "", "", "")).unwrap();
    let client = client_for(&server);
    let items = plan(&registry.all().unwrap(), &cache, &server.uri());
    let report = run(&client, &cache, &registry, &server.uri(), items).await;

    assert_eq!(
        report,
        ReplenishReport {
            updated_rows: 1,
            fetched_files: 1,
            skipped: 2
        }
    );
    let rows = registry.all().unwrap();
    let r1 = rows.iter().find(|r| r.rom_id == Some(1)).unwrap();
    assert_eq!(r1.cover_small_path, "/assets/1.png");
    assert_eq!(r1.cover_large_path, "/assets/1l.png");
    assert_eq!(
        r1.screenshot_urls,
        format!("{}/assets/roms/1/screenshots/a.png", server.uri())
    );
    assert!(cache
        .find_existing(&image_key(&format!("{}/assets/1.png", server.uri())))
        .is_some());
}
