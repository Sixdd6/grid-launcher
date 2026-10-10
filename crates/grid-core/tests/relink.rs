//! Wiremock coverage for the Q4 relink pass (`grid_core::library::relink`):
//! which requests it makes and what it writes to the registry.

use std::sync::Arc;

use grid_core::library::registry::{InstalledGame, Registry, IMAGES_VERSION};
use grid_core::library::relink::{relink_pass, server_roms_for_platform, RelinkMatch};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(server: &MockServer) -> RommClient {
    RommClient::new(
        &server.uri(),
        Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real")),
    )
    .unwrap()
}

fn temp_registry() -> (tempfile::TempDir, Arc<Registry>) {
    let dir = tempfile::tempdir().unwrap();
    let registry = Arc::new(Registry::open(&dir.path().join("grid-launcher.db")).unwrap());
    (dir, registry)
}

fn row(title: &str, platform: &str, rom_id: Option<i64>) -> InstalledGame {
    InstalledGame {
        title: title.to_string(),
        platform: platform.to_string(),
        rom_id,
        installed_at: 1,
        ..Default::default()
    }
}

async fn mount_platforms(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/platforms"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 1, "name": "Super Nintendo Entertainment System", "slug": "snes",
             "rom_count": 3, "display_name": "Super Nintendo Entertainment System"},
            {"id": 2, "name": "Arcade", "slug": "arcade", "rom_count": 1}
        ])))
        .mount(server)
        .await;
}

async fn mount_snes_roms(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"id": 101, "name": "Super Mario World", "platform_id": 1},
                {"id": 103, "name": "Secret of Mana", "platform_id": 1},
                {"id": 104, "name": "Secret of Mana", "platform_id": 1}
            ],
            "total": 3
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_pass_links_unique_matches_and_leaves_the_rest() {
    let server = MockServer::start().await;
    mount_platforms(&server).await;
    mount_snes_roms(&server).await;
    // No unlinked row names Arcade, so its list is never asked for.
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("platform_ids", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": []})))
        .expect(0)
        .mount(&server)
        .await;

    let (_dir, registry) = temp_registry();
    registry
        .upsert(&row("Chrono Trigger", "SNES", Some(401)))
        .unwrap();
    registry
        .upsert(&row("Super Mario World", "SNES", None))
        .unwrap();
    registry
        .upsert(&row("Secret of Mana", "SNES", None))
        .unwrap();
    registry.upsert(&row("Mystery Game", "SNES", None)).unwrap();

    let linked = relink_pass(registry.clone(), &client_for(&server))
        .await
        .unwrap();

    assert_eq!(
        linked,
        vec![RelinkMatch {
            title: "Super Mario World".to_string(),
            platform: "SNES".to_string(),
            rom_id: 101,
        }]
    );
    let mario = registry
        .find(None, "Super Mario World", "SNES")
        .unwrap()
        .unwrap();
    assert_eq!(mario.rom_id, Some(101));
    assert_eq!(mario.images_version, 0);
    for title in ["Secret of Mana", "Mystery Game"] {
        let left = registry.find(None, title, "SNES").unwrap().unwrap();
        assert_eq!(left.rom_id, None, "{title} has two or no candidates");
        assert_eq!(left.images_version, IMAGES_VERSION);
    }
    let chrono = registry
        .find(None, "Chrono Trigger", "SNES")
        .unwrap()
        .unwrap();
    assert_eq!(chrono.rom_id, Some(401));
}

#[tokio::test]
async fn the_pass_makes_no_request_when_every_row_is_linked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let (_dir, registry) = temp_registry();
    registry
        .upsert(&row("Chrono Trigger", "SNES", Some(401)))
        .unwrap();

    let linked = relink_pass(registry, &client_for(&server)).await.unwrap();
    assert!(linked.is_empty());
}

#[tokio::test]
async fn the_picker_lists_the_roms_of_the_platform_the_row_names() {
    let server = MockServer::start().await;
    mount_platforms(&server).await;
    mount_snes_roms(&server).await;

    let roms = server_roms_for_platform(&client_for(&server), "snes")
        .await
        .unwrap();
    let ids: Vec<i64> = roms.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![101, 103, 104]);
}

#[tokio::test]
async fn the_picker_is_empty_for_a_platform_the_server_does_not_have() {
    let server = MockServer::start().await;
    mount_platforms(&server).await;

    let roms = server_roms_for_platform(&client_for(&server), "Nintendo 64")
        .await
        .unwrap();
    assert!(roms.is_empty());
}
