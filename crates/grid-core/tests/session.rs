use grid_core::secrets::{Credential, MemoryStore, SecretError, SecretStore};
use grid_core::session::SessionManager;
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
    mgr2.restore_romm().await.expect("restore should not error");
    let s = status(&mgr2);
    assert_eq!(s.connection, RommConnection::Connected);
    assert_eq!(s.server_url, server.uri());
    assert!(mgr2.client().is_some());
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
    mgr2.restore_romm().await.unwrap();
    let s = status(&mgr2);
    assert_eq!(s.connection, RommConnection::Connected);
    assert_eq!(s.server_url, expected);
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
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert_eq!(s.server_url, "");
    assert_eq!(s.username, "");
}

/// A Python import writes `server_url`/`username` into config.toml but no
/// credential, so RomM is Not connected — and the status must hand both
/// fields back for the sign-in form to prefill.
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
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert_eq!(s.server_url, "https://romm.example.test");
    assert_eq!(s.username, "importer");
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
    mgr2.restore_romm().await.unwrap();
    let s = status(&mgr2);
    assert_eq!(s.connection, RommConnection::Unreachable);
    assert_eq!(s.server_url, uri);
    assert!(!s.last_error.is_empty());
    assert!(mgr2.client().is_none());
    assert_eq!(mgr2.server_url(), uri);
    // A reconnect against a dead server stays Unreachable (the card's
    // "Can't reach" + Retry), never Expired.
    mgr2.reconnect().await.unwrap();
    let s = status(&mgr2);
    assert_eq!(s.connection, RommConnection::Unreachable);
    assert!(!s.last_error.is_empty());
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

// --- Q6: a rejected credential is "Sign-in expired" -----------------------

use grid_core::session::AuthKind;
use std::sync::Mutex;

/// A config + keyring pair as a previous run leaves them: the server URL and
/// verified username in config.toml, the credential in the store.
fn stored_session(
    dir: &tempfile::TempDir,
    server_url: &str,
    cred: Credential,
) -> (SessionManager, Arc<MemoryStore>) {
    let config_path = dir.path().join("config.toml");
    let mut cfg = grid_core::config::Config {
        server_url: server_url.into(),
        username: "six".into(),
        ..Default::default()
    };
    cfg.stores.romm.connected = true;
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
async fn restore_with_a_rejected_token_reports_expired_and_keeps_the_credential() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());

    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.username, "six");
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(mgr.client().is_none());
    // User decision Q6: the keyring item stays until a new connect succeeds.
    assert!(SecretStore::load(&*store).unwrap().is_some());
    // The IPC shape carries no secret.
    let wire = serde_json::to_string(&s).unwrap();
    assert!(!wire.contains("FAKE-TEST-TOKEN-not-real"), "{wire}");
    let wire = serde_json::to_value(&s).unwrap();
    assert_eq!(wire["connection"], "expired");
    assert_eq!(wire["auth_kind"], "token");
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
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.auth_kind, Some(AuthKind::Basic));
    let wire = serde_json::to_string(&s).unwrap();
    assert!(!wire.contains("FAKE-TEST-PASSWORD"), "{wire}");
}

/// 403 is "insufficient permission", not a dead credential: not Expired.
#[tokio::test]
async fn restore_with_forbidden_is_not_unauthorized() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 403).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Unreachable);
    assert!(s.last_error.contains("permission"), "{}", s.last_error);
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn restore_with_a_network_error_stays_unreachable() {
    let server = MockServer::builder().start().await;
    let uri = server.uri();
    drop(server);
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &uri, token());
    mgr.restore_romm().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Unreachable);
}

#[tokio::test]
async fn reconnect_with_a_rejected_token_reports_expired() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.reconnect().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.username, "six");
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(mgr.client().is_none());
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn reconnect_that_connects_reports_connected() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    mgr.reconnect().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    assert!(mgr.client().is_some());
}

#[tokio::test]
async fn a_mid_session_401_changes_the_status_once_and_keeps_the_credential() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    let calls = counting_hook(&mgr);

    let client = mgr.client().unwrap();
    let (a, b) = tokio::join!(client.platforms(), client.platforms());
    assert!(matches!(a, Err(grid_core::romm::RommError::Unauthorized)));
    assert!(matches!(b, Err(grid_core::romm::RommError::Unauthorized)));
    assert!(client.platforms().await.is_err());

    assert_eq!(
        *calls.lock().unwrap(),
        1,
        "one change per session, not per request"
    );
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.username, "six");
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(mgr.client().is_none(), "the dead session is dropped");
    assert!(SecretStore::load(&*store).unwrap().is_some());
}

#[tokio::test]
async fn a_mid_session_403_does_not_end_the_session() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 403).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let calls = counting_hook(&mgr);
    let client = mgr.client().unwrap();
    assert!(matches!(
        client.platforms().await,
        Err(grid_core::romm::RommError::Forbidden)
    ));
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    assert!(mgr.client().is_some());
}

/// A request still in flight on the old client must not throw the user out
/// of the session that replaced it.
#[tokio::test]
async fn a_401_on_a_replaced_client_does_not_end_the_session() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let old = mgr.client().unwrap();
    mgr.reconnect().await.unwrap();
    let calls = counting_hook(&mgr);
    assert!(old.platforms().await.is_err());
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    assert!(mgr.client().is_some());
}

/// A 401 on the connect probe is the sign-in form's own error, not a
/// mid-session "Sign-in expired".
#[tokio::test]
async fn a_rejected_connect_is_not_expired() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
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
    assert_eq!(status(&mgr).connection, RommConnection::NotConnected);
}

// --- RomM as a storefront (romm-storefront-plan 0.2, 0.4) ------------------

use grid_core::config::Config;
use grid_core::session::{RommConnection, RommStatus};

/// A `SecretStore` that fails the test if anything reads the keyring: a
/// not-connected start must never cause a secret-service unlock prompt.
#[derive(Default)]
struct NoReadStore;

impl SecretStore for NoReadStore {
    fn save(&self, _cred: &Credential) -> Result<(), SecretError> {
        panic!("the keyring was written")
    }
    fn load(&self) -> Result<Option<Credential>, SecretError> {
        panic!("the keyring was read")
    }
    fn clear(&self) -> Result<(), SecretError> {
        panic!("the keyring was cleared")
    }
}

/// Counts change-hook calls.
fn counting_hook(mgr: &SessionManager) -> Arc<Mutex<u32>> {
    let calls = Arc::new(Mutex::new(0));
    let sink = calls.clone();
    mgr.set_change_hook(Arc::new(move || *sink.lock().unwrap() += 1));
    calls
}

fn status(mgr: &SessionManager) -> RommStatus {
    mgr.status(0)
}

fn set_connected(dir: &tempfile::TempDir, connected: bool) {
    let path = dir.path().join("config.toml");
    let mut cfg = Config::load(&path).unwrap();
    cfg.stores.romm.connected = connected;
    cfg.save(&path).unwrap();
}

/// `stores.romm.connected = false`: restore makes no request, reads no
/// keyring, and RomM is Not connected with the stored server and user.
/// The mock matches every request and expects zero calls.
#[tokio::test]
async fn restore_while_not_connected_makes_no_request_and_reads_no_keyring() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut cfg = Config {
        server_url: server.uri(),
        username: "six".into(),
        library_path: "/lib".into(),
        ..Default::default()
    };
    cfg.setup.welcomed = true;
    cfg.save(&config_path).unwrap();
    let mgr = SessionManager::new(
        config_path,
        dir.path().join("covers"),
        Arc::new(NoReadStore),
    );
    mgr.prime().unwrap();
    assert!(!status(&mgr).busy, "nothing to restore");
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.username, "six");
    assert!(!s.busy);
    assert!(!s.in_rail);
    assert!(mgr.status(3).in_rail, "installed RomM games keep the group");
    assert!(mgr.client().is_none());
    // Image filtering still knows the configured server.
    assert_eq!(mgr.server_url(), server.uri());
    server.verify().await;
}

/// A migrated offline config ("Use without a server" before RomM became a
/// storefront) starts the same way: no probe, no keyring read.
#[tokio::test]
async fn a_migrated_offline_config_restores_without_the_keyring() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "schema_version = 1\nserver_url = \"https://romm.example\"\nusername = \"six\"\nlibrary_path = \"/lib\"\noffline = true\n",
    )
    .unwrap();
    let mgr = SessionManager::new(
        config_path,
        dir.path().join("covers"),
        Arc::new(NoReadStore),
    );
    mgr.restore_romm().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::NotConnected);
    assert_eq!(status(&mgr).server_url, "https://romm.example");
    assert!(mgr.client().is_none());
}

/// A migrated connected config restores with its stored credential, so a
/// connected user never signs in again.
#[tokio::test]
async fn a_migrated_connected_config_restores_with_the_stored_credential() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "schema_version = 1\nserver_url = \"{}\"\nusername = \"six\"\nlibrary_path = \"/lib\"\n",
            server.uri()
        ),
    )
    .unwrap();
    let store = Arc::new(MemoryStore::default());
    SecretStore::save(&*store, &token()).unwrap();
    let mgr = SessionManager::new(config_path, dir.path().join("covers"), store);
    mgr.prime().unwrap();
    assert!(status(&mgr).busy, "a stored connection reads as connecting");
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Connected);
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(!s.busy);
    assert!(s.in_rail);
    assert!(mgr.client().is_some());
}

/// Connected with nothing in the keyring: Not connected, with the stored
/// server and user for the sign-in form to prefill.
#[tokio::test]
async fn restore_connected_without_a_credential_is_not_connected() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut cfg = Config {
        server_url: "https://romm.example".into(),
        username: "six".into(),
        ..Default::default()
    };
    cfg.stores.romm.connected = true;
    cfg.setup.welcomed = true;
    cfg.save(&config_path).unwrap();
    let mgr = SessionManager::new(
        config_path,
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert_eq!(s.server_url, "https://romm.example");
    assert_eq!(s.username, "six");
}

#[tokio::test]
async fn the_status_follows_each_restore_outcome_and_fires_the_hook() {
    // Connected.
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    let calls = counting_hook(&mgr);
    mgr.restore_romm().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    assert!(*calls.lock().unwrap() >= 2, "busy on, then the result");

    // Expired (401): the auth kind is known, the keyring kept.
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(s.last_error.is_empty());
    assert!(SecretStore::load(&*store).unwrap().is_some());

    // Unreachable: the probe's reason, secret-free.
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 503).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Unreachable);
    assert!(!s.last_error.is_empty());
    assert!(!s.last_error.contains("FAKE-TEST-TOKEN-not-real"));
    assert!(!s.busy);
    let wire = serde_json::to_value(&s).unwrap();
    assert_eq!(wire["connection"], "unreachable");
}

#[tokio::test]
async fn a_mid_session_401_sets_expired_keeps_the_keyring_and_fires_the_hook() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    mount_status(&server, "/api/platforms", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let calls = counting_hook(&mgr);
    let client = mgr.client().unwrap();
    assert!(client.platforms().await.is_err());
    assert_eq!(*calls.lock().unwrap(), 1);
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Expired);
    assert_eq!(s.auth_kind, Some(AuthKind::Token));
    assert!(s.in_rail);
    assert!(SecretStore::load(&*store).unwrap().is_some());
    // The config still says connected: the next start probes (and says
    // "Sign-in expired" again) until the user signs in or disconnects.
    let cfg = Config::load(&dir.path().join("config.toml")).unwrap();
    assert!(cfg.stores.romm.connected);
}

#[tokio::test]
async fn connect_sets_connected_in_the_config_and_the_status() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let calls = counting_hook(&mgr);
    mgr.connect(
        server.uri(),
        String::new(),
        SecretString::from("FAKE-TEST-TOKEN-not-real"),
        false,
    )
    .await
    .unwrap();
    let cfg = Config::load(&dir.path().join("config.toml")).unwrap();
    assert!(cfg.stores.romm.connected);
    assert!(
        !cfg.setup.welcomed,
        "connect alone does not finish the welcome"
    );
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::Connected);
    assert_eq!(s.username, "six");
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.auth_kind, Some(AuthKind::Basic));
    assert!(!s.busy);
    assert_eq!(*calls.lock().unwrap(), 2, "busy on, then connected");
}

#[tokio::test]
async fn a_failed_connect_leaves_the_status_as_it_was() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 401).await;
    let dir = tempfile::tempdir().unwrap();
    let mgr = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    let calls = counting_hook(&mgr);
    assert!(mgr
        .connect(
            server.uri(),
            String::new(),
            SecretString::from("FAKE-TEST-TOKEN-not-real"),
            true,
        )
        .await
        .is_err());
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert!(!s.busy);
    assert!(s.server_url.is_empty());
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[tokio::test]
async fn disconnect_keeps_the_server_and_user_and_stops_probing() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    mgr.restore_romm().await.unwrap();
    let calls = counting_hook(&mgr);
    mgr.disconnect().unwrap();
    assert_eq!(*calls.lock().unwrap(), 1);
    assert!(mgr.client().is_none());
    assert!(SecretStore::load(&*store).unwrap().is_none());
    let s = status(&mgr);
    assert_eq!(s.connection, RommConnection::NotConnected);
    assert_eq!(s.server_url, server.uri());
    assert_eq!(s.username, "six");
    assert_eq!(s.auth_kind, None);
    let cfg = Config::load(&dir.path().join("config.toml")).unwrap();
    assert!(!cfg.stores.romm.connected);
    assert_eq!(cfg.server_url, server.uri());
    assert_eq!(cfg.username, "six");
    // The next start does not probe.
    let fresh = SessionManager::new(
        dir.path().join("config.toml"),
        dir.path().join("covers"),
        Arc::new(NoReadStore),
    );
    fresh.restore_romm().await.unwrap();
    assert_eq!(status(&fresh).connection, RommConnection::NotConnected);
}

#[tokio::test]
async fn reconnect_without_a_credential_is_no_stored_session() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let cfg = Config {
        server_url: "https://romm.example".into(),
        ..Default::default()
    };
    cfg.save(&config_path).unwrap();
    let mgr = SessionManager::new(
        config_path,
        dir.path().join("covers"),
        Arc::new(MemoryStore::default()),
    );
    assert!(matches!(
        mgr.reconnect().await,
        Err(grid_core::session::SessionError::NoStoredSession)
    ));
    assert!(!status(&mgr).busy);
}

/// Not connected with a credential still in the keyring (a migrated
/// "Use without a server" config): a reconnect that connects turns probing
/// back on, so the next start restores.
#[tokio::test]
async fn a_reconnect_that_connects_turns_probing_back_on() {
    let server = MockServer::start().await;
    mount_users_me(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, store) = stored_session(&dir, &server.uri(), token());
    set_connected(&dir, false);
    mgr.restore_romm().await.unwrap();
    assert!(mgr.client().is_none());
    assert_eq!(status(&mgr).connection, RommConnection::NotConnected);

    mgr.reconnect().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Connected);
    let config_path = dir.path().join("config.toml");
    assert!(Config::load(&config_path).unwrap().stores.romm.connected);
    let fresh = SessionManager::new(config_path, dir.path().join("covers"), store);
    fresh.restore_romm().await.unwrap();
    assert_eq!(status(&fresh).connection, RommConnection::Connected);
}

#[tokio::test]
async fn a_reconnect_that_fails_keeps_not_probing() {
    let server = MockServer::start().await;
    mount_status(&server, "/api/users/me", 503).await;
    let dir = tempfile::tempdir().unwrap();
    let (mgr, _store) = stored_session(&dir, &server.uri(), token());
    set_connected(&dir, false);
    mgr.reconnect().await.unwrap();
    assert_eq!(status(&mgr).connection, RommConnection::Unreachable);
    let cfg = Config::load(&dir.path().join("config.toml")).unwrap();
    assert!(!cfg.stores.romm.connected);
}
