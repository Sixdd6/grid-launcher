use base64::Engine;
use grid_core::romm::{RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_cred() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

#[tokio::test]
async fn connect_returns_user_info() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .and(header("authorization", "Bearer FAKE-TEST-TOKEN-not-real"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let user = client.connect().await.unwrap();
    assert_eq!(user.username, "six");
}

#[tokio::test]
async fn unauthorized_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    match client.connect().await {
        Err(RommError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}

#[tokio::test]
async fn errors_never_contain_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let err = client.connect().await.unwrap_err();
    let shown = format!("{err} {err:?}");
    assert!(!shown.contains("FAKE-TEST-TOKEN-not-real"), "leak: {shown}");
}

/// Regression test for a base URL with a subpath (e.g. a RomM server hosted
/// at `https://host/romm`). `Url::join` with a leading-slash path resets to
/// the origin root and would drop `/romm`, so this must fail against that
/// old behavior and pass against the verbatim-concatenation fix.
#[tokio::test]
async fn connect_preserves_base_url_subpath() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/romm/api/users/me"))
        .and(header("authorization", "Bearer FAKE-TEST-TOKEN-not-real"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let base = format!("{}/romm", server.uri());
    let client = RommClient::new(&base, token_cred()).unwrap();
    let user = client.connect().await.unwrap();
    assert_eq!(user.username, "six");
}

/// Regression test: HTTP Basic auth had zero coverage. Asserts the exact
/// `Authorization: Basic <base64(user:pass)>` header value the server
/// receives.
#[tokio::test]
async fn connect_with_basic_auth_sends_expected_header() {
    let server = MockServer::start().await;
    let expected = base64::engine::general_purpose::STANDARD.encode("six:pw-FAKE-not-real");
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .and(header("authorization", format!("Basic {expected}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let cred = Credential::Basic {
        username: "six".to_string(),
        password: SecretString::from("pw-FAKE-not-real"),
    };
    let client = RommClient::new(&server.uri(), cred).unwrap();
    let user = client.connect().await.unwrap();
    assert_eq!(user.username, "six");
}

/// Regression test: `excerpt()` byte-sliced the (server-controlled) body at
/// a fixed offset and could panic if that offset landed mid-UTF-8-char.
/// This body is built so the 240-byte cutoff falls inside a 2-byte
/// character; asserts we get an `Err` back, not a panic.
#[tokio::test]
async fn errors_survive_non_ascii_body_without_panicking() {
    let server = MockServer::start().await;
    let body = format!("{}{}", "a".repeat(239), "é".repeat(50));
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(500).set_body_string(body))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let err = client.connect().await.unwrap_err();
    match err {
        RommError::Http { status, .. } => assert_eq!(status, 500),
        other => panic!("expected Http error, got {other:?}"),
    }
}

// --- firmware -----------------------------------------------------------

#[tokio::test]
async fn firmware_sends_the_platform_id_query_param_and_decodes_items() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .and(query_param("platform_id", "19"))
        .and(header("authorization", "Bearer FAKE-TEST-TOKEN-not-real"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 42, "file_name": "scph5501.bin"},
            {"id": 43, "file_name": "scph5502.bin"}
        ])))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let firmware = client.firmware(19).await.unwrap();
    assert_eq!(firmware.len(), 2);
    assert_eq!(firmware[0].id, 42);
    assert_eq!(firmware[0].file_name, "scph5501.bin");
    assert_eq!(firmware[1].id, 43);
    assert_eq!(firmware[1].file_name, "scph5502.bin");
}

#[tokio::test]
async fn firmware_skips_items_without_an_integer_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 1, "file_name": "good.bin"},
            {"file_name": "missing-id.bin"},
            {"id": "not-a-number", "file_name": "bad-id.bin"}
        ])))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let firmware = client.firmware(19).await.unwrap();
    assert_eq!(firmware.len(), 1);
    assert_eq!(firmware[0].id, 1);
}

#[tokio::test]
async fn firmware_non_array_body_yields_empty_vec() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"detail": "nope"})),
        )
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let firmware = client.firmware(19).await.unwrap();
    assert!(firmware.is_empty());
}

#[tokio::test]
async fn firmware_bytes_encodes_the_file_name_and_returns_the_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/42/content/my%20firmware.bin"))
        .and(header("authorization", "Bearer FAKE-TEST-TOKEN-not-real"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"firmware-bytes".to_vec()))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    let bytes = client.firmware_bytes(42, "my firmware.bin").await.unwrap();
    assert_eq!(bytes, b"firmware-bytes".to_vec());
}

#[tokio::test]
async fn firmware_bytes_unauthorized_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/firmware/42/content/scph5501.bin"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = RommClient::new(&server.uri(), token_cred()).unwrap();
    match client.firmware_bytes(42, "scph5501.bin").await {
        Err(RommError::Unauthorized) => {}
        other => panic!("expected Unauthorized, got {other:?}"),
    }
}
