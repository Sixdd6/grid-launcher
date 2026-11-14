//! Amazon login through nile: `auth --login --non-interactive` (the login
//! URL and the values `register` needs), `register` (the code and those
//! values on argv), the account from `auth --status`, and `auth --logout`.
//!
//! This is the only file that reads [`AmazonLoginStart`]'s secret and the
//! login code (`scripts/check_secret_hygiene.sh`), and the only one that
//! writes them into `nile register`'s argv: nile has no stdin or
//! environment path for them. `ToolCommand`'s `Debug` hides every
//! argument, and nile itself never echoes them (spike 2; the client id
//! appears in one stderr line the runner drops).
//!
//! nile refuses both `auth --login` and `register` while
//! `current_user.json` names a user ("You are already logged in"), even
//! when that login has expired. A new sign-in therefore starts by removing
//! nile's saved login ([`clear_saved_login`]).
//!
//! nile exits 0 when Amazon refuses a code (it prints Amazon's answer on
//! stdout, which GRID never reads) and exits 1 when Amazon can't be
//! reached, so a login counts as done only when no `Failed to register a
//! device` line came and nile wrote `current_user.json` with a user id.

use std::fs;
use std::io;
use std::path::Path;

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;

use super::list::json_value;
use super::{AmazonAccount, AmazonEndpoints, AmazonLoginStart};
use crate::stores::runner::{ToolExit, ToolSignal};
use crate::stores::StoreError;

/// nile's file naming the signed-in user (`{"name", "user_id"}`), in its
/// config folder.
pub const CURRENT_USER_FILE: &str = "current_user.json";
/// The extension of nile's token file (`<md5(user_id)>.enc`).
pub const TOKEN_EXTENSION: &str = "enc";
/// What `auth --status` says for `Username` when signed out.
pub const NOT_LOGGED_IN: &str = "<not logged in>";
/// The words for a code Amazon refused.
pub const LOGIN_REFUSED: &str = "Amazon didn't accept the sign-in. Try again.";

/// `auth --login --non-interactive`: one JSON line with the login URL and
/// the values `register` needs. Generated locally (no request to Amazon).
pub fn login_args() -> Vec<String> {
    vec!["auth".into(), "--login".into(), "--non-interactive".into()]
}

/// `auth --status`: local only (it reads `current_user.json`).
pub fn status_args() -> Vec<String> {
    vec!["auth".into(), "--status".into()]
}

/// `auth --logout`: deregisters the device (nile deletes its files only
/// when Amazon answers OK, so Disconnect deletes the folder itself).
pub fn logout_args() -> Vec<String> {
    vec!["auth".into(), "--logout".into()]
}

/// The values nile's `auth --login --non-interactive` gives besides the
/// URL. Kept only inside a [`SecretString`] as JSON.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct LoginValues {
    client_id: String,
    code_verifier: String,
    serial: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LoginAnswer {
    client_id: String,
    code_verifier: String,
    serial: String,
    url: String,
}

/// The login start in `auth --login --non-interactive`'s stdout: the URL
/// (which must be a sign-in page [`AmazonEndpoints::allows_login_url`]
/// accepts) and the three values, kept in a [`SecretString`]. Other stdout
/// lines are skipped.
pub fn parse_login_start(
    stdout: &str,
    ep: &AmazonEndpoints,
) -> Result<AmazonLoginStart, StoreError> {
    let not_understood = || StoreError::ToolFailed {
        tool: "nile",
        reason: "it gave an answer GRID doesn't understand".to_string(),
    };
    let value = json_value(stdout).ok_or_else(not_understood)?;
    let answer: LoginAnswer = serde_json::from_value(value).map_err(|_| not_understood())?;
    if [&answer.client_id, &answer.code_verifier, &answer.serial]
        .iter()
        .any(|v| v.trim().is_empty())
    {
        return Err(not_understood());
    }
    let url = Url::parse(answer.url.trim()).map_err(|_| not_understood())?;
    if !ep.allows_login_url(&url) {
        return Err(StoreError::Refused(
            "nile gave a sign-in address that isn't Amazon's, so GRID didn't open it.".to_string(),
        ));
    }
    let values = LoginValues {
        client_id: answer.client_id.trim().to_string(),
        code_verifier: answer.code_verifier.trim().to_string(),
        serial: answer.serial.trim().to_string(),
    };
    let json = serde_json::to_string(&values).map_err(|_| not_understood())?;
    Ok(AmazonLoginStart::new(url, SecretString::from(json)))
}

/// `register --code <code> --code-verifier <v> --serial <s> --client-id
/// <c>`. `None` when the kept values are not readable.
pub fn register_args(start: &AmazonLoginStart, code: &SecretString) -> Option<Vec<String>> {
    let values: LoginValues = serde_json::from_str(start.secret().expose_secret()).ok()?;
    let code = code.expose_secret().trim();
    if code.is_empty() || values.code_verifier.is_empty() || values.client_id.is_empty() {
        return None;
    }
    Some(vec![
        "register".to_string(),
        "--code".to_string(),
        code.to_string(),
        "--code-verifier".to_string(),
        values.code_verifier,
        "--serial".to_string(),
        values.serial,
        "--client-id".to_string(),
        values.client_id,
    ])
}

/// Whether nile has a saved login: `current_user.json` names a user id.
pub fn has_saved_login(config_dir: &Path) -> bool {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct CurrentUser {
        user_id: Option<String>,
    }
    fs::read_to_string(config_dir.join(CURRENT_USER_FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<CurrentUser>(&text).ok())
        .and_then(|user| user.user_id)
        .is_some_and(|id| !id.trim().is_empty())
}

/// Whether nile's token file is there (any `*.enc` in its config folder).
pub fn has_token_file(config_dir: &Path) -> bool {
    fs::read_dir(config_dir)
        .map(|entries| {
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == TOKEN_EXTENSION)
            })
        })
        .unwrap_or(false)
}

/// Removes nile's saved login (`current_user.json` and the token file),
/// so `auth --login` and `register` run (nile refuses both while a user is
/// saved). The library and installed games stay.
pub fn clear_saved_login(config_dir: &Path) -> io::Result<()> {
    match fs::remove_file(config_dir.join(CURRENT_USER_FILE)) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let entries = match fs::read_dir(config_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == TOKEN_EXTENSION) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Whether a `register` run signed in, else GRID's words for why not.
/// `saved` is [`has_saved_login`] after the run (the folder was cleared
/// before it). A failed library sync after a good registration is not a
/// login failure: GRID's own sync comes next and reports it.
pub fn login_outcome(exit: &ToolExit, saved: bool) -> Result<(), StoreError> {
    if exit.has(&ToolSignal::LoginFailed) {
        return Err(StoreError::Refused(LOGIN_REFUSED.to_string()));
    }
    if saved {
        return Ok(());
    }
    if exit.has(&ToolSignal::Network) {
        return Err(StoreError::Network("Amazon Games".to_string()));
    }
    Err(StoreError::ToolFailed {
        tool: "nile",
        reason: "it couldn't finish the sign-in".to_string(),
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StatusAnswer {
    #[serde(rename = "Username")]
    username: Option<String>,
    #[serde(rename = "LoggedIn")]
    logged_in: Option<bool>,
}

/// The account `auth --status` names: `Some(None)` when it says signed
/// out, `None` when stdout is not a status answer.
pub fn parse_status(stdout: &str) -> Option<Option<AmazonAccount>> {
    let value = json_value(stdout)?;
    if !value.is_object() {
        return None;
    }
    let answer: StatusAnswer = serde_json::from_value(value).ok()?;
    let logged_in = answer.logged_in?;
    let name = answer.username.unwrap_or_default().trim().to_string();
    if !logged_in || name == NOT_LOGGED_IN {
        return Some(None);
    }
    Some(Some(AmazonAccount { username: name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    fn exit(code: i32, signals: Vec<ToolSignal>) -> ToolExit {
        ToolExit {
            code: Some(code),
            stdout: SecretString::from(FAKE),
            signals,
        }
    }

    fn login_json(url: &str) -> String {
        format!(
            r#"{{"client_id": "c-{FAKE}", "code_verifier": "{FAKE}", "serial": "s-{FAKE}", "url": "{url}", "fake_session": "{FAKE}"}}"#
        )
    }

    #[test]
    fn argv_builders_never_debug() {
        for args in [login_args(), status_args(), logout_args()] {
            assert!(!args.iter().any(|a| a == "-d"), "{args:?}");
        }
        assert_eq!(login_args(), ["auth", "--login", "--non-interactive"]);
        assert_eq!(status_args(), ["auth", "--status"]);
        assert_eq!(logout_args(), ["auth", "--logout"]);
    }

    #[test]
    fn the_login_start_keeps_its_values_secret_and_checks_the_host() {
        let ep = AmazonEndpoints::real();
        let url = "https://amazon.com/ap/signin?openid.assoc_handle=amzn_sonic_games_launcher";
        let stdout = format!("{FAKE}\n{}\n", login_json(url));
        let start = parse_login_start(&stdout, &ep).unwrap();
        assert_eq!(start.url.as_str(), url);
        assert!(!format!("{start:?}").contains(FAKE));
        let code = SecretString::from("THE-CODE");
        let args = register_args(&start, &code).unwrap();
        assert_eq!(
            args,
            [
                "register",
                "--code",
                "THE-CODE",
                "--code-verifier",
                FAKE,
                "--serial",
                &format!("s-{FAKE}"),
                "--client-id",
                &format!("c-{FAKE}"),
            ]
        );
        assert!(register_args(&start, &SecretString::from("  ")).is_none());

        for bad in [
            "https://evil.example/ap/signin",
            "http://amazon.com/ap/signin",
            "https://amazon.com/somewhere",
        ] {
            let error = parse_login_start(&login_json(bad), &ep).unwrap_err();
            assert!(matches!(error, StoreError::Refused(_)), "{bad}: {error}");
            assert!(!error.to_string().contains(FAKE));
        }
        for junk in [
            "Traceback (most recent call last):".to_string(),
            r#"{"url": "https://amazon.com/ap/signin"}"#.to_string(),
            format!(
                r#"{{"client_id": "a", "code_verifier": "b", "serial": "c", "url": "not a url {FAKE}"}}"#
            ),
        ] {
            let error = parse_login_start(&junk, &ep).unwrap_err();
            assert!(error.to_string().starts_with("nile stopped:"), "{error}");
            assert!(!error.to_string().contains(FAKE));
        }
        let e2e = AmazonEndpoints::e2e(Url::parse("http://127.0.0.1:4400/").unwrap());
        assert!(
            parse_login_start(&login_json("http://127.0.0.1:4400/ap/signin?x=1"), &e2e).is_ok()
        );
    }

    #[test]
    fn a_login_needs_no_refusal_and_the_saved_user() {
        assert!(login_outcome(&exit(0, vec![]), true).is_ok());
        assert!(
            login_outcome(&exit(0, vec![ToolSignal::OtherError]), true).is_ok(),
            "a failed sync after registering"
        );
        let refused = login_outcome(&exit(0, vec![ToolSignal::LoginFailed]), false).unwrap_err();
        assert_eq!(refused.to_string(), LOGIN_REFUSED);
        let offline = login_outcome(
            &exit(1, vec![ToolSignal::OtherError, ToolSignal::Network]),
            false,
        )
        .unwrap_err();
        assert!(matches!(offline, StoreError::Network(_)), "{offline}");
        let error = login_outcome(&exit(0, vec![]), false).unwrap_err();
        assert!(error.to_string().starts_with("nile stopped:"), "{error}");
        assert!(!error.to_string().contains(FAKE));
    }

    #[test]
    fn status_names_the_account_or_says_signed_out() {
        let signed_in =
            format!(r#"{{"Username": "sam.plays", "LoggedIn": true, "fake_session": "{FAKE}"}}"#);
        assert_eq!(
            parse_status(&format!("{FAKE}\n{signed_in}")),
            Some(Some(AmazonAccount {
                username: "sam.plays".into()
            }))
        );
        assert_eq!(
            parse_status(r#"{"Username": "<not logged in>", "LoggedIn": false}"#),
            Some(None)
        );
        assert_eq!(parse_status("Traceback (most recent call last):"), None);
        assert_eq!(parse_status("[]"), None);
        assert_eq!(parse_status(r#"{"Username": "x"}"#), None);
    }

    #[test]
    fn the_saved_login_is_read_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path();
        assert!(!has_saved_login(cfg));
        clear_saved_login(&cfg.join("missing")).unwrap();
        fs::write(
            cfg.join(CURRENT_USER_FILE),
            r#"{"name": "Sam", "user_id": ""}"#,
        )
        .unwrap();
        assert!(!has_saved_login(cfg), "a blank id is no login");
        fs::write(
            cfg.join(CURRENT_USER_FILE),
            r#"{"name": "Sam", "user_id": "amzn1.account.X"}"#,
        )
        .unwrap();
        fs::write(cfg.join("0123abcd.enc"), FAKE).unwrap();
        fs::write(cfg.join("library.json"), "[]").unwrap();
        assert!(has_saved_login(cfg));
        assert!(has_token_file(cfg));
        clear_saved_login(cfg).unwrap();
        assert!(!has_saved_login(cfg));
        assert!(!has_token_file(cfg));
        assert!(cfg.join("library.json").exists(), "the library stays");
    }
}
