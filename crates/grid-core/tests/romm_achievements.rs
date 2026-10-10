//! U6: saving the RA username in GRID also sets it on the RomM account.
//! Only the username crosses to RomM; the request carries the RomM
//! credential and nothing else secret.

use grid_core::romm::{RaUsernameSync, RommClient};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{body_json, body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "FAKE-TEST-TOKEN-not-real";

fn client(server: &MockServer) -> RommClient {
    RommClient::new(&server.uri(), Credential::Token(SecretString::from(TOKEN))).unwrap()
}

async fn mount_me(server: &MockServer, ra_username: &str) {
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 7, "username": "tester", "ra_username": ra_username, "ra_progression": {}
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_new_name_is_put_as_a_form_then_a_failed_refresh_is_not_fatal() {
    let server = MockServer::start().await;
    mount_me(&server, "").await;
    Mock::given(method("PUT"))
        .and(path("/api/users/7"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
        .and(body_string("ra_username=fake-ra-player"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 7 })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/users/7/ra/refresh"))
        .and(body_json(serde_json::json!({ "incremental": true })))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = client(&server).sync_ra_username(" fake-ra-player ").await;
    assert_eq!(outcome, RaUsernameSync::Updated { refreshed: false });
}

#[tokio::test]
async fn a_successful_refresh_is_reported() {
    let server = MockServer::start().await;
    mount_me(&server, "old-name").await;
    Mock::given(method("PUT"))
        .and(path("/api/users/7"))
        .and(body_string("ra_username=fake-ra-player"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/users/7/ra/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = client(&server).sync_ra_username("fake-ra-player").await;
    assert_eq!(outcome, RaUsernameSync::Updated { refreshed: true });
}

#[tokio::test]
async fn the_same_name_sends_no_update() {
    let server = MockServer::start().await;
    mount_me(&server, "fake-ra-player").await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let outcome = client(&server).sync_ra_username("fake-ra-player").await;
    assert_eq!(outcome, RaUsernameSync::Unchanged);
}

#[tokio::test]
async fn a_blank_name_sends_nothing_at_all() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let outcome = client(&server).sync_ra_username("   ").await;
    assert_eq!(outcome, RaUsernameSync::Skipped);
}

#[tokio::test]
async fn a_rejected_update_fails_without_leaking_the_token() {
    let server = MockServer::start().await;
    mount_me(&server, "").await;
    Mock::given(method("PUT"))
        .and(path("/api/users/7"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let outcome = client(&server).sync_ra_username("fake-ra-player").await;
    let RaUsernameSync::Failed { error } = &outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(!error.contains(TOKEN));
    assert!(!format!("{outcome:?}").contains(TOKEN));
}

#[tokio::test]
async fn rom_and_user_ra_fields_decode_from_the_live_endpoints() {
    let server = MockServer::start().await;
    mount_me(&server, "").await;
    Mock::given(method("GET"))
        .and(path("/api/roms/194"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 194,
            "ra_id": 5001,
            "merged_ra_metadata": { "achievements": [{ "ra_id": 11, "title": "First" }] }
        })))
        .mount(&server)
        .await;
    let client = client(&server);
    let rom = client.rom_ra(194).await.unwrap();
    let user = client.me_ra().await.unwrap();
    assert_eq!(rom.achievements().len(), 1);
    assert_eq!(user.ra_username(), None);
}
