//! Amazon launch prep: `nile launch --json --no-wine <id>`, a dry run.
//!
//! nile reads the game's `fuel.json` and prints
//! `{"command": {"instruction": <abs exe>, "arguments": [...]},
//! "game_directory": <install>, "env": {FUEL_DIR, AMAZON_GAMES_SDK_PATH,
//! AMAZON_GAMES_FUEL_ENTITLEMENT_ID, AMAZON_GAMES_FUEL_PRODUCT_SKU,
//! AMAZON_GAMES_FUEL_DISPLAY_NAME}}`. There is no token in it, and it
//! needs no network (spike 3). `AMAZON_GAMES_FUEL_DISPLAY_NAME` is the
//! user's given name: personal data, so [`StoreLaunchPrep`]'s `Debug`
//! (counts only) is the only form that is ever printed.
//!
//! GRID owns the game process (`launch::native::build_native_command`),
//! so only the executable, the arguments and the five variables are used.
//! There is no working-folder key: nile applies `WorkingSubdirOverride`
//! to its own launch only, so the row's (from `fuel.json`) is used.

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::list::json_value;
use super::meta::AmazonMeta;
use super::AmazonService;
use crate::launch::{LaunchError, StoreLaunchPrep};
use crate::library::registry::InstalledGame;
use crate::stores::runner::{ToolExit, ToolSignal};
use crate::stores::StoreError;

/// The environment variables nile's dry run names that reach the game.
pub const FUEL_ENV: [&str; 5] = [
    "FUEL_DIR",
    "AMAZON_GAMES_SDK_PATH",
    "AMAZON_GAMES_FUEL_ENTITLEMENT_ID",
    "AMAZON_GAMES_FUEL_PRODUCT_SKU",
    "AMAZON_GAMES_FUEL_DISPLAY_NAME",
];

/// `launch --json --no-wine <id>`: a dry run (`--json` returns before nile
/// would start anything).
pub fn launch_args(id: &str) -> Vec<String> {
    vec![
        "launch".to_string(),
        "--json".to_string(),
        "--no-wine".to_string(),
        id.trim().to_string(),
    ]
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DryRunJson {
    command: Option<DryRunCommand>,
    game_directory: Option<String>,
    env: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DryRunCommand {
    instruction: Option<String>,
    arguments: Option<Vec<serde_json::Value>>,
}

/// A parsed dry run. `Debug` prints counts only: the environment holds the
/// user's given name.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct DryRun {
    pub instruction: String,
    pub arguments: Vec<String>,
    pub game_directory: String,
    /// The [`FUEL_ENV`] variables nile named, in that order.
    pub env: Vec<(String, String)>,
}

impl std::fmt::Debug for DryRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DryRun")
            .field("instruction", &self.instruction)
            .field(
                "arguments",
                &format_args!("<{} args>", self.arguments.len()),
            )
            .field("game_directory", &self.game_directory)
            .field("env", &format_args!("<{} vars>", self.env.len()))
            .finish()
    }
}

/// The dry run in nile's stdout (the first JSON value; other lines are
/// skipped). `None` when there is none or it names no executable.
pub fn parse_dry_run(stdout: &str) -> Option<DryRun> {
    let json: DryRunJson = serde_json::from_value(json_value(stdout)?).ok()?;
    let command = json.command?;
    let instruction = command.instruction.unwrap_or_default().trim().to_string();
    if instruction.is_empty() {
        return None;
    }
    let arguments = command
        .arguments
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .collect();
    let env = FUEL_ENV
        .iter()
        .filter_map(|key| match json.env.get(*key) {
            Some(serde_json::Value::String(value)) => Some((key.to_string(), value.clone())),
            _ => None,
        })
        .collect();
    Some(DryRun {
        instruction,
        arguments,
        game_directory: json.game_directory.unwrap_or_default().trim().to_string(),
        env,
    })
}

/// Whether `exe` is inside `dir` (no `..`, same prefix).
fn inside(exe: &Path, dir: &Path) -> bool {
    !dir.as_os_str().is_empty()
        && exe.is_absolute()
        && exe.starts_with(dir)
        && !exe
            .components()
            .any(|part| matches!(part, Component::ParentDir))
}

/// The launch for `row` from nile's dry run.
///
/// - The executable is nile's, unless the user pinned one in Game Settings
///   or nile's is outside the game folder: then it is left empty and the
///   row's is used. The working folder is always left to the row.
/// - `game_args` are nile's arguments less the row's own launch parameters
///   that lead them (`fuel.json`'s `Main.Args`, which the row carries and
///   `build_native_command` appends), so nothing is passed twice.
/// - `env` is the five FUEL variables. No `secret_args`.
pub fn prep_from_dry_run(run: DryRun, row: &InstalledGame) -> StoreLaunchPrep {
    let meta = AmazonMeta::from_json(&row.store_meta);
    let exe = PathBuf::from(run.instruction.replace('\\', "/"));
    let executable = if meta.exe_user_pinned || !inside(&exe, Path::new(&run.game_directory)) {
        PathBuf::new()
    } else {
        exe
    };
    let lead =
        crate::launch::template::split_template(&row.native_launch_parameters).unwrap_or_default();
    let mut game_args = run.arguments;
    if !lead.is_empty() && game_args.len() >= lead.len() && game_args[..lead.len()] == lead[..] {
        game_args.drain(..lead.len());
    }
    StoreLaunchPrep {
        executable,
        working_dir: PathBuf::new(),
        game_args,
        secret_args: Vec::new(),
        env: run.env,
    }
}

/// GRID's error for a dry run that gave no launch.
fn dry_run_error(exit: &ToolExit) -> StoreError {
    if exit.has(&ToolSignal::NotInstalled) {
        return StoreError::Refused(
            "nile doesn't have this game installed. Verify and repair it, or install it again."
                .to_string(),
        );
    }
    let reason = match exit.code {
        Some(0) if exit.has(&ToolSignal::OtherError) => "it reported an error".to_string(),
        Some(0) => "it gave an answer GRID doesn't understand".to_string(),
        Some(code) if code < 0 => format!("it was stopped (signal {})", -code),
        Some(code) => format!("it exited with code {code}"),
        None => "it was stopped".to_string(),
    };
    StoreError::ToolFailed {
        tool: "nile",
        reason,
    }
}

/// [`AmazonService::launch_prep`]: the dry run, with the quick timeout
/// (nile first tries Amazon's FuelSDK, which can hang).
pub(crate) async fn launch_prep(
    svc: &AmazonService,
    row: &InstalledGame,
) -> Result<StoreLaunchPrep, StoreError> {
    let id = row.store_id.trim();
    if id.is_empty() {
        return Err(StoreError::Refused(
            "GRID doesn't know this game's Amazon Games id.".to_string(),
        ));
    }
    let mut cmd = svc.tool(launch_args(id))?;
    cmd.timeout = Some(super::QUICK_RUN_TIMEOUT);
    let exit = svc.run_blocking(cmd).await?;
    match exit.read_stdout(parse_dry_run) {
        Some(run) if !exit.has(&ToolSignal::NotInstalled) => Ok(prep_from_dry_run(run, row)),
        _ => Err(dry_run_error(&exit)),
    }
}

/// The launch error for a failed [`AmazonService::launch_prep`]:
/// "Couldn't start <title> through nile: …".
pub fn launch_error(title: &str, error: &StoreError) -> LaunchError {
    LaunchError::Validation(format!(
        "Couldn\u{2019}t start {} through nile: {error}",
        title.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    const FAKE: &str = "FAKE-STORE-TOKEN-not-real";

    fn dry_run_json() -> String {
        format!(
            r#"{{"command": {{"instruction": "/games/Amazon Games/Fogline/bin/Fogline.exe", "arguments": ["-windowed", "--lang=en", 3]}},
              "game_directory": "/games/Amazon Games/Fogline",
              "env": {{"FUEL_DIR": "/d/stores/nile/nile/SDK/Amazon Games Services/Legacy",
                       "AMAZON_GAMES_SDK_PATH": "/d/stores/nile/nile/SDK/Amazon Games Services/AmazonGamesSDK",
                       "AMAZON_GAMES_FUEL_ENTITLEMENT_ID": "ent-1", "AMAZON_GAMES_FUEL_PRODUCT_SKU": "SKU1",
                       "AMAZON_GAMES_FUEL_DISPLAY_NAME": "FakeGivenName", "WINEPREFIX": "/elsewhere"}},
              "fake_session": "{FAKE}"}}"#
        )
    }

    fn row(params: &str, pinned: bool) -> InstalledGame {
        InstalledGame {
            title: "Fogline".into(),
            store_id: "amzn1.adg.product.aaaa".into(),
            native_launch_parameters: params.into(),
            store_meta: AmazonMeta {
                product_id: "amzn1.adg.product.aaaa".into(),
                version: "v1".into(),
                exe_user_pinned: pinned,
                ..Default::default()
            }
            .to_json(),
            ..Default::default()
        }
    }

    #[test]
    fn the_dry_run_argv_never_debugs_or_starts_wine() {
        assert_eq!(
            launch_args(" amzn1.adg.product.aaaa "),
            ["launch", "--json", "--no-wine", "amzn1.adg.product.aaaa"]
        );
        assert!(!launch_args("x")
            .iter()
            .any(|a| a == "-d" || a == "--override-exe"));
    }

    #[test]
    fn the_dry_run_parses_amid_other_lines_and_keeps_only_fuel_env() {
        let stdout = format!("{FAKE}\n{}\n", dry_run_json());
        let run = parse_dry_run(&stdout).unwrap();
        assert_eq!(
            run.instruction,
            "/games/Amazon Games/Fogline/bin/Fogline.exe"
        );
        assert_eq!(run.arguments, ["-windowed", "--lang=en"]);
        assert_eq!(run.env.len(), 5);
        assert!(run.env.iter().all(|(k, _)| FUEL_ENV.contains(&k.as_str())));
        let printed = format!("{run:?}");
        assert!(!printed.contains("FakeGivenName"), "{printed}");
        assert!(parse_dry_run(r#"{"command": {"instruction": ""}}"#).is_none());
        assert!(parse_dry_run("Traceback (most recent call last):").is_none());
    }

    #[test]
    fn the_prep_drops_the_rows_own_args_and_respects_a_pinned_exe() {
        let run = parse_dry_run(&dry_run_json()).unwrap();
        let prep = prep_from_dry_run(run.clone(), &row("-windowed --lang=en", false));
        assert_eq!(
            prep.executable,
            PathBuf::from("/games/Amazon Games/Fogline/bin/Fogline.exe")
        );
        assert_eq!(prep.working_dir, PathBuf::new(), "the row's is used");
        assert!(prep.game_args.is_empty(), "no doubling");
        assert!(prep.secret_args.is_empty());
        assert_eq!(prep.env.len(), 5);
        let printed = format!("{prep:?}");
        assert!(!printed.contains("FakeGivenName"), "{printed}");

        let prep = prep_from_dry_run(run.clone(), &row("", false));
        assert_eq!(prep.game_args, ["-windowed", "--lang=en"]);
        let prep = prep_from_dry_run(run.clone(), &row("-other", false));
        assert_eq!(prep.game_args, ["-windowed", "--lang=en"]);

        let pinned = prep_from_dry_run(run.clone(), &row("-windowed --lang=en", true));
        assert_eq!(pinned.executable, PathBuf::new());
        assert_eq!(pinned.env.len(), 5);

        let mut outside = run;
        outside.instruction = "/games/Amazon Games/Fogline/../../evil.exe".into();
        assert_eq!(
            prep_from_dry_run(outside, &row("", false)).executable,
            PathBuf::new()
        );
    }

    #[test]
    fn dry_run_errors_are_grids_words() {
        let exit = |code: i32, signals: Vec<ToolSignal>| ToolExit {
            code: Some(code),
            stdout: SecretString::from(FAKE),
            signals,
        };
        let cases = [
            (
                exit(0, vec![ToolSignal::NotInstalled]),
                "doesn't have this game installed",
            ),
            (
                exit(0, vec![ToolSignal::OtherError]),
                "nile stopped: it reported an error",
            ),
            (
                exit(1, vec![ToolSignal::OtherError]),
                "nile stopped: it exited with code 1",
            ),
            (exit(0, vec![]), "doesn't understand"),
        ];
        for (exit, words) in cases {
            let error = dry_run_error(&exit);
            assert!(error.to_string().contains(words), "{error}");
            assert!(!error.to_string().contains(FAKE));
        }
        let LaunchError::Validation(text) =
            launch_error(" Fogline ", &StoreError::ToolMissing("nile"))
        else {
            panic!("a validation error");
        };
        assert_eq!(
            text,
            "Couldn\u{2019}t start Fogline through nile: nile isn't installed"
        );
    }
}
