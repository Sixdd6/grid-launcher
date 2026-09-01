# Rust/Tauri Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 + Svelte 5 app in `rewrite/` that connects to a RomM server and browses the library as a smooth, gamepad-navigable cover grid, on production-shaped foundations.

**Architecture:** Cargo workspace with a UI-agnostic `grid-core` library (config, secrets, RomM client, cover cache, session) and a thin Tauri shell (commands, gamepad thread, asset protocol). Svelte 5 frontend owns rendering and focus visuals only.

**Tech Stack:** Rust stable, Tauri 2, reqwest(rustls), secrecy, keyring, tokio, wiremock (tests), Svelte 5 + TypeScript + Vite, gilrs.

**Spec:** `docs/superpowers/specs/2026-08-31-rust-tauri-walking-skeleton-design.md`

## Global Constraints

- Tokens/passwords: only in OS keyring + `secrecy::SecretString` in memory. Never in config files, logs, errors, IPC payloads, fixtures, or test snapshots. Exactly ONE `expose_secret()` call site in the codebase (the Authorization header builder).
- `grid-core` must not depend on Tauri.
- TLS verification always on. No insecure-mode flags anywhere.
- Config writes are atomic: write `<file>.tmp`, then rename over target.
- RomM pagination: page size 200, loop until a short page (per docs/porting/01-romm-api.md).
- Gamepad: dead zone 0.3, repeat interval 0.2 s (per docs/porting/09-tv-mode.md).
- `cargo clippy --workspace -- -D warnings` and `cargo fmt --check` must pass at every commit.
- Test fixtures use the literal token `FAKE-TEST-TOKEN-not-real`. Nothing resembling a real credential is committed.
- All commands below run from `rewrite/` unless a path says otherwise. Rust deps are added with `cargo add` (resolves current versions); do not hand-pin versions unless a task says so.
- Commit messages: end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`. Never run git checkout/restore/reset/stash on tracked files.
- Design substitution vs spec: covers are served via Tauri's built-in **asset protocol** (`ensure_cover` command returns a cache path; frontend converts with `convertFileSrc`) instead of a custom `covers://` scheme. Same guarantees (auth Rust-side, no base64 IPC, browser-native decode), less code.

---

### Task 1: Workspace scaffold

**Files:**
- Create: `rewrite/Cargo.toml`, `rewrite/rustfmt.toml`, `rewrite/crates/grid-core/Cargo.toml`, `rewrite/crates/grid-core/src/lib.rs`
- Modify: `.gitignore` (repo root)

**Interfaces:**
- Produces: workspace layout every later task builds in; `grid_core` crate name.

- [ ] **Step 1: Create workspace files**

`rewrite/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/grid-core"]

[workspace.package]
edition = "2021"
license = "MIT"
```

`rewrite/rustfmt.toml`:
```toml
edition = "2021"
```

`rewrite/crates/grid-core/Cargo.toml`:
```toml
[package]
name = "grid-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]

[dev-dependencies]
```

`rewrite/crates/grid-core/src/lib.rs`:
```rust
//! GRID Launcher core library: config, secrets, RomM client, covers, session.
//! UI-agnostic — this crate must never depend on Tauri.
```

- [ ] **Step 2: Ignore build outputs**

Append to the repo-root `.gitignore`:
```
# Rust rewrite
rewrite/target/
rewrite/app/node_modules/
rewrite/app/dist/
```

- [ ] **Step 3: Verify the workspace builds**

Run: `cd rewrite && cargo build && cargo fmt --check && cargo clippy --workspace -- -D warnings`
Expected: all succeed (empty crate).

- [ ] **Step 4: Commit**

```bash
git add rewrite/ .gitignore
git commit -m "rewrite: scaffold Cargo workspace with grid-core crate"
```

---

### Task 2: grid-core config (TOML, atomic writes, unknown-key preservation)

**Files:**
- Create: `rewrite/crates/grid-core/src/config.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`, `rewrite/crates/grid-core/Cargo.toml`
- Test: inline `#[cfg(test)]` in `config.rs`

**Interfaces:**
- Produces: `Config { schema_version: u32, server_url: String, username: String }`, `Config::load(path) -> Result<Config, ConfigError>`, `Config::save(&self, path) -> Result<(), ConfigError>`, `Config::default_path() -> PathBuf`.

- [ ] **Step 1: Add dependencies**

Run: `cargo add -p grid-core serde --features derive` then `cargo add -p grid-core toml thiserror directories` and `cargo add -p grid-core --dev tempfile`

- [ ] **Step 2: Write failing tests**

In `config.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            schema_version: 1,
            server_url: "https://romm.example".into(),
            username: "six".into(),
            extra: Default::default(),
        };
        cfg.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), cfg);
    }

    #[test]
    fn missing_file_yields_default() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(&dir.path().join("nope.toml")).unwrap();
        assert_eq!(cfg.schema_version, 1);
        assert_eq!(cfg.server_url, "");
    }

    #[test]
    fn preserves_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "schema_version = 1\nserver_url = \"s\"\nusername = \"u\"\nfuture_key = \"kept\"\n").unwrap();
        let cfg = Config::load(&path).unwrap();
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("future_key"));
    }

    #[test]
    fn save_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Config::default().save(&path).unwrap();
        assert!(!dir.path().join("config.toml.tmp").exists());
        assert!(path.exists());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p grid-core config`
Expected: compile failure — `Config` not defined.

- [ ] **Step 4: Implement**

`config.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// App configuration. Secrets are NEVER part of this struct — they live in
/// the OS keyring only (see secrets.rs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub username: String,
    /// Unknown keys survive load/save round trips for forward compatibility.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self { schema_version: 1, server_url: String::new(), username: String::new(), extra: BTreeMap::new() }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
            .expect("home directory must exist")
            .config_dir()
            .join("config.toml")
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic: write `<path>.tmp`, then rename over the target.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}
```

In `lib.rs` add: `pub mod config;`

- [ ] **Step 5: Run tests, fmt, clippy**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`
Expected: 4 tests pass. Note: `save_leaves_no_tmp_file` checks `config.toml.tmp`; `with_extension("toml.tmp")` produces exactly that.

- [ ] **Step 6: Commit**

```bash
git add rewrite/
git commit -m "rewrite: grid-core config with atomic writes and unknown-key preservation"
```

---

### Task 3: grid-core secrets (keyring + SecretString, redaction proven)

**Files:**
- Create: `rewrite/crates/grid-core/src/secrets.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`
- Test: inline `#[cfg(test)]`

**Interfaces:**
- Produces: `Credential` enum (`Token(SecretString)` | `Basic { username: String, password: SecretString }`), `SecretStore` trait (`save(&self, cred: &Credential)`, `load(&self) -> Result<Option<Credential>, SecretError>`, `clear(&self)`), `KeyringStore::new()`, `MemoryStore::default()` (test double).

- [ ] **Step 1: Add dependencies**

Run: `cargo add -p grid-core secrecy keyring serde_json`
(keyring needs its platform feature on Linux: check `cargo add keyring` output; if features are required, use `cargo add -p grid-core keyring --features linux-native-sync-persistent` or the current documented default for Secret Service. Verify with `cargo build`.)

- [ ] **Step 2: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    #[test]
    fn debug_output_redacts_secrets() {
        let cred = Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"));
        let debug = format!("{cred:?}");
        assert!(!debug.contains("FAKE-TEST-TOKEN-not-real"), "leak: {debug}");
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        store
            .save(&Credential::Basic { username: "six".into(), password: SecretString::from("pw-FAKE") })
            .unwrap();
        match store.load().unwrap() {
            Some(Credential::Basic { username, .. }) => assert_eq!(username, "six"),
            other => panic!("wrong credential: {other:?}"),
        }
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p grid-core secrets`
Expected: compile failure.

- [ ] **Step 4: Implement**

```rust
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("secret encoding: {0}")]
    Encoding(String),
}

/// A server credential. Debug is derived but the SecretString fields render
/// as redacted by the secrecy crate, so this type is safe to log by accident.
#[derive(Debug, Clone)]
pub enum Credential {
    Token(SecretString),
    Basic { username: String, password: SecretString },
}

pub trait SecretStore: Send + Sync {
    fn save(&self, cred: &Credential) -> Result<(), SecretError>;
    fn load(&self) -> Result<Option<Credential>, SecretError>;
    fn clear(&self) -> Result<(), SecretError>;
}

const SERVICE: &str = "grid-launcher";
const ACCOUNT: &str = "romm-credential";

/// Serialized form kept ONLY inside the OS keyring item.
#[derive(serde::Serialize, serde::Deserialize)]
enum StoredCredential {
    Token { token: String },
    Basic { username: String, password: String },
}

pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        Self
    }
    fn entry() -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| SecretError::Keyring(e.to_string()))
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn save(&self, cred: &Credential) -> Result<(), SecretError> {
        let stored = match cred {
            Credential::Token(t) => StoredCredential::Token { token: t.expose_secret().to_string() },
            Credential::Basic { username, password } => StoredCredential::Basic {
                username: username.clone(),
                password: password.expose_secret().to_string(),
            },
        };
        let json = serde_json::to_string(&stored).map_err(|e| SecretError::Encoding(e.to_string()))?;
        Self::entry()?.set_password(&json).map_err(|e| SecretError::Keyring(e.to_string()))
    }

    fn load(&self) -> Result<Option<Credential>, SecretError> {
        let json = match Self::entry()?.get_password() {
            Ok(j) => j,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(SecretError::Keyring(e.to_string())),
        };
        let stored: StoredCredential =
            serde_json::from_str(&json).map_err(|e| SecretError::Encoding(e.to_string()))?;
        Ok(Some(match stored {
            StoredCredential::Token { token } => Credential::Token(SecretString::from(token)),
            StoredCredential::Basic { username, password } => {
                Credential::Basic { username, password: SecretString::from(password) }
            }
        }))
    }

    fn clear(&self) -> Result<(), SecretError> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }
}

/// In-memory store for tests and for the Tauri layer's unit tests.
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<Option<Credential>>);

impl SecretStore for MemoryStore {
    fn save(&self, cred: &Credential) -> Result<(), SecretError> {
        *self.0.lock().unwrap() = Some(cred.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<Credential>, SecretError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn clear(&self) -> Result<(), SecretError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
```

NOTE: `KeyringStore::save` contains `expose_secret()` calls. These plus the
Authorization builder (Task 4) are the ONLY permitted call sites; the CI guard
(Task 10) enforces the list. In `lib.rs` add: `pub mod secrets;`

- [ ] **Step 5: Run tests, fmt, clippy; commit**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`
Expected: PASS.

```bash
git add rewrite/
git commit -m "rewrite: grid-core secret store with keyring backend and redacting types"
```

---

### Task 4: grid-core RomM client — errors, auth, connect probe

**Files:**
- Create: `rewrite/crates/grid-core/src/romm/mod.rs`, `rewrite/crates/grid-core/src/romm/error.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`
- Test: `rewrite/crates/grid-core/tests/romm_client.rs`

**Interfaces:**
- Consumes: `Credential` from Task 3.
- Produces: `RommClient::new(base_url: &str, cred: Credential) -> Result<RommClient, RommError>`, `RommClient::connect(&self) -> Result<UserInfo, RommError>` (async), `UserInfo { id: i64, username: String }`, `RommError` enum with `Display` guaranteed free of credentials.

- [ ] **Step 1: Read the auth reference**

Read `docs/porting/01-romm-api.md` sections on auth header construction and failure classification, and grep `openapi.json` for the `/api/users/me` response schema. The client sends `Authorization: Bearer <token>` for `Credential::Token` and HTTP basic for `Credential::Basic` — same two modes as the reference.

- [ ] **Step 2: Add dependencies**

Run: `cargo add -p grid-core reqwest --no-default-features --features rustls-tls,json` then `cargo add -p grid-core tokio --features rt-multi-thread,macros` then `cargo add -p grid-core url base64` then `cargo add -p grid-core --dev wiremock`

- [ ] **Step 3: Write failing tests**

`tests/romm_client.rs`:
```rust
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
```

Also add `cargo add -p grid-core --dev serde_json secrecy` if not already dev-visible (secrecy is a normal dep from Task 3; serde_json likewise).

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p grid-core --test romm_client`
Expected: compile failure — `romm` module missing.

- [ ] **Step 5: Implement**

`romm/error.rs`:
```rust
/// Errors are user-presentable and MUST NEVER embed the request or its
/// headers. Body excerpts are capped and come from the server response only.
#[derive(Debug, thiserror::Error)]
pub enum RommError {
    #[error("invalid server URL")]
    InvalidUrl,
    #[error("could not reach the server: {0}")]
    Connection(String),
    #[error("the server rejected the credentials")]
    Unauthorized,
    #[error("server error {status}: {excerpt}")]
    Http { status: u16, excerpt: String },
    #[error("unexpected response from the server: {0}")]
    Decode(String),
}

pub(crate) fn excerpt(body: &str) -> String {
    const MAX: usize = 240;
    let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() > MAX {
        format!("{}...", &collapsed[..MAX])
    } else {
        collapsed
    }
}
```

`romm/mod.rs`:
```rust
mod error;
pub use error::RommError;

use crate::secrets::Credential;
use base64::Engine;
use secrecy::ExposeSecret;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

pub struct RommClient {
    http: reqwest::Client,
    base: url::Url,
    /// Prebuilt Authorization header value. Held as a reqwest HeaderValue
    /// marked sensitive so reqwest's own debug output redacts it.
    auth: reqwest::header::HeaderValue,
}

impl RommClient {
    /// The ONLY place (besides KeyringStore serialization) where a secret is
    /// exposed. Builds the Authorization header value once.
    pub fn new(base_url: &str, cred: Credential) -> Result<Self, RommError> {
        let base = url::Url::parse(base_url).map_err(|_| RommError::InvalidUrl)?;
        let raw = match &cred {
            Credential::Token(t) => format!("Bearer {}", t.expose_secret()),
            Credential::Basic { username, password } => {
                let joined = format!("{username}:{}", password.expose_secret());
                format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(joined))
            }
        };
        let mut auth = reqwest::header::HeaderValue::from_str(&raw)
            .map_err(|_| RommError::InvalidUrl)?;
        auth.set_sensitive(true);
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| RommError::Connection(e.to_string()))?;
        Ok(Self { http, base, auth })
    }

    fn endpoint(&self, path: &str) -> Result<url::Url, RommError> {
        self.base.join(path).map_err(|_| RommError::InvalidUrl)
    }

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, RommError> {
        let resp = self
            .http
            .get(self.endpoint(path)?)
            .query(query)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(RommError::Unauthorized);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RommError::Http { status: status.as_u16(), excerpt: error::excerpt(&body) });
        }
        resp.json::<T>().await.map_err(|e| RommError::Decode(e.without_url().to_string()))
    }

    pub async fn connect(&self) -> Result<UserInfo, RommError> {
        self.get_json("/api/users/me", &[]).await
    }
}
```

In `lib.rs` add: `pub mod romm;`
(`reqwest::Error::without_url()` strips the URL, which can carry the host but never credentials; header is `set_sensitive`, so `{:?}` of requests redacts it.)

- [ ] **Step 6: Run tests, fmt, clippy; commit**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`
Expected: PASS (3 new tests).

```bash
git add rewrite/
git commit -m "rewrite: RomM client with connect probe and credential-free errors"
```

---

### Task 5: grid-core RomM client — platforms and paginated roms

**Files:**
- Modify: `rewrite/crates/grid-core/src/romm/mod.rs`
- Test: `rewrite/crates/grid-core/tests/romm_catalog.rs`

**Interfaces:**
- Consumes: `RommClient`, `get_json` from Task 4.
- Produces: `Platform { id: i64, name: String, slug: String, rom_count: i64 }`, `GameSummary { id: i64, name: String, platform_id: i64, cover_path: Option<String> }`, `RommClient::platforms() -> Result<Vec<Platform>, RommError>`, `RommClient::games(platform_id: i64) -> Result<Vec<GameSummary>, RommError>`.

- [ ] **Step 1: Check field names against the spec**

Grep `openapi.json` for the `/api/platforms` and `/api/roms` response schemas and `docs/porting/01-romm-api.md` rows 2–3 (query params `platform_id`, `limit`, `offset`; paged response shape `{ items: [...], total: N }` — confirm the exact envelope key names in openapi.json 5.2.0 and adjust the structs below to match reality, not the plan).

- [ ] **Step 2: Write failing tests**

`tests/romm_catalog.rs`:
```rust
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(uri: &str) -> RommClient {
    RommClient::new(uri, Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))).unwrap()
}

fn rom(id: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id, "name": format!("Game {id}"), "platform_id": 7,
        "path_cover_small": format!("/assets/romm/resources/roms/{id}/cover/small.png")
    })
}

#[tokio::test]
async fn platforms_skips_zero_rom_entries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/platforms"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": 7, "name": "SNES", "slug": "snes", "rom_count": 12},
            {"id": 8, "name": "Empty", "slug": "empty", "rom_count": 0}
        ])))
        .mount(&server)
        .await;
    let platforms = client(&server.uri()).platforms().await.unwrap();
    assert_eq!(platforms.len(), 1);
    assert_eq!(platforms[0].slug, "snes");
}

#[tokio::test]
async fn games_paginate_until_short_page() {
    let server = MockServer::start().await;
    let page1: Vec<_> = (0..200).map(rom).collect();
    let page2: Vec<_> = (200..250).map(rom).collect();
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"items": page1, "total": 250})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/roms"))
        .and(query_param("offset", "200"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({"items": page2, "total": 250})))
        .mount(&server)
        .await;
    let games = client(&server.uri()).games(7).await.unwrap();
    assert_eq!(games.len(), 250);
    assert_eq!(games[0].name, "Game 0");
    assert!(games[0].cover_path.as_deref().unwrap().contains("/cover/"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p grid-core --test romm_catalog`
Expected: compile failure — `platforms`/`games` missing.

- [ ] **Step 4: Implement**

Append to `romm/mod.rs`:
```rust
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Platform {
    pub id: i64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub rom_count: i64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct GameSummary {
    pub id: i64,
    pub name: String,
    pub platform_id: i64,
    /// Server-relative cover path (small variant), when present.
    #[serde(rename = "path_cover_small")]
    pub cover_path: Option<String>,
}

#[derive(Deserialize)]
struct Paged<T> {
    items: Vec<T>,
}

const PAGE_SIZE: usize = 200;

impl RommClient {
    pub async fn platforms(&self) -> Result<Vec<Platform>, RommError> {
        let all: Vec<Platform> = self.get_json("/api/platforms", &[]).await?;
        Ok(all.into_iter().filter(|p| p.rom_count > 0).collect())
    }

    /// Pages through /api/roms with limit 200 until a short page, per
    /// docs/porting/01-romm-api.md.
    pub async fn games(&self, platform_id: i64) -> Result<Vec<GameSummary>, RommError> {
        let mut out = Vec::new();
        let mut offset = 0usize;
        loop {
            let page: Paged<GameSummary> = self
                .get_json(
                    "/api/roms",
                    &[
                        ("platform_id", platform_id.to_string()),
                        ("limit", PAGE_SIZE.to_string()),
                        ("offset", offset.to_string()),
                    ],
                )
                .await?;
            let n = page.items.len();
            out.extend(page.items);
            if n < PAGE_SIZE {
                break;
            }
            offset += PAGE_SIZE;
        }
        Ok(out)
    }
}
```

Adjust field/envelope names to whatever Step 1 found in `openapi.json` — the tests' JSON must mirror the real schema, then the structs must match the tests.

- [ ] **Step 5: Run tests, fmt, clippy; commit**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`

```bash
git add rewrite/
git commit -m "rewrite: RomM platforms and paginated games listing"
```

---

### Task 6: grid-core cover cache

**Files:**
- Create: `rewrite/crates/grid-core/src/covers.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`, `rewrite/crates/grid-core/src/romm/mod.rs`
- Test: `rewrite/crates/grid-core/tests/covers.rs`

**Interfaces:**
- Consumes: `RommClient` (adds `get_bytes`), `GameSummary.cover_path`.
- Produces: `CoverCache::new(dir: PathBuf)`, `CoverCache::ensure(&self, client: &RommClient, game_id: i64, cover_path: &str) -> Result<PathBuf, RommError>` (async; fetch-on-miss, in-flight dedup), `cover_key(game_id) -> String` (sha256 hex).

- [ ] **Step 1: Add dependencies**

Run: `cargo add -p grid-core sha2` and `cargo add -p grid-core tokio --features sync` (Mutex for the in-flight map).

- [ ] **Step 2: Write failing tests**

`tests/covers.rs`:
```rust
use grid_core::covers::{cover_key, CoverCache};
use grid_core::romm::RommClient;
use grid_core::secrets::Credential;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[test]
fn cover_key_is_stable_sha256() {
    assert_eq!(cover_key(42), cover_key(42));
    assert_eq!(cover_key(42).len(), 64);
    assert_ne!(cover_key(42), cover_key(43));
}

#[tokio::test]
async fn ensure_fetches_once_then_hits_cache() {
    let server = MockServer::start().await;
    let mock = Mock::given(method("GET"))
        .and(path("/assets/cover.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG_MAGIC))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let cache = CoverCache::new(dir.path().to_path_buf());
    let client = RommClient::new(&server.uri(), Credential::Token(SecretString::from("FAKE-TEST-TOKEN-not-real"))).unwrap();

    let first = cache.ensure(&client, 42, "/assets/cover.png").await.unwrap();
    let second = cache.ensure(&client, 42, "/assets/cover.png").await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.extension().unwrap(), "png");
    assert!(first.starts_with(dir.path()));
    drop(mock); // expect(1) verified on drop: second call hit the disk cache
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p grid-core --test covers`
Expected: compile failure.

- [ ] **Step 4: Implement**

Add to `romm/mod.rs`:
```rust
impl RommClient {
    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, RommError> {
        let resp = self
            .http
            .get(self.endpoint(path)?)
            .header(reqwest::header::AUTHORIZATION, self.auth.clone())
            .send()
            .await
            .map_err(|e| RommError::Connection(e.without_url().to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(RommError::Http { status: status.as_u16(), excerpt: String::new() });
        }
        Ok(resp.bytes().await.map_err(|e| RommError::Connection(e.without_url().to_string()))?.to_vec())
    }
}
```

`covers.rs`:
```rust
use crate::romm::{RommClient, RommError};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// One filename scheme for the whole cache: sha256 of the game id.
pub fn cover_key(game_id: i64) -> String {
    let mut h = Sha256::new();
    h.update(game_id.to_le_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sniff_extension(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xFF, 0xD8, 0xFF, ..] => "jpg",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        _ => "img",
    }
}

pub struct CoverCache {
    dir: PathBuf,
    in_flight: Arc<Mutex<HashMap<String, Arc<tokio::sync::Notify>>>>,
}

impl CoverCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, in_flight: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn find_existing(&self, key: &str) -> Option<PathBuf> {
        for ext in ["png", "jpg", "webp", "gif", "img"] {
            let p = self.dir.join(format!("{key}.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    /// Fetch-on-miss with in-flight deduplication: concurrent calls for the
    /// same key wait for the first fetch instead of re-downloading.
    pub async fn ensure(
        &self,
        client: &RommClient,
        game_id: i64,
        cover_path: &str,
    ) -> Result<PathBuf, RommError> {
        let key = cover_key(game_id);
        loop {
            if let Some(p) = self.find_existing(&key) {
                return Ok(p);
            }
            let notify = {
                let mut map = self.in_flight.lock().await;
                if let Some(existing) = map.get(&key) {
                    // Someone else is fetching: wait, then re-check the disk.
                    Some(existing.clone())
                } else {
                    map.insert(key.clone(), Arc::new(tokio::sync::Notify::new()));
                    None
                }
            };
            if let Some(n) = notify {
                n.notified().await;
                continue;
            }
            // We own the fetch.
            let result = self.fetch_and_store(client, &key, cover_path).await;
            let n = self.in_flight.lock().await.remove(&key);
            if let Some(n) = n {
                n.notify_waiters();
            }
            return result;
        }
    }

    async fn fetch_and_store(
        &self,
        client: &RommClient,
        key: &str,
        cover_path: &str,
    ) -> Result<PathBuf, RommError> {
        let bytes = client.get_bytes(cover_path).await?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| RommError::Connection(e.to_string()))?;
        let target = self.dir.join(format!("{key}.{}", sniff_extension(&bytes)));
        let tmp = target.with_extension("part");
        std::fs::write(&tmp, &bytes).map_err(|e| RommError::Connection(e.to_string()))?;
        std::fs::rename(&tmp, &target).map_err(|e| RommError::Connection(e.to_string()))?;
        Ok(target)
    }
}
```

In `lib.rs` add: `pub mod covers;`

- [ ] **Step 5: Run tests, fmt, clippy; commit**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`

```bash
git add rewrite/
git commit -m "rewrite: cover cache with single hash scheme and in-flight dedup"
```

---

### Task 7: grid-core session

**Files:**
- Create: `rewrite/crates/grid-core/src/session.rs`
- Modify: `rewrite/crates/grid-core/src/lib.rs`
- Test: inline `#[cfg(test)]` plus `rewrite/crates/grid-core/tests/session.rs`

**Interfaces:**
- Consumes: `Config`, `SecretStore`, `Credential`, `RommClient`, `CoverCache`.
- Produces: `SessionManager::new(config_path: PathBuf, cache_dir: PathBuf, secrets: Arc<dyn SecretStore>)`, `SessionManager::connect(&self, server_url: String, username: String, secret: SecretString, use_token: bool) -> Result<SessionState, SessionError>` (async; saves config + credential on success), `SessionManager::restore(&self) -> Result<Option<SessionState>, SessionError>` (reconnect from saved config + keyring), `SessionManager::disconnect(&self)` (clears credential), `SessionManager::client(&self) -> Option<Arc<RommClient>>`, `SessionManager::cache(&self) -> &CoverCache`, `SessionState { connected: bool, username: String, server_url: String }` (serde Serialize — this is the ONLY shape that crosses IPC).

- [ ] **Step 1: Write failing test**

`tests/session.rs`:
```rust
use grid_core::secrets::MemoryStore;
use grid_core::session::SessionManager;
use secrecy::SecretString;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    let mgr = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), store.clone());

    let state = mgr
        .connect(server.uri(), "six".into(), SecretString::from("FAKE-TEST-TOKEN-not-real"), true)
        .await
        .unwrap();
    assert!(state.connected);
    assert_eq!(state.username, "six");

    // A fresh manager over the same config path + store restores the session.
    let mgr2 = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), store);
    let restored = mgr2.restore().await.unwrap().expect("session should restore");
    assert!(restored.connected);
    assert_eq!(restored.server_url, server.uri());
}

#[tokio::test]
async fn disconnect_clears_credential() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    let mgr = SessionManager::new(dir.path().join("config.toml"), dir.path().join("covers"), store.clone());
    mgr.disconnect().unwrap();
    use grid_core::secrets::SecretStore;
    assert!(store.load().unwrap().is_none());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p grid-core --test session`
Expected: compile failure.

- [ ] **Step 3: Implement**

`session.rs`:
```rust
use crate::config::Config;
use crate::covers::CoverCache;
use crate::romm::{RommClient, RommError};
use crate::secrets::{Credential, SecretError, SecretStore};
use secrecy::SecretString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Romm(#[from] RommError),
    #[error("config: {0}")]
    Config(#[from] crate::config::ConfigError),
    #[error("secrets: {0}")]
    Secrets(#[from] SecretError),
}

/// The only session shape that may cross the IPC boundary. No secrets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionState {
    pub connected: bool,
    pub username: String,
    pub server_url: String,
}

pub struct SessionManager {
    config_path: PathBuf,
    secrets: Arc<dyn SecretStore>,
    cache: CoverCache,
    client: Mutex<Option<Arc<RommClient>>>,
}

impl SessionManager {
    pub fn new(config_path: PathBuf, cache_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self { config_path, secrets, cache: CoverCache::new(cache_dir), client: Mutex::new(None) }
    }

    pub fn cache(&self) -> &CoverCache {
        &self.cache
    }

    pub fn client(&self) -> Option<Arc<RommClient>> {
        self.client.lock().unwrap().clone()
    }

    /// `use_token`: true = `secret` is an API token; false = it is the
    /// account password (HTTP basic). On success the config and credential
    /// are persisted; the plain secret is consumed and dropped here.
    pub async fn connect(
        &self,
        server_url: String,
        username: String,
        secret: SecretString,
        use_token: bool,
    ) -> Result<SessionState, SessionError> {
        let cred = if use_token {
            Credential::Token(secret)
        } else {
            Credential::Basic { username: username.clone(), password: secret }
        };
        let state = self.try_connect(&server_url, &username, cred.clone()).await?;
        let mut cfg = Config::load(&self.config_path)?;
        cfg.server_url = server_url;
        cfg.username = username;
        cfg.save(&self.config_path)?;
        self.secrets.save(&cred)?;
        Ok(state)
    }

    pub async fn restore(&self) -> Result<Option<SessionState>, SessionError> {
        let cfg = Config::load(&self.config_path)?;
        if cfg.server_url.is_empty() {
            return Ok(None);
        }
        let Some(cred) = self.secrets.load()? else { return Ok(None) };
        Ok(Some(self.try_connect(&cfg.server_url, &cfg.username, cred).await?))
    }

    async fn try_connect(
        &self,
        server_url: &str,
        username: &str,
        cred: Credential,
    ) -> Result<SessionState, SessionError> {
        let client = RommClient::new(server_url, cred)?;
        let user = client.connect().await?;
        *self.client.lock().unwrap() = Some(Arc::new(client));
        Ok(SessionState {
            connected: true,
            username: if user.username.is_empty() { username.to_string() } else { user.username },
            server_url: server_url.to_string(),
        })
    }

    pub fn disconnect(&self) -> Result<(), SessionError> {
        *self.client.lock().unwrap() = None;
        self.secrets.clear()?;
        Ok(())
    }
}
```

In `lib.rs` add: `pub mod session;`

- [ ] **Step 4: Run tests, fmt, clippy; commit**

Run: `cargo test -p grid-core && cargo fmt --check && cargo clippy --workspace -- -D warnings`

```bash
git add rewrite/
git commit -m "rewrite: session manager with connect/restore/disconnect"
```

---

### Task 8: Tauri app scaffold (Svelte 5 + Vite + Tauri 2)

**Files:**
- Create: `rewrite/app/` (Vite + Svelte project), `rewrite/app/src-tauri/` (Tauri project)
- Modify: `rewrite/Cargo.toml` (add workspace member)

**Interfaces:**
- Produces: a building desktop app shell; `app/src-tauri/src/lib.rs` `run()` entry later tasks extend; `npm run tauri dev` works.

- [ ] **Step 1: Check current scaffolding commands**

Query context7 (`/tauri-apps/tauri-docs`) for "create-tauri-app with an existing Vite frontend" and for the current `tauri.conf.json` v2 schema keys used below. Use current reality over this plan if they differ.

- [ ] **Step 2: Scaffold**

From `rewrite/`:
```bash
npm create vite@latest app -- --template svelte-ts
cd app && npm install
npm install -D @tauri-apps/cli
npm install @tauri-apps/api
npx tauri init --app-name grid-launcher --window-title "GRID Launcher" \
  --frontend-dist ../dist --dev-url http://localhost:5173 \
  --before-dev-command "npm run dev" --before-build-command "npm run build"
```
Then in `rewrite/Cargo.toml` add `"app/src-tauri"` to `members`. In `app/src-tauri/Cargo.toml` add `grid-core = { path = "../../crates/grid-core" }`. Set the identifier in `app/src-tauri/tauri.conf.json` to `"io.github.sixdd6.gridlauncher2"` (distinct from the Python app).

- [ ] **Step 3: Enable the asset protocol for the cover cache**

In `app/src-tauri/tauri.conf.json` under `app.security`:
```json
{
  "assetProtocol": { "enable": true, "scope": ["$CACHE/grid-launcher/covers/**"] },
  "csp": {
    "default-src": "'self'",
    "img-src": "'self' asset: http://asset.localhost",
    "connect-src": "ipc: http://ipc.localhost",
    "style-src": "'unsafe-inline' 'self'"
  }
}
```
Confirm the `$CACHE` variable name against current Tauri docs (Step 1); the scope must resolve to the same directory `CoverCache` uses (the `directories` crate cache dir for `io.github/Sixdd6/grid-launcher`). If the variable set differs, list the absolute-path glob that matches.

- [ ] **Step 4: Verify dev build**

Run: `cd rewrite/app && npm run build && cd .. && cargo build`
Expected: frontend builds; workspace (now including src-tauri) compiles.

- [ ] **Step 5: Commit**

```bash
git add rewrite/ && git commit -m "rewrite: scaffold Tauri 2 + Svelte 5 app shell"
```

---

### Task 9: Tauri commands and state

**Files:**
- Create: `rewrite/app/src-tauri/src/commands.rs`
- Modify: `rewrite/app/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `SessionManager`, `SessionState`, `Platform`, `GameSummary`, `CoverCache::ensure`, `KeyringStore`.
- Produces (frontend contract, camelCase via serde): commands `connect(serverUrl, username, secret, useToken) -> SessionState`, `restore_session() -> SessionState | null`, `disconnect()`, `list_platforms() -> Platform[]`, `list_games(platformId) -> GameSummary[]`, `ensure_cover(gameId, coverPath) -> string (absolute path)`. All errors cross IPC as plain user-presentable strings (RommError Display), never structured internals.

- [ ] **Step 1: Implement commands**

`commands.rs`:
```rust
use grid_core::romm::{GameSummary, Platform};
use grid_core::session::{SessionManager, SessionState};
use secrecy::SecretString;
use tauri::State;

pub struct AppState {
    pub session: SessionManager,
}

fn err(e: impl std::fmt::Display) -> String {
    // RommError/SessionError Display are credential-free by construction.
    e.to_string()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    secret: String,
    use_token: bool,
) -> Result<SessionState, String> {
    // Wrap immediately; the plain String is dropped at the end of this scope.
    let secret = SecretString::from(secret);
    state.session.connect(server_url, username, secret, use_token).await.map_err(err)
}

#[tauri::command]
pub async fn restore_session(state: State<'_, AppState>) -> Result<Option<SessionState>, String> {
    state.session.restore().await.map_err(err)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    state.session.disconnect().map_err(err)
}

#[tauri::command]
pub async fn list_platforms(state: State<'_, AppState>) -> Result<Vec<Platform>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.platforms().await.map_err(err)
}

#[tauri::command]
pub async fn list_games(state: State<'_, AppState>, platform_id: i64) -> Result<Vec<GameSummary>, String> {
    let client = state.session.client().ok_or("not connected")?;
    client.games(platform_id).await.map_err(err)
}

#[tauri::command]
pub async fn ensure_cover(
    state: State<'_, AppState>,
    game_id: i64,
    cover_path: String,
) -> Result<String, String> {
    let client = state.session.client().ok_or("not connected")?;
    let path = state.session.cache().ensure(&client, game_id, &cover_path).await.map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}
```

`lib.rs` `run()`:
```rust
mod commands;

use commands::AppState;
use grid_core::config::Config;
use grid_core::secrets::KeyringStore;
use grid_core::session::SessionManager;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging policy (spec, normative): default filter carries no request or
    // header data anywhere; secrets are structurally unloggable (SecretString).
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
    ).init();
    let cache_dir = directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
        .expect("home directory must exist")
        .cache_dir()
        .join("covers");
    let session = SessionManager::new(Config::default_path(), cache_dir, Arc::new(KeyringStore::new()));
    tauri::Builder::default()
        .manage(AppState { session })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::restore_session,
            commands::disconnect,
            commands::list_platforms,
            commands::list_games,
            commands::ensure_cover,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```
Add deps to `app/src-tauri/Cargo.toml`: `cargo add -p grid-launcher directories tracing tracing-subscriber --features tracing-subscriber/env-filter` (use the src-tauri package's real name from its Cargo.toml).

- [ ] **Step 2: Verify it compiles clean**

Run: `cd rewrite && cargo build && cargo clippy --workspace -- -D warnings && cargo fmt --check`

- [ ] **Step 3: Commit**

```bash
git add rewrite/ && git commit -m "rewrite: Tauri commands for session, catalog, and covers"
```

---

### Task 10: Gamepad input (pure logic + gilrs thread)

**Files:**
- Create: `rewrite/app/src-tauri/src/gamepad/mod.rs`, `rewrite/app/src-tauri/src/gamepad/mapping.rs`
- Modify: `rewrite/app/src-tauri/src/lib.rs`
- Test: inline `#[cfg(test)]` in `mapping.rs`

**Interfaces:**
- Consumes: doc 09 event vocabulary.
- Produces: Tauri event `"nav"` with payload `{"action": "up"|"down"|"left"|"right"|"accept"|"back"}`; pure `mapping::Mapper` (`Mapper::new(dead_zone: f32, repeat: Duration)`, `Mapper::button(&mut self, btn: Button, pressed: bool, now: Instant) -> Option<NavAction>`, `Mapper::axis(&mut self, axis: Axis, value: f32, now: Instant) -> Option<NavAction>`), local `Button`/`Axis` enums decoupled from gilrs.

- [ ] **Step 1: Write failing tests for the pure mapper**

`mapping.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn mapper() -> Mapper {
        Mapper::new(0.3, Duration::from_millis(200))
    }

    #[test]
    fn dpad_press_maps_and_release_does_not() {
        let mut m = mapper();
        let t = Instant::now();
        assert_eq!(m.button(Button::DpadUp, true, t), Some(NavAction::Up));
        assert_eq!(m.button(Button::DpadUp, false, t), None);
        assert_eq!(m.button(Button::South, true, t), Some(NavAction::Accept));
        assert_eq!(m.button(Button::East, true, t), Some(NavAction::Back));
    }

    #[test]
    fn axis_respects_dead_zone() {
        let mut m = mapper();
        let t = Instant::now();
        assert_eq!(m.axis(Axis::LeftStickY, 0.2, t), None);
        assert_eq!(m.axis(Axis::LeftStickY, 0.9, t), Some(NavAction::Up));
    }

    #[test]
    fn held_axis_repeats_at_interval() {
        let mut m = mapper();
        let t0 = Instant::now();
        assert_eq!(m.axis(Axis::LeftStickX, 1.0, t0), Some(NavAction::Right));
        // Held below the repeat interval: no event.
        assert_eq!(m.axis(Axis::LeftStickX, 1.0, t0 + Duration::from_millis(100)), None);
        // Past the interval: repeat fires.
        assert_eq!(m.axis(Axis::LeftStickX, 1.0, t0 + Duration::from_millis(210)), Some(NavAction::Right));
        // Returning to center resets so the next push fires immediately.
        assert_eq!(m.axis(Axis::LeftStickX, 0.0, t0 + Duration::from_millis(220)), None);
        assert_eq!(m.axis(Axis::LeftStickX, -1.0, t0 + Duration::from_millis(230)), Some(NavAction::Left));
    }
}
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `cargo test -p grid-launcher gamepad` — compile failure. Then `mapping.rs`:
```rust
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    South, // accept
    East,  // back
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    LeftStickX,
    LeftStickY,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NavAction {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
}

struct AxisState {
    direction: i8, // -1, 0, 1 after dead zone
    last_emit: Instant,
}

pub struct Mapper {
    dead_zone: f32,
    repeat: Duration,
    axes: std::collections::HashMap<Axis, AxisState>,
}

impl Mapper {
    pub fn new(dead_zone: f32, repeat: Duration) -> Self {
        Self { dead_zone, repeat, axes: Default::default() }
    }

    pub fn button(&mut self, btn: Button, pressed: bool, _now: Instant) -> Option<NavAction> {
        if !pressed {
            return None;
        }
        Some(match btn {
            Button::DpadUp => NavAction::Up,
            Button::DpadDown => NavAction::Down,
            Button::DpadLeft => NavAction::Left,
            Button::DpadRight => NavAction::Right,
            Button::South => NavAction::Accept,
            Button::East => NavAction::Back,
        })
    }

    pub fn axis(&mut self, axis: Axis, value: f32, now: Instant) -> Option<NavAction> {
        let direction = if value > self.dead_zone {
            1
        } else if value < -self.dead_zone {
            -1
        } else {
            0
        };
        let state = self.axes.entry(axis).or_insert(AxisState { direction: 0, last_emit: now - self.repeat });
        let changed = state.direction != direction;
        state.direction = direction;
        if direction == 0 {
            state.last_emit = now - self.repeat; // reset: next push fires immediately
            return None;
        }
        if !changed && now.duration_since(state.last_emit) < self.repeat {
            return None;
        }
        state.last_emit = now;
        Some(match (axis, direction) {
            (Axis::LeftStickX, 1) => NavAction::Right,
            (Axis::LeftStickX, _) => NavAction::Left,
            // Stick Y is positive-up in gilrs; the thread normalizes if needed.
            (Axis::LeftStickY, 1) => NavAction::Up,
            (Axis::LeftStickY, _) => NavAction::Down,
        })
    }
}
```

- [ ] **Step 3: The gilrs thread**

Run `cargo add -p grid-launcher gilrs` (use src-tauri's real package name). `gamepad/mod.rs`:
```rust
pub mod mapping;

use mapping::{Axis, Button, Mapper, NavAction};
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
struct NavPayload {
    action: NavAction,
}

/// Dedicated poll thread; emits "nav" events to the frontend.
pub fn spawn(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("gamepad-poll".into())
        .spawn(move || {
            let Ok(mut gilrs) = gilrs::Gilrs::new() else {
                tracing::warn!("gamepad: gilrs unavailable; controller input disabled");
                return;
            };
            let mut mapper = Mapper::new(0.3, Duration::from_millis(200));
            loop {
                while let Some(ev) = gilrs.next_event() {
                    let now = Instant::now();
                    let action = match ev.event {
                        gilrs::EventType::ButtonPressed(b, _) => translate_button(b).and_then(|b| mapper.button(b, true, now)),
                        gilrs::EventType::ButtonReleased(b, _) => translate_button(b).and_then(|b| mapper.button(b, false, now)),
                        gilrs::EventType::AxisChanged(a, v, _) => translate_axis(a).and_then(|a| mapper.axis(a, v, now)),
                        _ => None,
                    };
                    if let Some(action) = action {
                        let _ = app.emit("nav", NavPayload { action });
                    }
                }
                // Held-stick repeats need re-evaluation between events.
                std::thread::sleep(Duration::from_millis(16));
                for (axis, value) in current_axis_values(&gilrs) {
                    if let Some(action) = mapper.axis(axis, value, Instant::now()) {
                        let _ = app.emit("nav", NavPayload { action });
                    }
                }
            }
        })
        .expect("spawn gamepad thread");
}

fn translate_button(b: gilrs::Button) -> Option<Button> {
    Some(match b {
        gilrs::Button::DPadUp => Button::DpadUp,
        gilrs::Button::DPadDown => Button::DpadDown,
        gilrs::Button::DPadLeft => Button::DpadLeft,
        gilrs::Button::DPadRight => Button::DpadRight,
        gilrs::Button::South => Button::South,
        gilrs::Button::East => Button::East,
        _ => return None,
    })
}

fn translate_axis(a: gilrs::Axis) -> Option<Axis> {
    Some(match a {
        gilrs::Axis::LeftStickX => Axis::LeftStickX,
        gilrs::Axis::LeftStickY => Axis::LeftStickY,
        _ => return None,
    })
}

fn current_axis_values(gilrs: &gilrs::Gilrs) -> Vec<(Axis, f32)> {
    let mut out = Vec::new();
    for (_id, gamepad) in gilrs.gamepads() {
        for (ga, la) in [(gilrs::Axis::LeftStickX, Axis::LeftStickX), (gilrs::Axis::LeftStickY, Axis::LeftStickY)] {
            if let Some(data) = gamepad.axis_data(ga) {
                out.push((la, data.value()));
            }
        }
    }
    out
}
```
Wire in `lib.rs` `run()` with a `.setup(|app| { gamepad::spawn(app.handle().clone()); Ok(()) })` builder call, and `cargo add -p grid-launcher tracing`. Check gilrs's current API names against docs if compilation disagrees.

- [ ] **Step 4: Run tests, fmt, clippy; commit**

Run: `cd rewrite && cargo test && cargo fmt --check && cargo clippy --workspace -- -D warnings`

```bash
git add rewrite/ && git commit -m "rewrite: gamepad poll thread with tested nav mapping (dead zone 0.3, repeat 200ms)"
```

---

### Task 11: Frontend — API layer, session store, connect screen

**Files:**
- Create: `rewrite/app/src/lib/api.ts`, `rewrite/app/src/lib/stores/session.svelte.ts`, `rewrite/app/src/lib/Connect.svelte`
- Modify: `rewrite/app/src/App.svelte`

**Interfaces:**
- Consumes: Task 9 commands (camelCase args via invoke).
- Produces: `api.connect(serverUrl, username, secret, useToken): Promise<SessionState>`, `api.restoreSession()`, `api.listPlatforms(): Promise<Platform[]>`, `api.listGames(platformId): Promise<GameSummary[]>`, `api.ensureCover(gameId, coverPath): Promise<string>`; `session` rune store `{ state: SessionState | null, error: string | null }`; `App.svelte` renders Connect when disconnected, Library when connected.

- [ ] **Step 1: Implement the API layer**

`lib/api.ts`:
```ts
import { invoke } from '@tauri-apps/api/core';

export type SessionState = { connected: boolean; username: string; server_url: string };
export type Platform = { id: number; name: string; slug: string; rom_count: number };
export type GameSummary = { id: number; name: string; platform_id: number; path_cover_small: string | null };

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<SessionState | null>('restore_session'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  ensureCover: (gameId: number, coverPath: string) =>
    invoke<string>('ensure_cover', { gameId, coverPath }),
};
```
NOTE: field-name casing between Rust serde and TS must match what the Rust structs actually serialize (snake_case by default). Keep TS types aligned with reality; do not add serde rename attributes just for cosmetics.

- [ ] **Step 2: Session store and connect screen**

`lib/stores/session.svelte.ts`:
```ts
import { api, type SessionState } from '../api';

export const session = $state<{ state: SessionState | null; error: string | null; busy: boolean }>({
  state: null,
  error: null,
  busy: false,
});

export async function restore() {
  try {
    session.state = await api.restoreSession();
  } catch {
    session.state = null; // silent: no stored session is normal
  }
}

export async function connect(serverUrl: string, username: string, secret: string, useToken: boolean) {
  session.busy = true;
  session.error = null;
  try {
    session.state = await api.connect(serverUrl, username, secret, useToken);
  } catch (e) {
    session.error = String(e);
  } finally {
    session.busy = false;
  }
}
```

`lib/Connect.svelte`:
```svelte
<script lang="ts">
  import { session, connect } from './stores/session.svelte';
  let serverUrl = $state('');
  let username = $state('');
  let secret = $state('');
  let useToken = $state(true);
</script>

<form
  class="connect"
  onsubmit={(e) => {
    e.preventDefault();
    connect(serverUrl, username, secret, useToken);
    secret = ''; // never keep the plain secret in frontend state
  }}
>
  <h1>Connect to RomM</h1>
  <label>Server URL <input bind:value={serverUrl} placeholder="https://romm.example" required /></label>
  <label>Username <input bind:value={username} autocomplete="username" /></label>
  <label>
    {useToken ? 'API token' : 'Password'}
    <input bind:value={secret} type="password" autocomplete="current-password" required />
  </label>
  <label class="mode"><input type="checkbox" bind:checked={useToken} /> Use API token</label>
  <button disabled={session.busy}>{session.busy ? 'Connecting…' : 'Connect'}</button>
  {#if session.error}<p class="error" role="alert">{session.error}</p>{/if}
</form>
```

`App.svelte`:
```svelte
<script lang="ts">
  import Connect from './lib/Connect.svelte';
  import Library from './lib/Library.svelte';
  import { session, restore } from './lib/stores/session.svelte';
  $effect(() => {
    restore();
  });
</script>

{#if session.state?.connected}
  <Library />
{:else}
  <Connect />
{/if}
```
Create a stub `lib/Library.svelte` (`<h1>Library</h1>`) so this task builds; Task 12 replaces it.

- [ ] **Step 3: Verify**

Run: `cd rewrite/app && npx svelte-check && npm run build`
Expected: no type errors; build succeeds. (Adjust Svelte-5 rune usage to current syntax if svelte-check disagrees — verify against Svelte docs via context7, not memory.)

- [ ] **Step 4: Commit**

```bash
git add rewrite/ && git commit -m "rewrite: frontend API layer, session store, connect screen"
```

---

### Task 12: Frontend — library grid with focus model and covers

**Files:**
- Create: `rewrite/app/src/lib/focus/grid.ts`, `rewrite/app/src/lib/focus/grid.test.ts`, `rewrite/app/src/lib/Library.svelte` (replace stub), `rewrite/app/src/lib/Cover.svelte`
- Modify: `rewrite/app/package.json` (vitest)

**Interfaces:**
- Consumes: `api.listPlatforms`, `api.listGames`, `api.ensureCover`, `convertFileSrc` from `@tauri-apps/api/core`.
- Produces: `moveFocus(index: number, action: 'up'|'down'|'left'|'right', columns: number, count: number): number` (pure, clamping, no wrap); `Library.svelte` shows platform tabs + virtualized game grid; `Cover.svelte` loads one cover lazily.

- [ ] **Step 1: Write failing tests for the focus math**

Run `cd rewrite/app && npm install -D vitest`, add `"test": "vitest run"` to package.json scripts. `lib/focus/grid.test.ts`:
```ts
import { describe, expect, it } from 'vitest';
import { moveFocus } from './grid';

describe('moveFocus (4 columns, 10 items)', () => {
  it('moves within a row and clamps at edges', () => {
    expect(moveFocus(0, 'right', 4, 10)).toBe(1);
    expect(moveFocus(3, 'right', 4, 10)).toBe(3); // row edge: clamp, no reading-order flow
  });
  it('clamps at row edges without wrapping', () => {
    expect(moveFocus(3, 'left', 4, 10)).toBe(2);
    expect(moveFocus(0, 'left', 4, 10)).toBe(0);
    expect(moveFocus(9, 'right', 4, 10)).toBe(9);
  });
  it('moves between rows and clamps on the last partial row', () => {
    expect(moveFocus(1, 'down', 4, 10)).toBe(5);
    expect(moveFocus(7, 'down', 4, 10)).toBe(9); // row below has only items 8,9 -> clamp to last
    expect(moveFocus(5, 'up', 4, 10)).toBe(1);
    expect(moveFocus(1, 'up', 4, 10)).toBe(1);
  });
});
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `npm test` — fails (module missing). `lib/focus/grid.ts`:
```ts
export type NavDirection = 'up' | 'down' | 'left' | 'right';

/** Spatial focus movement in a left-to-right grid: clamped, no wrap. */
export function moveFocus(index: number, action: NavDirection, columns: number, count: number): number {
  if (count <= 0) return 0;
  const row = Math.floor(index / columns);
  const col = index % columns;
  let next = index;
  if (action === 'left' && col > 0) next = index - 1;
  if (action === 'right' && col < columns - 1 && index + 1 < count) next = index + 1;
  if (action === 'up' && row > 0) next = index - columns;
  if (action === 'down') {
    const candidate = index + columns;
    if (candidate < count) next = candidate;
    else if (row < Math.floor((count - 1) / columns)) next = count - 1;
  }
  return Math.min(Math.max(next, 0), count - 1);
}
```

- [ ] **Step 3: Cover component and library view**

`lib/Cover.svelte`:
```svelte
<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api, type GameSummary } from './api';

  let { game }: { game: GameSummary } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    let cancelled = false;
    src = null;
    if (game.path_cover_small) {
      api.ensureCover(game.id, game.path_cover_small).then((path) => {
        if (!cancelled) src = convertFileSrc(path);
      }).catch(() => {}); // missing cover: placeholder stays
    }
    return () => { cancelled = true; };
  });
</script>

{#if src}
  <img {src} alt={game.name} loading="lazy" draggable="false" />
{:else}
  <div class="placeholder">{game.name}</div>
{/if}
```

`lib/Library.svelte`:
```svelte
<script lang="ts">
  import { api, type GameSummary, type Platform } from './api';
  import Cover from './Cover.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';

  const COLUMNS = 6;
  let platforms = $state<Platform[]>([]);
  let games = $state<GameSummary[]>([]);
  let activePlatform = $state<number | null>(null);
  let focusIndex = $state(0);
  let gridEl = $state<HTMLElement | null>(null);

  $effect(() => {
    api.listPlatforms().then((p) => {
      platforms = p;
      if (p.length && activePlatform === null) selectPlatform(p[0].id);
    });
  });

  async function selectPlatform(id: number) {
    activePlatform = id;
    focusIndex = 0;
    games = await api.listGames(id);
  }

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'accept' || action === 'back') return; // skeleton: navigation only
    focusIndex = moveFocus(focusIndex, action, COLUMNS, games.length);
    gridEl?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      handleNav(action);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<nav class="platforms">
  {#each platforms as p (p.id)}
    <button class:active={p.id === activePlatform} onclick={() => selectPlatform(p.id)}>{p.name}</button>
  {/each}
</nav>

<div class="grid" bind:this={gridEl} style="--columns: {COLUMNS}">
  {#each games as game, i (game.id)}
    <div class="card" class:focused={i === focusIndex}>
      <Cover {game} />
    </div>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--columns), 1fr);
    gap: 16px;
    padding: 24px;
    content-visibility: auto;
  }
  .card {
    aspect-ratio: 3 / 4;
    border-radius: 8px;
    overflow: hidden;
    transform: scale(1);
    transition: transform 160ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
  }
  .card.focused {
    transform: scale(1.08);
    outline: 3px solid #7aa2ff;
    z-index: 1;
  }
  .card :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    background: #2a2d34;
    color: #aab;
    font-size: 0.8rem;
    text-align: center;
    padding: 8px;
  }
</style>
```
(True windowed virtualization is deferred: `content-visibility: auto` plus lazy covers carries the skeleton; note this in the commit message. If scrolling jank appears in Task 14's manual run, virtualization becomes the first follow-up.)

- [ ] **Step 4: Run tests and build; commit**

Run: `cd rewrite/app && npm test && npx svelte-check && npm run build`

```bash
git add rewrite/ && git commit -m "rewrite: library grid with tested focus model, lazy covers, focus animation"
```

---

### Task 13: Frontend — gamepad events drive the same focus model

**Files:**
- Modify: `rewrite/app/src/App.svelte`, `rewrite/app/src/lib/Library.svelte` (only if the export needs adjusting)

**Interfaces:**
- Consumes: Tauri event `"nav"` payload `{ action: string }` (Task 10), `Library.handleNav` (Task 12).

- [ ] **Step 1: Wire the listener**

In `App.svelte`:
```svelte
<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import Connect from './lib/Connect.svelte';
  import Library from './lib/Library.svelte';
  import { session, restore } from './lib/stores/session.svelte';

  let library = $state<ReturnType<typeof Library> | null>(null);

  $effect(() => {
    restore();
    const un = listen<{ action: 'up' | 'down' | 'left' | 'right' | 'accept' | 'back' }>('nav', (e) => {
      library?.handleNav(e.payload.action);
    });
    return () => { un.then((f) => f()); };
  });
</script>

{#if session.state?.connected}
  <Library bind:this={library} />
{:else}
  <Connect />
{/if}
```
(Svelte 5 component instance binding: verify `bind:this` exposes exported functions per current Svelte docs; if not, route nav actions through a small event-bus module instead — a `nav.svelte.ts` store with a `pending` action the Library consumes in an `$effect`.)

- [ ] **Step 2: Verify and commit**

Run: `cd rewrite/app && npx svelte-check && npm run build`

```bash
git add rewrite/ && git commit -m "rewrite: gamepad nav events drive the library focus model"
```

---

### Task 14: CI workflow and secret-leak guard

**Files:**
- Create: `.github/workflows/rust-rewrite.yml`, `rewrite/scripts/check_secret_hygiene.sh`

**Interfaces:**
- Consumes: whole workspace.
- Produces: CI gate for every later rewrite task.

- [ ] **Step 1: The hygiene script**

`rewrite/scripts/check_secret_hygiene.sh`:
```bash
#!/usr/bin/env bash
# Fails if secret-handling rules are violated:
# 1. expose_secret() outside the two permitted call sites.
# 2. Anything resembling a real bearer token in committed test fixtures.
set -euo pipefail
cd "$(dirname "$0")/.."

allowed_files=("crates/grid-core/src/secrets.rs" "crates/grid-core/src/romm/mod.rs")
violations=$(grep -rn "expose_secret" crates app/src-tauri --include="*.rs" \
  | grep -vF -e "${allowed_files[0]}" -e "${allowed_files[1]}" || true)
if [ -n "$violations" ]; then
  echo "expose_secret() outside permitted call sites:" >&2
  echo "$violations" >&2
  exit 1
fi

# Real-looking secrets in tests/fixtures: long bearer-ish strings that are not
# the sanctioned fake. The fake token is allowed everywhere.
suspicious=$(grep -rnE "(Bearer|token|password)[\"': =]+[A-Za-z0-9+/_-]{30,}" \
  crates app/src --include="*.rs" --include="*.ts" --include="*.json" \
  | grep -v "FAKE-TEST-TOKEN-not-real" || true)
if [ -n "$suspicious" ]; then
  echo "Possible real credential in committed code/fixtures:" >&2
  echo "$suspicious" >&2
  exit 1
fi
echo "secret hygiene OK"
```
Run `chmod +x rewrite/scripts/check_secret_hygiene.sh`.

- [ ] **Step 2: Run it now**

Run: `rewrite/scripts/check_secret_hygiene.sh`
Expected: `secret hygiene OK` (the two allowed files are the keyring serializer and the auth-header builder).

- [ ] **Step 3: The workflow**

`.github/workflows/rust-rewrite.yml`:
```yaml
name: Rust rewrite

on:
  push:
    paths: ['rewrite/**', '.github/workflows/rust-rewrite.yml']
  pull_request:
    paths: ['rewrite/**', '.github/workflows/rust-rewrite.yml']

jobs:
  check:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: rewrite
    steps:
      - uses: actions/checkout@v4
      - name: System dependencies (Tauri on Linux)
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rewrite
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - name: Secret hygiene
        run: scripts/check_secret_hygiene.sh
      - name: Format
        run: cargo fmt --check
      - name: Frontend install and build
        run: cd app && npm ci && npx svelte-check && npm run build
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
      - name: Tests
        run: cargo test --workspace
      - name: Frontend tests
        run: cd app && npm test
```
(Confirm the current webkit2gtk package name for ubuntu-latest against Tauri's Linux prerequisites doc; `libudev-dev` is for gilrs.)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/rust-rewrite.yml rewrite/scripts/
git commit -m "rewrite: CI workflow with secret-hygiene guard"
```

---

### Task 15: AppImage build, README, human-testing handoff

**Files:**
- Create: `rewrite/README.md`
- Modify: `rewrite/app/src-tauri/tauri.conf.json` (bundle targets)

**Interfaces:**
- Consumes: everything.
- Produces: a runnable AppImage and the manual test checklist — the milestone's exit gate.

- [ ] **Step 1: Configure bundling**

In `tauri.conf.json` set `bundle.targets` to `["appimage"]` and `bundle.active` to `true`, and set `productName` to `GRID Launcher (Rust preview)`. Confirm key names against the current schema.

- [ ] **Step 2: Build**

Run: `cd rewrite/app && npx tauri build`
Expected: an AppImage under `rewrite/app/src-tauri/target/release/bundle/appimage/` (path may live under the workspace `rewrite/target/` — report where it lands).

- [ ] **Step 3: Write rewrite/README.md**

```markdown
# GRID Launcher — Rust rewrite (walking skeleton)

Milestone 1 of the Rust + Tauri rewrite. Behavior contract: `../docs/porting/`.
Spec: `../docs/superpowers/specs/2026-08-31-rust-tauri-walking-skeleton-design.md`.

## Layout
- `crates/grid-core` — UI-agnostic core: config, secrets (OS keyring only),
  RomM client, cover cache, session.
- `app/` — Tauri 2 shell + Svelte 5 frontend.

## Develop
    cd app && npm install && npx tauri dev

## Test
    cargo test --workspace          # Rust
    cd app && npm test              # frontend focus model
    scripts/check_secret_hygiene.sh # secret rules

## Build
    cd app && npx tauri build       # AppImage on Linux

## Secret handling
Credentials live only in the OS keyring and in redacting in-memory types.
They never appear in config files, logs, IPC payloads, or fixtures.
See the spec's "Secret handling" section — those rules are normative.
```

- [ ] **Step 4: Full verification sweep**

Run, from `rewrite/`:
```bash
cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace
cd app && npm test && npx svelte-check && npm run build
cd .. && scripts/check_secret_hygiene.sh
```
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add rewrite/ && git commit -m "rewrite: AppImage bundling and milestone README"
```

- [ ] **Step 6: HUMAN TESTING GATE — stop and report**

Manual checklist for the user (cannot be automated here):
1. Run the AppImage on the desktop.
2. Connect to the live RomM server with an API token; confirm connect succeeds and the token prompt never echoes the value anywhere (window, terminal, logs).
3. Browse platforms; covers populate and scrolling is smooth.
4. Navigate the grid with a real gamepad: d-pad and left stick move focus, held stick repeats at a comfortable rate.
5. Quit and relaunch: the session restores without re-entering credentials.
6. `cat ~/.config/grid-launcher/config.toml` — confirm no token/password present.

---

## Self-review notes

- Spec coverage: config (T2), secrets (T3), client auth+probe (T4), catalog (T5), covers (T6), session (T7), shell+asset protocol (T8), commands/IPC (T9), gamepad (T10), frontend connect (T11), grid+focus (T12), gamepad wiring (T13), CI+guard (T14), AppImage+handoff (T15). Spec's `identity.rs` is folded into the cover key (T6) — a separate identity module starts in the next milestone with docs/porting/10.
- Deliberate deviations recorded: asset protocol instead of `covers://` (header); virtualization deferred behind `content-visibility` (T12); both noted for the milestone review.
- Type consistency checked: `SessionState`/`Platform`/`GameSummary` field names flow Rust→TS as snake_case; T11 warns against cosmetic renames.
```
