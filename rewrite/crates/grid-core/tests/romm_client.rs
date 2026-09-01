use grid_core::romm::{RommClient, RommError};
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{header, method, path};
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
