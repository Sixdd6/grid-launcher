//! GOG's HTTP APIs against a mock server: the account, the owned list
//! (galaxy-library and the embed fallback, details from api.gog.com and
//! gamesdb, DLC folded), builds, remote config and the cloud listing. The
//! bearer is the sanctioned fake and never shows in an error.

use grid_core::stores::gog::api;
use grid_core::stores::gog::meta::GogPlatform;
use grid_core::stores::gog::GogEndpoints;
use grid_core::stores::StoreError;
use secrecy::SecretString;
use serde_json::json;
use url::Url;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

fn bearer() -> SecretString {
    SecretString::from(FAKE.to_string())
}

fn auth_header() -> String {
    format!("Bearer {FAKE}")
}

async fn server() -> (MockServer, GogEndpoints) {
    let server = MockServer::start().await;
    let ep = GogEndpoints::e2e(Url::parse(&format!("{}/", server.uri())).unwrap());
    (server, ep)
}

#[tokio::test]
async fn the_account_comes_from_user_data_and_a_401_is_login_expired() {
    let (server, ep) = server().await;
    Mock::given(method("GET"))
        .and(path("/userData.json"))
        .and(header("Authorization", auth_header().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "isLoggedIn": true, "username": "fakeuser", "galaxyUserId": "4862"
        })))
        .mount(&server)
        .await;
    let http = reqwest::Client::new();
    let account = api::user_data(&http, &ep, &bearer()).await.unwrap();
    assert_eq!(account.username, "fakeuser");
    assert_eq!(account.user_id, "4862");

    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/userData.json"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let error = api::user_data(&http, &ep, &bearer()).await.unwrap_err();
    assert!(matches!(error, StoreError::LoginExpired));
    assert!(!format!("{error} {error:?}").contains(FAKE));
}

#[tokio::test]
async fn the_owned_list_pages_merges_and_folds_dlc() {
    let (server, ep) = server().await;
    Mock::given(method("GET"))
        .and(path("/users/4862/releases"))
        .and(query_param("page_token", "p2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"platform_id": "gog", "external_id": "9"}],
            "next_page_token": ""
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/4862/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {"platform_id": "gog", "external_id": "1"},
                {"platform_id": "steam", "external_id": "620"}
            ],
            "next_page_token": "p2"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/products"))
        .and(query_param("ids", "1,9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 1, "title": "Base Game", "game_type": "game",
             "images": {"logo2x": "//images.gog.com/base_logo.jpg"},
             "content_system_compatibility": {"windows": true, "linux": false},
             "expanded_dlcs": [{"id": 9, "title": "Expansion"}]},
            {"id": 9, "title": "Expansion", "game_type": "dlc"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/platforms/gog/external_releases/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "title": {"*": "Base Game"},
            "type": "game",
            "supported_operating_systems": [{"slug": "windows"}, {"slug": "linux"}],
            "game": {
                "vertical_cover": {"url_format": "https://images.gog.com/vc{formatter}.{ext}"},
                "developers": [{"name": "Dev Studio"}],
                "genres": [{"name": {"*": "Adventure"}}]
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/platforms/gog/external_releases/9"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let products = api::owned_products(&http, &ep, &bearer(), "4862")
        .await
        .unwrap();
    assert_eq!(products.len(), 1);
    let base = &products[0];
    assert_eq!(base.id, "1");
    assert_eq!(base.cover, "https://images.gog.com/vc.jpg");
    assert_eq!(base.developer, "Dev Studio");
    assert_eq!(base.genres, ["Adventure"]);
    assert_eq!(base.platforms, [GogPlatform::Windows, GogPlatform::Linux]);
    assert_eq!(base.dlc.len(), 1);
    assert_eq!(base.dlc[0].id, "9");
    assert_eq!(base.dlc[0].title, "Expansion");
}

#[tokio::test]
async fn the_owned_list_falls_back_to_embed() {
    let (server, ep) = server().await;
    Mock::given(method("GET"))
        .and(path("/users/4862/releases"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/user/data/games"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"owned": [5, 6]})))
        .mount(&server)
        .await;
    let http = reqwest::Client::new();
    let ids = api::owned_ids(&http, &ep, &bearer(), "4862").await.unwrap();
    assert_eq!(ids, ["5", "6"]);
}

#[tokio::test]
async fn a_failing_network_says_gog_and_never_the_token() {
    let (server, ep) = server().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_body_string(FAKE))
        .mount(&server)
        .await;
    let http = reqwest::Client::new();
    let error = api::owned_products(&http, &ep, &bearer(), "4862")
        .await
        .unwrap_err();
    let text = format!("{error} {error:?}");
    assert_eq!(error.to_string(), "couldn't reach GOG");
    assert!(!text.contains(FAKE), "{text}");
}

#[tokio::test]
async fn builds_remote_config_and_cloud_listing() {
    let (server, ep) = server().await;
    Mock::given(method("GET"))
        .and(path("/products/1/os/windows/builds"))
        .and(query_param("generation", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_count": 2,
            "items": [
                {"build_id": "8", "branch": "beta", "version_name": "2b", "generation": 2, "link": "x"},
                {"build_id": "7", "branch": null, "version_name": "1.7", "generation": 2, "link": "x"}
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/components/galaxy_client/clients/5051"))
        .and(query_param("component_version", "2.0.45"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": {"Windows": {"cloudStorage": {"enabled": true, "locations": [
                {"name": "saves", "location": "<?SAVED_GAMES?>/Game"}
            ]}}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/4862/5051"))
        .and(header("Authorization", auth_header().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "saves/a.sav", "hash": "abc", "last_modified": "2024-01-02T03:04:05+00:00"}
        ])))
        .mount(&server)
        .await;
    let http = reqwest::Client::new();
    let builds = api::builds(&http, &ep, Some(&bearer()), "1", "windows")
        .await
        .unwrap();
    assert_eq!(api::default_build(&builds).unwrap().build_id, "7");
    let locations = api::save_locations(&http, &ep, "5051").await.unwrap();
    assert_eq!(locations[0].name, "saves");
    let files = api::cloud_files(&http, &ep, &bearer(), "4862", "5051")
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].modified, 1_704_164_645);
    let none = api::save_locations(&http, &ep, "404").await.unwrap();
    assert!(none.is_empty());
}
