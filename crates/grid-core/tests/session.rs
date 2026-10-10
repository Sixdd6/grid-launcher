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

/// A server URL typed with embedded credentials must never be stored in
/// memory or persisted to config: the password would land in `config.toml`
/// and in every `SessionState` crossing IPC, and the userinfo netloc would
/// make host filtering reject every absolute image URL.
#[tokio::test]
async fn connect_strips_userinfo_from_the_stored_and_persisted_server_url() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store.clone(),
    );

    // The mock's host with userinfo spliced in, so the probe still reaches it.
    let with_userinfo = server
        .uri()
        .replacen("http://", "http://user:FAKE-TEST-PASSWORD@", 1);
    let state = mgr
        .connect(
            with_userinfo,
            "six".into(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .unwrap();

    let expected = server.uri();
    assert_eq!(mgr.server_url(), expected);
    assert_eq!(state.server_url, expected);
    let persisted = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
    assert!(!persisted.contains("FAKE-TEST-PASSWORD"));
    assert!(persisted.contains(&expected));

    // A restore over the same config keeps the stripped URL.
    let mgr2 = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        store,
    );
    let RestoreOutcome::Connected { state } = mgr2.restore().await.unwrap() else {
        panic!("expected Connected")
    };
    assert_eq!(state.server_url, expected);
    assert_eq!(mgr2.server_url(), expected);
}

#[tokio::test]
async fn restore_reports_no_session_without_stored_server() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let RestoreOutcome::NoSession {
        server_url,
        username,
    } = mgr.restore().await.unwrap()
    else {
        panic!("expected NoSession")
    };
    assert_eq!(server_url, "");
    assert_eq!(username, "");
}

/// A Python import writes `server_url`/`username` into config.toml but no
/// credential, so restore is `NoSession` — and must hand both fields back
/// for the Connect form to prefill.
#[tokio::test]
async fn restore_reports_no_session_with_the_stored_server_when_no_credential_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let cfg = grid_core::config::Config {
        server_url: "https://romm.example.test".into(),
        username: "importer".into(),
        ..Default::default()
    };
    cfg.save(&config_path).unwrap();
    let mgr = SessionManager::new(
        config_path,
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let RestoreOutcome::NoSession {
        server_url,
        username,
    } = mgr.restore().await.unwrap()
    else {
        panic!("expected NoSession")
    };
    assert_eq!(server_url, "https://romm.example.test");
    assert_eq!(username, "importer");
}

#[tokio::test]
async fn restore_reports_unreachable_and_retry_fails_while_down() {
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
    // Retry answers with a typed outcome: a dead server stays Unreachable
    // (the shell's "Not connected + Retry"), never Unauthorized.
    match mgr2.retry().await.unwrap() {
        RestoreOutcome::Unreachable { error, .. } => assert!(!error.is_empty()),
        other => panic!("expected Unreachable, got {other:?}"),
    }
    assert!(mgr2.client().is_none());
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

// --- Q6: a rejected credential re-opens Connect ---------------------------

use grid_core::session::{AuthKind, SessionUnauthorized};
use std::sync::Mutex;

/// A config + keyring pair as a previous run leaves them: the server URL and
/// verified username in config.toml, the credential in the store.
fn stored_session(
    dir: &tempfile::TempDir,
    server_url: &str,
    cred: Credential,
) -> (SessionManager, Arc<MemoryStore>) {
    let config_path = dir.path().join("config.toml");
    let cfg = grid_core::config::Config {
        server_url: server_url.into(),
        username: "six".into(),
        ..Default::default()
    };
    cfg.save(&config_path).unwrap();
    let store = Arc::new(MemoryStore::default());
    SecretStore::save(&*store, &cred).unwrap();
    let mgr = SessionManager::new(config_path, dir.path().join("covers"), store.clone());
    (mgr, store)
}

fn token() -> Credential {
    Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))
}

async fn mount_status(server: &MockServer, route: &str, status: u16) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

#[tokio::test]
async fn restore_with_a_rejected_token_reports_unauthorized_and_keeps_the_credential() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());

    let outcome = mgr.restore().await.unwrap();
    match outcome {
        RestoreOutcome::Unauthorized {
            ref server_url,
            ref username,
            auth_kind,
        } => {
            assert_eq!(server_url, &server.uri());
            assert_eq!(username, "six");
            assert_eq!(auth_kind, AuthKind::Token);
        }
        ref other => panic!("expected Unauthorized, got {other:?}"),
    }
    assert!(mgr.client().is_none());
    // User decision Q6: the keyring item stays until a new connect succeeds.
    assert!(SecretStore::load(&*store).unwrap().is_some());
    // The IPC shape carries no secret.
    let wire = serde_json::to_string(&outcome).unwrap();
    assert!(!wire.contains("FAKE-TEST-TOKEN-not-real"), "{wire}");
    assert_eq!(
        serde_json::to_value(&outcome).unwrap(),
        serde_json::json!({
            "kind": "unauthorized",
            "server_url": server.uri(),
            "username": "six",
            "auth_kind": "token",
        })
    );
}

#[tokio::test]
async fn restore_with_a_rejected_password_reports_basic_auth_kind() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let cred = Credential::Basic {
        username: "six".into(),
        password: SecretString::from("FAKE-TEST-PASSWORD"),
    };
    let (mgr, _store) = stored_session(&dir, &server.uri(), cred);
    let outcome = mgr.restore().await.unwrap();
    assert!(
        matches!(
            outcome,
            RestoreOutcome::Unauthorized {
                auth_kind: AuthKind::Basic,
                ..
            }
        ),
        "{outcome:?}"
    );
    let wire = serde_json::to_string(&outcome).unwrap();
    assert!(!wire.contains("FAKE-TEST-PASSWORD"), "{wire}");
}

/// 403 is "insufficient permission", not a dead credential: no Connect form.
#[tokio::test]
async fn restore_with_forbidden_is_not_unauthorized() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 403).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    match mgr.restore().await.unwrap() {
        RestoreOutcome::Unreachable { error, .. } => {
            assert!(error.contains("permission"), "{error}");
        }
        other => panic!("expected Unreachable, got {other:?}"),
    }
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn restore_with_a_network_error_stays_unreachable() {
    let server = MockServer::builder().start().await;
    let uri = server.uri();
    drop(server);
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &uri, token());
    assert!(matches!(
        mgr.restore().await.unwrap(),
        RestoreOutcome::Unreachable { .. }
    ));
}

#[tokio::test]
async fn retry_with_a_rejected_token_reports_unauthorized() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    match mgr.retry().await.unwrap() {
        RestoreOutcome::Unauthorized {
            server_url,
            username,
            auth_kind,
        } => {
            assert_eq!(server_url, server.uri());
            assert_eq!(username, "six");
            assert_eq!(auth_kind, AuthKind::Token);
        }
        other => panic!("expected Unauthorized, got {other:?}"),
    }
    assert!(mgr.client().is_none());
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn retry_that_connects_reports_connected() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    assert!(matches!(
        mgr.retry().await.unwrap(),
        RestoreOutcome::Connected { .. }
    ));
    assert!(mgr.client().is_some());
}

/// Records every hook call.
fn recording_hook(mgr: &SessionManager) -> Arc<Mutex<Vec<SessionUnauthorized>>> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let sink = calls.clone();
    mgr.set_unauthorized_hook(Arc::new(move |info| sink.lock().unwrap().push(info)));
    calls
}

#[tokio::test]
async fn a_mid_session_401_fires_the_hook_once_and_keeps_the_credential() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    let calls = recording_hook(&mgr);
    assert!(matches!(
        mgr.restore().await.unwrap(),
        RestoreOutcome::Connected { .. }
    ));

    let client = mgr.client().unwrap();
    let (a, b) = tokio::join!(client.platforms(), client.platforms());
    assert!(matches!(a, Err(grid_core::romm::RommError::Unauthorized)));
    assert!(matches!(b, Err(grid_core::romm::RommError::Unauthorized)));
    assert!(client.platforms().await.is_err());

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "one event per session, not per request");
    assert_eq!(
        calls[0],
        SessionUnauthorized {
            server_url: server.uri(),
            username: "six".into(),
            auth_kind: AuthKind::Token,
        }
    );
    assert!(mgr.client().is_none(), "the dead session is dropped");
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn a_mid_session_403_does_not_fire_the_hook() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 403).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    let calls = recording_hook(&mgr);
    mgr.restore().await.unwrap();
    let client = mgr.client().unwrap();
    assert!(matches!(
        client.platforms().await,
        Err(grid_core::romm::RommError::Forbidden)
    ));
    assert!(calls.lock().unwrap().is_empty());
    assert!(mgr.client().is_some());
}

/// A request still in flight on the old client must not throw the user out
/// of the session that replaced it.
#[tokio::test]
async fn a_401_on_a_replaced_client_does_not_fire_the_hook() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    let calls = recording_hook(&mgr);
    mgr.restore().await.unwrap();
    let old = mgr.client().unwrap();
    mgr.retry().await.unwrap();
    assert!(old.platforms().await.is_err());
    assert!(calls.lock().unwrap().is_empty());
    assert!(mgr.client().is_some());
}

/// A 401 on the connect probe is the Connect form's own error, not a
/// mid-session event.
#[tokio::test]
async fn a_rejected_connect_does_not_fire_the_hook() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let calls = recording_hook(&mgr);
    let err = mgr
        .connect(
            server.uri(),
            String::new(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "the server rejected the credentials");
    assert!(calls.lock().unwrap().is_empty());
}
