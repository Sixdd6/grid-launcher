use grid_core::secrets::{Credential, MemoryStore, SecretError, SecretStore};
use grid_core::session::SessionManager;
use secrecy::SecretString;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A `SecretStore` whose `save()` always fails, to exercise the
/// persist-failure path of `SessionManager::connect()`.
#[derive(Default)]
struct FailingStore;

impl SecretStore for FailingStore {
    fn save(&self, _cred: &Credential) -> Result<(), SecretError> {
        Err(SecretError::Keyring("simulated save failure".into()))
    }
    fn load(&self) -> Result<Option<Credential>, SecretError> {
        Ok(None)
    }
    fn clear(&self) -> Result<(), SecretError> {
        Ok(())
    }
}

#[tokio::test]
async fn connect_persists_and_restore_reconnects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );

    let state = mgr
        .connect(
            server.uri(),
            "six".into(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .unwrap();
    assert!(state.connected);
    assert_eq!(state.username, "six");

    // A fresh manager over the same config path + store restores the session.
    let mgr2 = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store,
    );
    let restored = mgr2
        .restore()
        .await
        .unwrap()
        .expect("session should restore");
    assert!(restored.connected);
    assert_eq!(restored.server_url, server.uri());
}

#[tokio::test]
async fn connect_leaves_client_unset_when_secret_save_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(FailingStore);
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store,
    );

    let result = mgr
        .connect(
            server.uri(),
            "six".into(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await;
    assert!(
        result.is_err(),
        "connect() should surface the secret-store failure"
    );
    assert!(
        mgr.client().is_none(),
        "client() must stay unset when connect() failed to persist"
    );
}

#[tokio::test]
async fn disconnect_clears_credential() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );
    mgr.disconnect().unwrap();
    use grid_core::secrets::SecretStore;
    assert!(store.load().unwrap().is_none());
}

#[tokio::test]
async fn token_connect_rejects_mismatched_username_and_persists_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );
    let result = mgr
        .connect(
            server.uri(),
            "wronguser".into(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await;
    let err = result.expect_err("mismatched username must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("six") && msg.contains("wronguser"),
        "unhelpful error: {msg}"
    );
    assert!(
        mgr.client().is_none(),
        "client must not be set after rejection"
    );
    assert!(
        store.load().unwrap().is_none(),
        "credential must not persist"
    );
    assert!(
        !dir.path().join("config.toml").exists(),
        "config must not persist after rejection"
    );
}

#[tokio::test]
async fn token_connect_without_username_adopts_server_account() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("covers").with_file_name("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let state = mgr
        .connect(
            server.uri(),
            String::new(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .unwrap();
    assert_eq!(state.username, "six");
    let cfg = grid_core::config::Config::load(&dir.path().join("config.toml")).unwrap();
    assert_eq!(
        cfg.username, "six",
        "config must store the server-verified name"
    );
}

#[tokio::test]
async fn token_connect_accepts_case_insensitive_username_and_stores_server_casing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let state = mgr
        .connect(
            server.uri(),
            "SIX".into(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .unwrap();
    assert_eq!(state.username, "six");
    let cfg = grid_core::config::Config::load(&dir.path().join("config.toml")).unwrap();
    assert_eq!(cfg.username, "six");
}
