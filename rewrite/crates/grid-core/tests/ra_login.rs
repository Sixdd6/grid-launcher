use grid_core::retroachievements::{build_http_client, ra_login_with_base, RaLogin};
use secrecy::{ExposeSecret, SecretString};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAKE_PASSWORD: &str = "pw-FAKE";

async fn login(server: &MockServer) -> Result<RaLogin, String> {
    ra_login_with_base(
        &build_http_client(),
        &format!("{}/dorequest.php", server.uri()),
        "sixdd6",
        &SecretString::from(FAKE_PASSWORD),
    )
    .await
}

#[tokio::test]
async fn login_returns_the_server_reported_user_and_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dorequest.php"))
        .and(query_param("r", "login"))
        .and(query_param("u", "sixdd6"))
        .and(query_param("p", FAKE_PASSWORD))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": true,
            "User": "Sixdd6",
            "Token": "FAKE-RA-TOKEN-not-real"
        })))
        .mount(&server)
        .await;

    let login = login(&server).await.unwrap();
    // The SERVER's spelling wins over what was typed.
    assert_eq!(login.username, "Sixdd6");
    assert_eq!(login.token.expose_secret(), "FAKE-RA-TOKEN-not-real");
}

#[tokio::test]
async fn login_reports_the_servers_error_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": false,
            "Error": "Invalid User/Password combination. Please try again."
        })))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "Invalid User/Password combination. Please try again."
    );
}

#[tokio::test]
async fn a_success_payload_with_a_message_still_logs_in() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": true,
            "User": "Sixdd6",
            "Token": "FAKE-RA-TOKEN-not-real",
            "Message": "Welcome back"
        })))
        .mount(&server)
        .await;

    let login = login(&server).await.unwrap();
    assert_eq!(login.username, "Sixdd6");
    assert_eq!(login.token.expose_secret(), "FAKE-RA-TOKEN-not-real");
}

#[tokio::test]
async fn a_failure_payload_with_only_a_message_surfaces_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": false,
            "Message": "Account locked"
        })))
        .mount(&server)
        .await;

    assert_eq!(login(&server).await.unwrap_err(), "Account locked");
}

#[tokio::test]
async fn login_falls_back_to_invalid_credentials_when_the_server_says_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": false
        })))
        .mount(&server)
        .await;

    assert_eq!(login(&server).await.unwrap_err(), "Invalid credentials");
}

#[tokio::test]
async fn login_rejects_a_success_payload_with_no_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "Success": true,
            "User": "Sixdd6"
        })))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "RetroAchievements login response missing Token"
    );
}

#[tokio::test]
async fn login_maps_an_http_error_to_the_reference_wording() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
        .mount(&server)
        .await;

    assert_eq!(
        login(&server).await.unwrap_err(),
        "RetroAchievements HTTP 503: upstream down"
    );
}

/// Token secrecy: the password is in the query string, so no error may ever
/// carry the URL. The transport failure below is produced by pointing at a
/// port nothing is listening on: 127.0.0.1:1 refuses instantly, needs no DNS,
/// and cannot be bound by a parallel test (port 1 is privileged). Dropping a
/// `MockServer` for this instead would race its async shutdown and answer 404.
#[tokio::test]
async fn a_transport_failure_never_echoes_the_url_or_the_password() {
    let uri = "http://127.0.0.1:1".to_string();

    let err = ra_login_with_base(
        &build_http_client(),
        &format!("{uri}/dorequest.php"),
        "sixdd6",
        &SecretString::from(FAKE_PASSWORD),
    )
    .await
    .unwrap_err();

    assert!(
        err.starts_with("RetroAchievements request failed: "),
        "unexpected error: {err}"
    );
    assert!(!err.contains(FAKE_PASSWORD), "password leaked: {err}");
    assert!(!err.contains(&uri), "url leaked: {err}");
}

#[tokio::test]
async fn a_blank_username_or_password_never_reaches_the_network() {
    let server = MockServer::start().await;
    // No Mock is mounted: any request would 404 with a different message.
    let base = format!("{}/dorequest.php", server.uri());
    let http = build_http_client();

    assert_eq!(
        ra_login_with_base(&http, &base, "  ", &SecretString::from(FAKE_PASSWORD))
            .await
            .unwrap_err(),
        "username must be a non-empty string"
    );
    assert_eq!(
        ra_login_with_base(&http, &base, "sixdd6", &SecretString::from("  "))
            .await
            .unwrap_err(),
        "password must be a non-empty string"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}
