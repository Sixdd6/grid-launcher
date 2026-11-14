//! Epic launch tickets: `legendary launch <app> --json --skip-version-check`.
//!
//! legendary prints its `LaunchParameters` on stdout. Their
//! `egl_parameters` carry Epic's exchange code (`-AUTH_PASSWORD=…`), a
//! short-lived, single-use credential: they become
//! [`StoreLaunchPrep::secret_args`], appended last to the game's argv as
//! redacted positions (`NativeLaunch::redact`), so the early-exit message
//! and every printed argv show `<redacted>`. This file is the only place
//! the ticket is parsed. `--dry-run` is never used (it logs the whole
//! command line, code included).
//!
//! GRID owns the game process: legendary's `launch_command` and
//! `pre_launch_command` are ignored, because GRID's compat tool wraps the
//! game (`launch::native::build_native_command`).
//!
//! Every legendary run goes through [`EpicService::tool`] (env, parser,
//! private folder, the test override); [`tool_error`] maps a failed run to
//! GRID's words for the launch and cloud code.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::meta::EpicMeta;
use super::EpicService;
use crate::launch::{LaunchError, StoreLaunchPrep};
use crate::library::registry::InstalledGame;
use crate::stores::runner::{ToolExit, ToolSignal};
use crate::stores::tools::StoreTool;
use crate::stores::StoreError;

/// The words in front of every launch-ticket failure.
pub const LAUNCH_TICKET_FAILED: &str = "Couldn\u{2019}t get a launch ticket from Epic Games";

/// Environment keys legendary may name that belong to GRID's compat tool.
const COMPAT_ENV: [&str; 4] = [
    "WINEPREFIX",
    "CX_BOTTLE",
    "STEAM_COMPAT_DATA_PATH",
    "PROTONPATH",
];

// ---------------------------------------------------------------------
// The legendary invocation helper (shared with `cloud.rs`)
// ---------------------------------------------------------------------

/// GRID's error for a legendary run that failed, from its signals (the
/// lines themselves are never kept). Specific signals win over
/// `OtherError`.
pub(crate) fn tool_error(exit: &ToolExit) -> StoreError {
    if exit.has(&ToolSignal::Network) {
        return StoreError::Network("Epic Games".to_string());
    }
    if exit.has(&ToolSignal::NotLoggedIn) || exit.has(&ToolSignal::LoginFailed) {
        return StoreError::LoginExpired;
    }
    if exit.has(&ToolSignal::NotInstalled) {
        return StoreError::Refused(
            "legendary doesn\u{2019}t have this game installed. Verify and repair it, or install it again."
                .to_string(),
        );
    }
    let reason = match exit.code {
        Some(0) if exit.has(&ToolSignal::OtherError) => "it reported an error".to_string(),
        Some(0) => "it gave an answer GRID doesn\u{2019}t understand".to_string(),
        Some(code) if code < 0 => format!("it was stopped (signal {})", -code),
        Some(code) => format!("it exited with code {code}"),
        None => "it was stopped".to_string(),
    };
    StoreError::ToolFailed {
        tool: StoreTool::Legendary.name(),
        reason,
    }
}

// ---------------------------------------------------------------------
// The launch ticket
// ---------------------------------------------------------------------

/// `launch <app> --json --skip-version-check [--offline]`.
pub fn launch_args(app: &str, offline: bool) -> Vec<String> {
    let mut args = vec![
        "launch".to_string(),
        app.trim().to_string(),
        "--json".to_string(),
        "--skip-version-check".to_string(),
    ];
    if offline {
        args.push("--offline".to_string());
    }
    args
}

/// legendary's `LaunchParameters`, the fields GRID reads. Holds the
/// exchange code: no `Debug`, never printed.
#[derive(Deserialize, Default)]
#[serde(default)]
struct LaunchTicketJson {
    game_executable: String,
    game_directory: String,
    working_directory: String,
    game_parameters: Vec<String>,
    egl_parameters: Vec<String>,
    environment: BTreeMap<String, serde_json::Value>,
}

/// A parsed launch ticket. `Debug` prints counts only: `egl_parameters`
/// carry the exchange code.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LaunchTicket {
    pub game_executable: String,
    pub game_directory: String,
    pub working_directory: String,
    pub game_parameters: Vec<String>,
    pub egl_parameters: Vec<String>,
    pub environment: Vec<(String, String)>,
}

impl std::fmt::Debug for LaunchTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchTicket")
            .field("game_executable", &self.game_executable)
            .field("game_directory", &self.game_directory)
            .field("working_directory", &self.working_directory)
            .field(
                "game_parameters",
                &format_args!("<{} args>", self.game_parameters.len()),
            )
            .field(
                "egl_parameters",
                &format_args!("<{} redacted>", self.egl_parameters.len()),
            )
            .field(
                "environment",
                &format_args!("<{} vars>", self.environment.len()),
            )
            .finish()
    }
}

/// The ticket in legendary's stdout: the whole text, else the last line
/// that parses as a JSON object (other lines are ignored). `None` when
/// there is none, or it names no executable.
pub fn parse_launch_ticket(stdout: &str) -> Option<LaunchTicket> {
    let parse = |text: &str| serde_json::from_str::<LaunchTicketJson>(text.trim()).ok();
    let json = parse(stdout).or_else(|| {
        stdout
            .lines()
            .rev()
            .filter(|line| line.trim_start().starts_with('{'))
            .find_map(parse)
    })?;
    if json.game_executable.trim().is_empty() {
        return None;
    }
    Some(LaunchTicket {
        game_executable: json.game_executable,
        game_directory: json.game_directory,
        working_directory: json.working_directory,
        game_parameters: json.game_parameters,
        egl_parameters: json.egl_parameters,
        environment: json
            .environment
            .into_iter()
            .filter_map(|(key, value)| match value {
                serde_json::Value::String(text) => Some((key, text)),
                _ => None,
            })
            .collect(),
    })
}

/// Python's `shlex.split(text, posix=False)`, as legendary splits an
/// installed game's launch parameters: whitespace separates, a quoted
/// stretch stays in its token with its quotes.
pub fn split_like_legendary(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == q {
                    quote = None;
                }
            }
            None if c.is_whitespace() => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                }
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

/// The game's launch parameters as legendary keeps them
/// (`installed.json`), `None` when legendary has no record of the game.
fn installed_launch_parameters(tool_dir: &Path, app: &str) -> Option<String> {
    let text = std::fs::read_to_string(tool_dir.join("installed.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get(app)?
        .get("launch_parameters")?
        .as_str()
        .map(str::to_string)
}

/// The launch for `row` from `ticket`.
///
/// - The executable is legendary's (`game_directory` + `game_executable`),
///   unless the user pinned one in Game Settings or legendary's climbs out
///   of the game folder: then it is left empty and the row's is used.
/// - The working folder is legendary's, with its executable.
/// - `game_args` are legendary's `game_parameters` less the installed
///   launch parameters it puts first (`installed`, as legendary keeps
///   them): the row carries those (the user may edit them), and
///   `build_native_command` appends the row's after `game_args`.
/// - `secret_args` are every `egl_parameters` entry.
/// - `env` is legendary's environment, less the compat tool's keys.
pub fn prep_from_ticket(
    ticket: LaunchTicket,
    row: &InstalledGame,
    installed: Option<&str>,
) -> StoreLaunchPrep {
    let meta = EpicMeta::from_json(&row.store_meta);
    let relative = ticket.game_executable.replace('\\', "/");
    let relative = relative.trim().trim_start_matches('/');
    let climbs = Path::new(relative)
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir));
    let (executable, working_dir) =
        if meta.exe_user_pinned || climbs || ticket.game_directory.trim().is_empty() {
            (PathBuf::new(), PathBuf::new())
        } else {
            let executable = Path::new(ticket.game_directory.trim()).join(relative);
            let working_dir = if ticket.working_directory.trim().is_empty() {
                PathBuf::new()
            } else {
                PathBuf::from(ticket.working_directory.trim())
            };
            (executable, working_dir)
        };

    let installed = installed
        .map(str::to_string)
        .unwrap_or_else(|| row.native_launch_parameters.clone());
    let lead = split_like_legendary(&installed);
    let mut game_args = ticket.game_parameters;
    if !lead.is_empty() && game_args.len() >= lead.len() && game_args[..lead.len()] == lead[..] {
        game_args.drain(..lead.len());
    }

    let env = ticket
        .environment
        .into_iter()
        .filter(|(key, _)| !COMPAT_ENV.iter().any(|k| k.eq_ignore_ascii_case(key)))
        .collect();

    StoreLaunchPrep {
        executable,
        working_dir,
        game_args,
        secret_args: ticket.egl_parameters,
        env,
    }
}

/// The ticket a run printed, or GRID's error for it.
fn ticket_from(exit: &ToolExit) -> Result<LaunchTicket, StoreError> {
    if exit.success() {
        if let Some(ticket) = exit.read_stdout(parse_launch_ticket) {
            return Ok(ticket);
        }
    }
    Err(tool_error(exit))
}

/// [`EpicService::launch_prep`]: runs `launch --json` once, with
/// `--offline` when asked (no exchange code then). The one offline retry
/// on a network failure is the app's launch hook's
/// (`stores_service::epic::EpicStore::launch_ticket`), which also knows
/// whether the store is signed in; this function never retries.
pub(crate) async fn launch_prep(
    svc: &EpicService,
    row: &InstalledGame,
    offline: bool,
) -> Result<StoreLaunchPrep, StoreError> {
    let app = row.store_id.trim();
    if app.is_empty() {
        return Err(StoreError::Refused(
            "GRID doesn\u{2019}t know this game\u{2019}s Epic Games name.".to_string(),
        ));
    }
    let exit = svc
        .run_blocking(svc.tool(launch_args(app, offline))?)
        .await?;
    let ticket = ticket_from(&exit)?;
    let installed = installed_launch_parameters(&svc.tool_dir(), app);
    Ok(prep_from_ticket(ticket, row, installed.as_deref()))
}

/// The launch error for a failed [`EpicService::launch_prep`], in GRID's
/// words: "Couldn't get a launch ticket from Epic Games: …".
pub fn launch_ticket_error(error: &StoreError) -> LaunchError {
    let reason = match error {
        StoreError::LoginExpired | StoreError::NotSignedIn => {
            "your Epic Games login expired. Sign in again.".to_string()
        }
        StoreError::Network(_) => {
            "Epic Games didn\u{2019}t answer. Check your connection and try again.".to_string()
        }
        other => other.to_string(),
    };
    LaunchError::Validation(format!("{LAUNCH_TICKET_FAILED}: {reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::native::{build_native_command, redact_argv};
    use secrecy::SecretString;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    fn ticket_json() -> String {
        format!(
            r#"{{"game_executable": "Binaries/Win64/Starlight.exe", "game_directory": "/games/Epic Games/Starlight",
            "working_directory": "/games/Epic Games/Starlight/Binaries/Win64",
            "game_parameters": ["-nolauncher", "-meta"], "user_parameters": ["-user"],
            "egl_parameters": ["-AUTH_LOGIN=unused", "-AUTH_PASSWORD={FAKE}", "-AUTH_TYPE=exchangecode",
              "-epicapp=Starlight", "-epicenv=Prod", "-EpicPortal", "-epicusername=SamPlays"],
            "launch_command": ["wine"], "pre_launch_command": "rm -rf /", "pre_launch_wait": false,
            "environment": {{"WINEPREFIX": "/elsewhere", "DXVK_HUD": "1", "N": 3}}}}"#
        )
    }

    fn row() -> InstalledGame {
        InstalledGame {
            title: "Starlight Courier".into(),
            platform: "Epic Games".into(),
            platform_slug: "win".into(),
            store_id: "Starlight".into(),
            source: crate::stores::GameSource::Store(crate::stores::StoreKind::Epic),
            native_game_dir: "/games/Epic Games/Starlight".into(),
            extracted_dir: "/games/Epic Games/Starlight".into(),
            native_executable_path: "/games/Epic Games/Starlight/Binaries/Win64/Starlight.exe"
                .into(),
            native_launch_parameters: "-nolauncher".into(),
            store_meta: EpicMeta {
                app_name: "Starlight".into(),
                can_run_offline: true,
                ..Default::default()
            }
            .to_json(),
            ..Default::default()
        }
    }

    #[test]
    fn launch_argv_never_dry_runs() {
        assert_eq!(
            launch_args(" Starlight ", false),
            ["launch", "Starlight", "--json", "--skip-version-check"]
        );
        let offline = launch_args("Starlight", true);
        assert_eq!(offline.last().map(String::as_str), Some("--offline"));
        assert!(!offline.iter().any(|a| a == "--dry-run"));
    }

    #[test]
    fn the_ticket_parses_amid_other_stdout_lines() {
        let one_line = ticket_json().replace('\n', " ");
        let stdout = format!("{FAKE}\nsome notice\n{one_line}\n");
        let ticket = parse_launch_ticket(&stdout).unwrap();
        assert_eq!(ticket.game_executable, "Binaries/Win64/Starlight.exe");
        assert_eq!(ticket.egl_parameters.len(), 7);
        assert_eq!(
            ticket.environment,
            vec![
                ("DXVK_HUD".to_string(), "1".to_string()),
                ("WINEPREFIX".to_string(), "/elsewhere".to_string())
            ]
        );
        // The whole text as one (pretty) JSON object parses too.
        assert!(parse_launch_ticket(&ticket_json()).is_some());
        assert!(parse_launch_ticket("").is_none());
        assert!(parse_launch_ticket("{\"game_executable\": \"\"}").is_none());
        assert!(parse_launch_ticket("not json").is_none());
    }

    #[test]
    fn the_exchange_code_never_prints() {
        let ticket = parse_launch_ticket(&ticket_json()).unwrap();
        let shown = format!("{ticket:?}");
        assert!(!shown.contains(FAKE), "{shown}");
        assert!(shown.contains("<7 redacted>"), "{shown}");

        let prep = prep_from_ticket(ticket, &row(), Some("-nolauncher"));
        let shown = format!("{prep:?}");
        assert!(!shown.contains(FAKE), "{shown}");

        // In the built argv the code is a redacted position: what may be
        // printed (the early-exit message) shows `<redacted>`.
        let launch = build_native_command(
            &row(),
            Path::new("/lib"),
            "",
            "windows",
            &|_| None,
            Some(&prep),
        )
        .unwrap();
        assert!(launch.argv.iter().any(|a| a.contains(FAKE)));
        let printed = redact_argv(&launch.argv, &launch.redact);
        assert!(!printed.iter().any(|a| a.contains(FAKE)), "{printed:?}");
        assert!(printed
            .iter()
            .any(|a| a == crate::launch::native::REDACTED_ARG));
        assert_eq!(launch.redact.len(), 7);
        assert_eq!(
            launch.redact.first().copied(),
            Some(launch.argv.len() - 7),
            "the secrets come last"
        );
    }

    #[test]
    fn the_prep_takes_legendarys_executable_and_drops_the_duplicate_parameters() {
        let ticket = parse_launch_ticket(&ticket_json()).unwrap();
        let prep = prep_from_ticket(ticket, &row(), Some("-nolauncher"));
        assert_eq!(
            prep.executable,
            Path::new("/games/Epic Games/Starlight/Binaries/Win64/Starlight.exe")
        );
        assert_eq!(
            prep.working_dir,
            Path::new("/games/Epic Games/Starlight/Binaries/Win64")
        );
        // `-nolauncher` comes from the row; legendary's own copy goes.
        assert_eq!(prep.game_args, ["-meta"]);
        assert_eq!(prep.secret_args.len(), 7);
        assert_eq!(prep.env, vec![("DXVK_HUD".to_string(), "1".to_string())]);

        let launch = build_native_command(
            &row(),
            Path::new("/lib"),
            "",
            "windows",
            &|_| None,
            Some(&prep),
        )
        .unwrap();
        assert_eq!(launch.argv[1..3], ["-meta", "-nolauncher"]);
        assert_eq!(
            launch.argv.iter().filter(|a| *a == "-nolauncher").count(),
            1
        );
    }

    #[test]
    fn a_pinned_or_escaping_executable_falls_back_to_the_row() {
        let mut pinned = row();
        pinned.store_meta = EpicMeta {
            exe_user_pinned: true,
            ..Default::default()
        }
        .to_json();
        let prep = prep_from_ticket(parse_launch_ticket(&ticket_json()).unwrap(), &pinned, None);
        assert!(prep.executable.as_os_str().is_empty());
        assert!(prep.working_dir.as_os_str().is_empty());

        let mut ticket = parse_launch_ticket(&ticket_json()).unwrap();
        ticket.game_executable = "../../bin/sh".into();
        let prep = prep_from_ticket(ticket, &row(), None);
        assert!(prep.executable.as_os_str().is_empty());
    }

    #[test]
    fn edited_row_parameters_still_drop_legendarys_copy() {
        let mut edited = row();
        edited.native_launch_parameters = "-windowed".into();
        let ticket = parse_launch_ticket(&ticket_json()).unwrap();
        // legendary's installed.json still says `-nolauncher`.
        let prep = prep_from_ticket(ticket.clone(), &edited, Some("-nolauncher"));
        assert_eq!(prep.game_args, ["-meta"]);
        // Without installed.json the row's are compared: no match, kept.
        let prep = prep_from_ticket(ticket, &edited, None);
        assert_eq!(prep.game_args, ["-nolauncher", "-meta"]);
    }

    #[test]
    fn legendary_splits_keep_quotes() {
        assert_eq!(
            split_like_legendary(r#" -a  "b c" 'd e'f -g"#),
            [r#"-a"#, r#""b c""#, "'d e'f", "-g"]
        );
        assert!(split_like_legendary("   ").is_empty());
    }

    fn exit(code: i32, signals: Vec<ToolSignal>) -> ToolExit {
        ToolExit {
            code: Some(code),
            stdout: SecretString::from(String::new()),
            signals,
        }
    }

    #[test]
    fn failures_get_grids_words() {
        let network = tool_error(&exit(1, vec![ToolSignal::Network, ToolSignal::LoginFailed]));
        assert!(matches!(network, StoreError::Network(_)));
        assert!(matches!(
            tool_error(&exit(1, vec![ToolSignal::NotLoggedIn])),
            StoreError::LoginExpired
        ));
        assert!(tool_error(&exit(1, vec![ToolSignal::NotInstalled]))
            .to_string()
            .contains("doesn\u{2019}t have this game installed"));
        assert_eq!(
            tool_error(&exit(3, vec![])).to_string(),
            "legendary stopped: it exited with code 3"
        );
        let words = launch_ticket_error(&StoreError::LoginExpired).to_string();
        assert_eq!(
            words,
            "Couldn\u{2019}t get a launch ticket from Epic Games: your Epic Games login expired. Sign in again."
        );
        let words = launch_ticket_error(&network).to_string();
        assert!(words.starts_with(LAUNCH_TICKET_FAILED), "{words}");
        assert!(words.contains("didn\u{2019}t answer"), "{words}");
    }

    #[test]
    fn a_successful_exit_without_a_ticket_is_an_error() {
        let mut ok = exit(0, vec![]);
        assert!(ticket_from(&ok).is_err());
        ok.stdout = SecretString::from(ticket_json());
        assert!(ticket_from(&ok).is_ok());
    }
}
