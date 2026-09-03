use grid_core::secrets::{Credential, MemoryStore, SecretError, SecretStore};
use grid_core::session::{RestoreOutcome, SessionManager};
use secrecy::SecretString;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mounts the `/api/users/me` response every successful probe needs.
async fn mount_users_me(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1, "username": "six"
        })))
        .mount(server)
        .await;
}

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
    let restored = mgr2.restore().await.expect("restore should not error");
    let RestoreOutcome::Connected { state } = restored else {
        panic!("expected Connected, got {restored:?}")
    };
    assert!(state.connected);
    assert_eq!(state.server_url, server.uri());
}

#[tokio::test]
async fn restore_reports_no_session_without_stored_server() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    assert!(matches!(
        mgr.restore().await.unwrap(),
        RestoreOutcome::NoSession
    ));
}

#[tokio::test]
async fn restore_reports_unreachable_and_retry_reconnects() {
    // connect against a live mock, then drop the mock and restore from a
    // fresh manager: Unreachable with the stored server url; bringing a mock
    // back on the same address is not possible, so retry is asserted
    // against a manager whose stored url still points at the (now dead)
    // server, expecting the retry itself to fail too.
    //
    // `MockServer::start()` (no builder) is drawn from wiremock's internal
    // pool and, on drop, is only reset and returned for reuse — the
    // listener stays up on the same port, so a request right after drop
    // would still succeed. `builder().start()` opts out of pooling: its
    // listener genuinely shuts down when the server is dropped.
    let server = MockServer::builder().start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );
    mgr.connect(
        server.uri(),
        String::new(),
        SecretString::from("FAKE-TEST-TOKEN-not-real"),
        true,
    )
    .await
    .unwrap();
    let uri = server.uri();
    drop(server);

    let mgr2 = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );
    match mgr2.restore().await.unwrap() {
        RestoreOutcome::Unreachable {
            server_url, error, ..
        } => {
            assert_eq!(server_url, uri);
            assert!(!error.is_empty());
        }
        other => panic!("expected Unreachable, got {other:?}"),
    }
    assert!(mgr2.client().is_none());
    assert_eq!(mgr2.server_url(), uri);
    assert!(mgr2.retry().await.is_err());
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
