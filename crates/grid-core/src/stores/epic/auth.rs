//! Epic login through legendary: the argv (the code never on it), the
//! login outcome read from legendary's signals, the account from
//! `status --json`, and the first-run `config.ini`.
//!
//! legendary's `auth --disable-webview` prints a prompt on stdout and reads
//! the code from stdin (spike 2); GRID writes it there through
//! [`crate::stores::runner::ToolCommand::stdin`]. legendary exits 0 when a
//! login fails, and exits 1 before reading stdin when api.legendary.gl is
//! unreachable, so a login counts as done only when the run exited 0, no
//! failure signal came, and `user.json` exists.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use super::list::json_value;
use super::EpicAccount;
use crate::stores::runner::{ToolExit, ToolSignal};
use crate::stores::StoreError;

/// legendary's token file in its config folder.
pub const USER_FILE: &str = "user.json";
/// legendary's settings file in its config folder.
pub const CONFIG_FILE: &str = "config.ini";
/// What `status --json` says for `account` when signed out.
pub const NOT_LOGGED_IN: &str = "<not logged in>";

/// The `config.ini` GRID writes before legendary's first run: no update
/// notice (the update check itself stays on; it fetches Epic's client
/// changes from api.legendary.gl).
pub const FIRST_RUN_CONFIG: &str = "[Legendary]\ndisable_update_notice = true\n";

/// `auth --disable-webview`: the code comes on stdin, never as `--code`.
pub fn login_args() -> Vec<String> {
    vec!["auth".into(), "--disable-webview".into()]
}

/// `auth --delete`: deletes `user.json` (it does not revoke the session on
/// Epic's side).
pub fn logout_args() -> Vec<String> {
    vec!["auth".into(), "--delete".into()]
}

/// `status --json`, with `--offline` when legendary should not refresh the
/// login (and not run its update check).
pub fn status_args(offline: bool) -> Vec<String> {
    let mut args = vec!["status".to_string(), "--json".to_string()];
    if offline {
        args.push("--offline".into());
    }
    args
}

/// Writes [`FIRST_RUN_CONFIG`] into legendary's folder when it has no
/// `config.ini` yet. legendary keeps the value and adds its own defaults.
pub fn ensure_first_run_config(tool_dir: &Path) -> io::Result<()> {
    let path = tool_dir.join(CONFIG_FILE);
    if path.exists() {
        return Ok(());
    }
    crate::stores::paths::ensure_private_dir(tool_dir)?;
    fs::write(&path, FIRST_RUN_CONFIG)?;
    crate::stores::paths::make_private_file(&path)
}

/// Whether a login run worked: exit 0, no failure signal, and legendary
/// wrote its token file. Otherwise GRID's words for why not.
pub fn login_outcome(exit: &ToolExit, user_file_exists: bool) -> Result<(), StoreError> {
    if exit.has(&ToolSignal::Network) {
        return Err(StoreError::Network("Epic Games".to_string()));
    }
    if exit.has(&ToolSignal::LoginFailed) {
        return Err(StoreError::Refused(
            "Epic Games didn't accept the sign-in. Try again.".to_string(),
        ));
    }
    if !exit.success() || exit.has(&ToolSignal::OtherError) || !user_file_exists {
        return Err(StoreError::ToolFailed {
            tool: "legendary",
            reason: "it couldn't finish the sign-in".to_string(),
        });
    }
    Ok(())
}

/// The account `status --json` names: `Some(None)` when it says signed
/// out, `None` when stdout is not a status answer.
pub fn parse_status(stdout: &str) -> Option<Option<EpicAccount>> {
    let value = json_value(stdout)?;
    let account = value.as_object()?.get("account")?;
    let name = match account {
        Value::String(name) => name.trim().to_string(),
        Value::Null => String::new(),
        _ => return None,
    };
    if name.is_empty() || name == NOT_LOGGED_IN {
        Some(None)
    } else {
        Some(Some(EpicAccount { username: name }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    fn exit(code: i32, signals: Vec<ToolSignal>) -> ToolExit {
        ToolExit {
            code: Some(code),
            stdout: SecretString::from("FAKE-STORE-TOKEN-not-real"),
            signals,
        }
    }

    #[test]
    fn the_code_is_never_an_argument() {
        for args in [
            login_args(),
            logout_args(),
            status_args(true),
            status_args(false),
        ] {
            assert!(!args.iter().any(|a| a.contains("--code")), "{args:?}");
            assert!(!args.iter().any(|a| a.contains("FAKE")), "{args:?}");
        }
        assert_eq!(login_args(), ["auth", "--disable-webview"]);
        assert_eq!(logout_args(), ["auth", "--delete"]);
        assert_eq!(status_args(true), ["status", "--json", "--offline"]);
        assert_eq!(status_args(false), ["status", "--json"]);
    }

    #[test]
    fn a_login_needs_exit_0_no_failure_and_the_token_file() {
        assert!(login_outcome(&exit(0, vec![]), true).is_ok());
        let refused = login_outcome(&exit(0, vec![ToolSignal::LoginFailed]), false).unwrap_err();
        assert!(refused.to_string().contains("didn't accept"), "{refused}");
        let offline = login_outcome(
            &exit(0, vec![ToolSignal::Network, ToolSignal::LoginFailed]),
            false,
        )
        .unwrap_err();
        assert!(matches!(offline, StoreError::Network(_)), "{offline}");
        // api.legendary.gl unreachable: a traceback and exit 1, stdin unread.
        let early = login_outcome(&exit(1, vec![ToolSignal::Network]), false).unwrap_err();
        assert!(matches!(early, StoreError::Network(_)));
        for (code, signals, file) in [
            (1, vec![], true),
            (0, vec![ToolSignal::OtherError], true),
            (0, vec![], false),
        ] {
            let error = login_outcome(&exit(code, signals), file).unwrap_err();
            assert!(
                error.to_string().starts_with("legendary stopped:"),
                "{error}"
            );
            assert!(!error.to_string().contains("FAKE"));
        }
    }

    #[test]
    fn status_names_the_account_or_says_signed_out() {
        let signed_in = r#"{"account": "SamPlays", "games_available": 6, "games_installed": 2, "egl_sync_enabled": false, "config_directory": "/d/stores/legendary"}"#;
        assert_eq!(
            parse_status(signed_in),
            Some(Some(EpicAccount {
                username: "SamPlays".into()
            }))
        );
        let out = r#"{"account": "<not logged in>", "games_available": 0, "games_installed": 0, "egl_sync_enabled": false, "config_directory": "/x"}"#;
        assert_eq!(parse_status(out), Some(None));
        assert_eq!(parse_status("Traceback (most recent call last):"), None);
        assert_eq!(parse_status("[]"), None);
    }

    #[test]
    fn the_first_run_config_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("legendary");
        ensure_first_run_config(&tool).unwrap();
        let path = tool.join(CONFIG_FILE);
        assert_eq!(fs::read_to_string(&path).unwrap(), FIRST_RUN_CONFIG);
        fs::write(
            &path,
            "[Legendary]\ndisable_update_notice = true\nlocale = en-US\n",
        )
        .unwrap();
        ensure_first_run_config(&tool).unwrap();
        assert!(
            fs::read_to_string(&path).unwrap().contains("locale"),
            "kept as is"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&tool).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
    }
}
