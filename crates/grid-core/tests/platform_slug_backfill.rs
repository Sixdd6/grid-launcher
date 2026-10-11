//! Wiremock coverage for the registry v8 platform-slug backfill
//! (`grid_core::library::platform_slug_backfill`): which rows it asks the
//! server about and what it stores.

use std::sync::Arc;

use grid_core::library::platform_slug_backfill::backfill_platform_slugs;
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn row(title: &str, platform: &str, rom_id: Option<i64>, slug: &str) -> InstalledGame {
    InstalledGame {
        title: title.to_string(),
        platform: platform.to_string(),
        rom_id,
        platform_slug: slug.to_string(),
        installed_at: 1,
        ..Default::default()
    }
}

fn detail(id: i64, name: &str, display: &str, slug: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "platform_id": 52,
        "platform_display_name": display,
        "platform_slug": slug,
        "platform_fs_slug": "win-fangames-ports",
        "fs_name": format!("{name}.zip"),
        "files": [],
    })
}

#[tokio::test]
async fn the_pass_fills_blank_slugs_by_rom_id_and_skips_failures() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/roms/52001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail(
            52001,
            "Tamriel Rebuilt",
            "PC Fangames & Ports",
            "win",
        )))
        .expect(1)
        .mount(&server)
        .await;
    // A ROM the server no longer has: skipped, the pass goes on.
    Mock::given(method("GET"))
        .and(path("/api/roms/404"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    // A row that already has its slug is never asked about.
    Mock::given(method("GET"))
        .and(path("/api/roms/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail(7, "X", "SNES", "snes")))
        .expect(0)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let registry = Arc::new(Registry::open(&dir.path().join("grid-launcher.db")).unwrap());
    registry
        .upsert(&row(
            "Tamriel Rebuilt",
            "PC Fangames & Ports",
            Some(52001),
            "",
        ))
        .unwrap();
    registry
        .upsert(&row("Gone", "SNES", Some(404), ""))
        .unwrap();
    registry
        .upsert(&row("Linked", "SNES", Some(7), "snes"))
        .unwrap();
    registry.upsert(&row("Unlinked", "SNES", None, "")).unwrap();
    let before = registry.find(Some(52001), "", "").unwrap().unwrap();
    assert!(!before.is_native(), "the display name alone is not native");

    let filled = backfill_platform_slugs(&registry, &client_for(&server))
        .await
        .unwrap();
    assert_eq!(filled, vec![52001]);

    let after = registry.find(Some(52001), "", "").unwrap().unwrap();
    assert_eq!(after.platform_slug, "win");
    assert!(after.is_native());
    let gone = registry.find(Some(404), "", "").unwrap().unwrap();
    assert_eq!(gone.platform_slug, "");
}

#[tokio::test]
async fn the_pass_is_harmless_when_the_server_is_unreachable() {
    // Nothing listens on port 1: every fetch fails, nothing is written.
    let client = RommClient::new(
        "http://127.0.0.1:1",
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let registry = Registry::open(&dir.path().join("grid-launcher.db")).unwrap();
    registry
        .upsert(&row(
            "Tamriel Rebuilt",
            "PC Fangames & Ports",
            Some(52001),
            "",
        ))
        .unwrap();
    let filled = backfill_platform_slugs(&registry, &client).await.unwrap();
    assert!(filled.is_empty());
    let stored = registry.find(Some(52001), "", "").unwrap().unwrap();
    assert_eq!(stored.platform_slug, "");
}
