//! RetroAchievements login client.
//!
//! Ports `ra_login` and the `_fetch_json` error mapping it depends on
//! (`grid_launcher/server/retroachievements.py:22-45, 68-89`), reached from
//! `RALoginWorker` (`grid_launcher/background/workers.py:799-816`) and
//! `_ra_login_clicked` (grid-launcher.py:2705-2756). Only the login endpoint
//! is ported: the achievement-list calls next to it in the Python module
//! belong to the achievements panel, which is a documented exclusion.
//!
//! **Token secrecy.** The endpoint takes the password as a QUERY PARAMETER
//! (`?r=login&u=<user>&p=<password>`), so the request URL is itself a
//! secret. Nothing here ever puts the URL, the password, or the returned
//! token in an error, a log line or a `Debug` rendering:
//!
//! * every `reqwest::Error` goes through `.without_url()` before it is
//!   formatted, the same rule `romm/mod.rs:73, 92` follows;
//! * the returned token is a [`SecretString`], which redacts under `Debug`;
//! * [`RaLogin`]'s `Debug` can print only the account name and that
//!   redaction — the struct carries nothing else;
//! * this module is on `scripts/check_secret_hygiene.sh`'s `expose_secret`
//!   allowlist for exactly two calls: the blank-password check and putting
//!   the password into the query.

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

/// `_RA_DOREQUEST_URL` (retroachievements.py:9).
pub const RA_DOREQUEST_URL: &str = "https://retroachievements.org/dorequest.php";

/// `Request(url, headers={"User-Agent": ...})` (retroachievements.py:25).
const USER_AGENT: &str = "grid-launcher/1.0 (retroachievements-client)";

/// A successful login: the account name the SERVER reports (never the typed
/// one) and its connect token.
#[derive(Debug)]
pub struct RaLogin {
    pub username: String,
    pub token: SecretString,
}

/// A plain client with a `User-Agent` and a 10s timeout (matching
/// `urlopen(..., timeout=10)`, retroachievements.py:26) and no other default
/// header. It must never share a client with `RommClient`: different host,
/// and the RomM token must never reach retroachievements.org.
pub fn build_http_client() -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(USER_AGENT),
    );
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .default_headers(headers)
        .build()
        .expect("ra http client: static header/timeout config always builds")
}

/// `ra_login` (retroachievements.py:68-89) against the real endpoint.
pub async fn ra_login(
    http: &reqwest::Client,
    username: &str,
    password: &SecretString,
) -> Result<RaLogin, String> {
    ra_login_with_base(http, RA_DOREQUEST_URL, username, password).await
}

/// The `base_url`-parameterised form, so tests can point at a mock server —
/// the same split `pcgw.rs`'s `fetch_windows_save_paths_with_base` uses.
pub async fn ra_login_with_base(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &SecretString,
) -> Result<RaLogin, String> {
    // retroachievements.py:69-72 — validated before anything is sent.
    if username.trim().is_empty() {
        return Err("username must be a non-empty string".to_string());
    }
    if password.expose_secret().trim().is_empty() {
        return Err("password must be a non-empty string".to_string());
    }

    let payload = fetch_json(
        http,
        base_url,
        &[
            ("r", "login"),
            ("u", username),
            // The second of this module's two `expose_secret` calls
            // (the other is the blank-password check above): the endpoint
            // has no other way to take the password.
            ("p", password.expose_secret()),
        ],
    )
    .await?;

    // retroachievements.py:76-88.
    if payload.get("Success").and_then(Value::as_bool) == Some(true) {
        let user = payload.get("User").and_then(Value::as_str).unwrap_or("");
        if user.is_empty() {
            return Err("RetroAchievements login response missing User".to_string());
        }
        let token = payload.get("Token").and_then(Value::as_str).unwrap_or("");
        if token.is_empty() {
            return Err("RetroAchievements login response missing Token".to_string());
        }
        return Ok(RaLogin {
            username: user.to_string(),
            token: SecretString::from(token),
        });
    }

    Err(error_text(&payload).unwrap_or_else(|| "Invalid credentials".to_string()))
}

/// `_fetch_json` (retroachievements.py:22-45). The query is built with
/// reqwest's own encoder rather than by string concatenation so a password
/// with `&`/`=` in it cannot corrupt the request.
///
/// The whole `Success` decision is left to [`ra_login_with_base`]: a
/// `Success: false` payload with no `Error` must read as `"Invalid
/// credentials"`, not as this function's generic wording.
async fn fetch_json(
    http: &reqwest::Client,
    base_url: &str,
    query: &[(&str, &str)],
) -> Result<Value, String> {
    let response = http
        .get(base_url)
        .query(query)
        .send()
        .await
        // `.without_url()`: the URL carries the password.
        .map_err(|e| format!("RetroAchievements request failed: {}", e.without_url()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("RetroAchievements request failed: {}", e.without_url()))?;

    if !status.is_success() {
        // retroachievements.py:32-38: a <=300-char body excerpt, or the
        // status line when the body is empty. RetroAchievements answers with
        // its own JSON error object here, never an echo of the request.
        let detail = if body.trim().is_empty() {
            status.to_string()
        } else {
            body.chars().take(300).collect::<String>()
        };
        return Err(format!(
            "RetroAchievements HTTP {}: {detail}",
            status.as_u16()
        ));
    }

    let payload: Value = serde_json::from_str(&body)
        .map_err(|e| format!("RetroAchievements request failed: {e}"))?;

    if !payload.is_object() {
        return Err("RetroAchievements response must be a JSON object".to_string());
    }

    // retroachievements.py:40-42: a non-empty `Error` is an error even on a
    // 200. `Message` alone (e.g. on a `Success: true` payload) is not — the
    // `Error`‖`Message` fallback belongs only to `ra_login_with_base`'s
    // failure branch.
    if let Some(text) = payload
        .get("Error")
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
    {
        return Err(text.to_string());
    }

    Ok(payload)
}

/// `payload.get("Error") or payload.get("Message")`, blank treated as absent.
fn error_text(payload: &Value) -> Option<String> {
    for key in ["Error", "Message"] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}
