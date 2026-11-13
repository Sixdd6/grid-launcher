//! GOG sign-in: the login page, the code in GOG's redirect, gogdl's `auth`
//! answers, and the in-memory bearer.
//!
//! gogdl keeps the refresh token in its own file
//! (`<data_dir>/stores/gogdl/auth.json`, 0600: the store-token exception in
//! AGENTS.md). GRID holds the access token only in memory, as a
//! [`SecretString`] inside [`GogToken`], whose `Debug` never prints it.
//! This file is one of the two allowed `expose_secret` call sites for GOG
//! (`scripts/check_secret_hygiene.sh`).

use std::path::Path;

use secrecy::{ExposeSecret as _, SecretString};
use url::Url;

use super::GogEndpoints;

/// GOG Galaxy's public client id: the login page and gogdl both use it.
pub const CLIENT_ID: &str = "46899977096215655";

/// The path of GOG's post-login redirect, under the embed host.
pub const REDIRECT_PATH: &str = "on_login_success";

/// The file gogdl keeps its tokens in, under the gogdl tool folder.
pub const AUTH_FILE: &str = "auth.json";

/// A GOG access token, held only in memory. `Debug` prints neither the
/// token nor its length.
#[derive(Clone)]
pub struct GogToken {
    pub access: SecretString,
    pub user_id: String,
    /// Epoch seconds the token stops working (`loginTime + expires_in`).
    pub expires_at: i64,
}

impl std::fmt::Debug for GogToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GogToken")
            .field("access", &"<redacted>")
            .field("user_id", &self.user_id)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl GogToken {
    /// Whether the token is still good for at least `margin` seconds.
    pub fn fresh_at(&self, now: i64, margin: i64) -> bool {
        self.expires_at - margin > now
    }
}

/// What gogdl's `auth` printed.
#[derive(Debug)]
pub enum AuthOutput {
    Token(GogToken),
    /// `null`: no credentials, or the refresh failed.
    Null,
    /// `{"error": true}`: GOG refused the code, or was unreachable.
    Error,
}

/// Parses gogdl's `auth` stdout. The last line that is JSON wins (gogdl
/// prints one line; anything else is ignored). `None` when nothing in it
/// is JSON GRID understands.
pub fn parse_auth_output(stdout: &str) -> Option<AuthOutput> {
    let value = stdout
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())?;
    if value.is_null() {
        return Some(AuthOutput::Null);
    }
    let object = value.as_object()?;
    if object
        .get("error")
        .is_some_and(|e| e.as_bool() == Some(true))
    {
        return Some(AuthOutput::Error);
    }
    let access = object.get("access_token")?.as_str()?.trim();
    if access.is_empty() {
        return None;
    }
    let user_id = match object.get("user_id") {
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let login = object
        .get("loginTime")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let expires_in = object
        .get("expires_in")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    Some(AuthOutput::Token(GogToken {
        access: SecretString::from(access.to_string()),
        user_id,
        expires_at: (login + expires_in) as i64,
    }))
}

/// The login page: GOG's auth host with the Galaxy client id and the embed
/// redirect.
pub fn login_url(ep: &GogEndpoints) -> Url {
    let mut url = ep.auth.join("auth").unwrap_or_else(|_| ep.auth.clone());
    let redirect = redirect_url(ep);
    url.query_pairs_mut()
        .clear()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", redirect.as_str())
        .append_pair("response_type", "code")
        .append_pair("layout", "galaxy");
    url
}

/// `<embed>/on_login_success?origin=client`.
pub fn redirect_url(ep: &GogEndpoints) -> Url {
    let mut url = ep
        .embed
        .join(REDIRECT_PATH)
        .unwrap_or_else(|_| ep.embed.clone());
    url.query_pairs_mut()
        .clear()
        .append_pair("origin", "client");
    url
}

/// The login code in GOG's redirect, `None` for any other URL (another
/// host, another path, or a blank code).
pub fn code_from_redirect(ep: &GogEndpoints, url: &Url) -> Option<SecretString> {
    let expected = redirect_url(ep);
    if url.scheme() != expected.scheme()
        || url.host_str() != expected.host_str()
        || url.port_or_known_default() != expected.port_or_known_default()
        || url.path() != expected.path()
    {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.trim().to_string())
        .filter(|code| !code.is_empty())
        .map(SecretString::from)
}

/// gogdl's arguments for exchanging a login code. `--auth-config-path`
/// comes before the subcommand (after it, gogdl ignores it).
pub fn code_args(auth_file: &Path, code: &SecretString) -> Vec<String> {
    vec![
        "--auth-config-path".to_string(),
        auth_file.to_string_lossy().into_owned(),
        "auth".to_string(),
        "--code".to_string(),
        code.expose_secret().to_string(),
    ]
}

/// gogdl's arguments for the current (refreshed) Galaxy credentials, or a
/// game client's credentials when `client` is `(id, secret)`.
pub fn credentials_args(auth_file: &Path, client: Option<(&str, &SecretString)>) -> Vec<String> {
    let mut args = vec![
        "--auth-config-path".to_string(),
        auth_file.to_string_lossy().into_owned(),
        "auth".to_string(),
    ];
    if let Some((id, secret)) = client {
        args.extend([
            "--client-id".to_string(),
            id.to_string(),
            "--client-secret".to_string(),
            secret.expose_secret().to_string(),
        ]);
    }
    args
}

/// Hands a tool's kept stdout to `parse` (a JSON parse) without it ever
/// leaving the [`SecretString`] otherwise. The one way GOG code reads
/// gogdl's stdout; the text must never be logged or put in an error.
pub(crate) fn read_stdout<T>(stdout: &SecretString, parse: impl FnOnce(&str) -> T) -> T {
    parse(stdout.expose_secret())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    #[test]
    fn a_token_answer_parses_and_never_prints() {
        let out = format!(
            r#"{{"access_token": "{FAKE}", "expires_in": 3600, "token_type": "bearer", "refresh_token": "{FAKE}", "user_id": "48628349957132247", "session_id": "x", "loginTime": 1700000000.5}}"#
        );
        let Some(AuthOutput::Token(token)) = parse_auth_output(&out) else {
            panic!("no token")
        };
        assert_eq!(token.user_id, "48628349957132247");
        assert_eq!(token.expires_at, 1_700_003_600);
        assert!(token.fresh_at(1_700_000_000, 60));
        assert!(!token.fresh_at(1_700_003_590, 60));
        let debug = format!("{token:?}");
        assert!(!debug.contains(FAKE), "{debug}");
        let output = format!("{:?}", AuthOutput::Token(token));
        assert!(!output.contains(FAKE), "{output}");
    }

    #[test]
    fn null_error_and_junk_answers() {
        assert!(matches!(
            parse_auth_output("null\n"),
            Some(AuthOutput::Null)
        ));
        assert!(matches!(
            parse_auth_output(r#"{"error": true}"#),
            Some(AuthOutput::Error)
        ));
        assert!(parse_auth_output("").is_none());
        assert!(parse_auth_output("not json").is_none());
        assert!(parse_auth_output(r#"{"access_token": ""}"#).is_none());
        let numeric = format!(r#"{{"access_token": "{FAKE}", "user_id": 42}}"#);
        let Some(AuthOutput::Token(token)) = parse_auth_output(&numeric) else {
            panic!()
        };
        assert_eq!(token.user_id, "42");
    }

    #[test]
    fn the_login_url_and_redirect_code() {
        let ep = GogEndpoints::real();
        let login = login_url(&ep);
        assert_eq!(login.host_str(), Some("auth.gog.com"));
        assert_eq!(login.path(), "/auth");
        let pairs: Vec<(String, String)> = login.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("client_id".into(), CLIENT_ID.into())));
        assert!(pairs.contains(&(
            "redirect_uri".into(),
            "https://embed.gog.com/on_login_success?origin=client".into()
        )));
        assert!(pairs.contains(&("response_type".into(), "code".into())));

        let good = Url::parse(&format!(
            "https://embed.gog.com/on_login_success?origin=client&code={FAKE}"
        ))
        .unwrap();
        let code = code_from_redirect(&ep, &good).unwrap();
        assert_eq!(code.expose_secret(), FAKE);
        for bad in [
            "https://embed.gog.com/on_login_success?origin=client",
            "https://embed.gog.com/on_login_success?code=",
            "https://evil.example/on_login_success?code=x",
            "https://embed.gog.com/elsewhere?code=x",
            "http://embed.gog.com/on_login_success?code=x",
        ] {
            assert!(
                code_from_redirect(&ep, &Url::parse(bad).unwrap()).is_none(),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_e2e_redirect_is_on_the_mock() {
        let ep = GogEndpoints::e2e(Url::parse("http://127.0.0.1:4567/").unwrap());
        let url = Url::parse("http://127.0.0.1:4567/on_login_success?code=abc").unwrap();
        assert!(code_from_redirect(&ep, &url).is_some());
        assert!(login_url(&ep)
            .as_str()
            .starts_with("http://127.0.0.1:4567/auth?"));
    }

    #[test]
    fn auth_config_path_comes_before_the_subcommand() {
        let file = Path::new("/data/stores/gogdl/auth.json");
        let code = SecretString::from(FAKE.to_string());
        let args = code_args(file, &code);
        assert_eq!(args[0], "--auth-config-path");
        assert_eq!(args[1], "/data/stores/gogdl/auth.json");
        assert_eq!(args[2], "auth");
        assert_eq!(&args[3..], ["--code", FAKE]);
        let creds = credentials_args(file, None);
        assert_eq!(
            creds,
            ["--auth-config-path", "/data/stores/gogdl/auth.json", "auth"]
        );
        let secret = SecretString::from("s".to_string());
        let game = credentials_args(file, Some(("123", &secret)));
        assert_eq!(
            &game[2..],
            ["auth", "--client-id", "123", "--client-secret", "s"]
        );
    }
}
