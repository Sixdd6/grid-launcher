//! The registry row of an installed Amazon game, built from its nile
//! `installed.json` entry, its [`AmazonMeta`], the game's `fuel.json` and
//! (when GRID has it) the owned list's details. Every Amazon install path
//! writes its row through [`amazon_row`], so they all launch alike.
//!
//! The row takes the native launch branch (`launch::native`): it sets
//! `extracted_dir` and `native_game_dir` (the install folder), the
//! executable `fuel.json` names (`Main.Command`) as an absolute path under
//! it, `Main.Args` as the launch parameters, the working folder
//! (`Main.WorkingSubdirOverride`, else the executable's own) and the `win`
//! platform slug: every Amazon game is a Windows build, so it runs through
//! the compat tool off Windows.
//!
//! `fuel.json` is JSON5 in the wild (nile reads it with `json5.loads`):
//! [`parse_fuel`] drops comments and trailing commas, then reads it as
//! JSON. [`parse_post_install`] reads its `PostInstall` steps (the
//! redistributable installers `stores::redist` runs after a job).

use std::path::Path;

use serde::Deserialize;

use super::meta::AmazonMeta;
use super::{AmazonGame, AmazonInstalled};
use crate::library::registry::InstalledGame;
use crate::library::specials::native::{LAUNCH_SOURCE_SIDECAR, LAUNCH_SOURCE_USER};
use crate::stores::gog::row::relative_path;
use crate::stores::{GameSource, StoreKind};

/// The file in an install folder that says how the game starts.
pub const FUEL_FILE: &str = "fuel.json";

/// `fuel.json`'s `Main` block: how the game starts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FuelMain {
    /// `Main.Command`: the executable, relative to the install folder,
    /// either separator.
    pub command: String,
    /// `Main.Args`.
    pub args: Vec<String>,
    /// `Main.WorkingSubdirOverride`, `""` for none.
    pub working_subdir: String,
}

/// The `fuel.json` in `install`, `None` when it is missing or unreadable or
/// names no command.
pub fn read_fuel(install: &Path) -> Option<FuelMain> {
    let text = std::fs::read_to_string(install.join(FUEL_FILE)).ok()?;
    parse_fuel(&text)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FuelFile {
    #[serde(rename = "Main")]
    main: Option<FuelMainJson>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FuelMainJson {
    #[serde(rename = "Command")]
    command: Option<String>,
    #[serde(rename = "Args")]
    args: Option<Vec<String>>,
    #[serde(rename = "WorkingSubdirOverride")]
    working_subdir: Option<String>,
}

/// A `fuel.json` text's `Main` block. Comments and trailing commas (JSON5)
/// are allowed; unknown keys and `null`s are ignored. `None` when the text
/// is not readable or `Main.Command` is missing or blank.
pub fn parse_fuel(text: &str) -> Option<FuelMain> {
    let json = strip_json5(text.trim_start_matches('\u{feff}'));
    let file: FuelFile = serde_json::from_str(&json).ok()?;
    let main = file.main?;
    let command = main.command.unwrap_or_default().trim().to_string();
    if command.is_empty() {
        return None;
    }
    Some(FuelMain {
        command,
        args: main.args.unwrap_or_default(),
        working_subdir: main.working_subdir.unwrap_or_default().trim().to_string(),
    })
}

/// `text` with `//` and `/* */` comments and trailing commas (before `}`
/// or `]`) removed, outside strings.
fn strip_json5(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    // Pass 1: comments.
    let mut plain = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            plain.push(c);
            if c == '\\' && i + 1 < chars.len() {
                plain.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match (c, chars.get(i + 1)) {
            ('"', _) => {
                in_string = true;
                plain.push(c);
                i += 1;
            }
            ('/', Some('/')) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    i += 1;
                }
                i += 2;
                plain.push(' ');
            }
            _ => {
                plain.push(c);
                i += 1;
            }
        }
    }
    // Pass 2: trailing commas.
    let chars: Vec<char> = plain.chars().collect();
    let mut out = String::with_capacity(plain.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
        }
        if c == ',' {
            let next = chars[i + 1..].iter().find(|ch| !ch.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `args` as one launch-parameter string that `launch::template`'s split
/// gives back unchanged: a plain argument as it is, any other in single
/// quotes (a `'` inside becomes `'\''`). Windows paths keep their `\`.
pub fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            let plain = !arg.is_empty()
                && arg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_=.,/:+@%".contains(c));
            if plain {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One `fuel.json` `PostInstall` step (`{"Command": "oalinst.exe", "Args":
/// ["/silent"], "ValidReturns": [0]}`): the store-neutral
/// [`crate::stores::redist::RedistStep`], with no `id` (Amazon gives none).
/// No secret: [`AmazonMeta`] keeps the list that last worked.
pub use crate::stores::redist::RedistStep as FuelStep;

/// The `PostInstall` steps of the `fuel.json` in `install`, in order;
/// empty when the file is missing or unreadable or lists none.
pub fn read_post_install(install: &Path) -> Vec<FuelStep> {
    std::fs::read_to_string(install.join(FUEL_FILE))
        .map(|text| parse_post_install(&text))
        .unwrap_or_default()
}

/// A `fuel.json` text's `PostInstall` steps, in order. JSON5 as
/// [`parse_fuel`]. A step with no `Command` (or a blank one) is left out;
/// a number or `true`/`false` in `Args` is kept as its text, anything else
/// there is dropped.
pub fn parse_post_install(text: &str) -> Vec<FuelStep> {
    let json = strip_json5(text.trim_start_matches('\u{feff}'));
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return Vec::new();
    };
    let Some(steps) = value.get("PostInstall").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    steps
        .iter()
        .filter_map(|step| {
            let command = step.get("Command")?.as_str()?.trim().to_string();
            if command.is_empty() {
                return None;
            }
            let args = step
                .get("Args")
                .and_then(|v| v.as_array())
                .map(|args| {
                    args.iter()
                        .filter_map(|arg| match arg {
                            serde_json::Value::String(text) => Some(text.clone()),
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            serde_json::Value::Bool(b) => Some(b.to_string()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let valid_returns = step
                .get("ValidReturns")
                .and_then(|v| v.as_array())
                .map(|codes| codes.iter().filter_map(|c| c.as_i64()).collect())
                .unwrap_or_default();
            Some(FuelStep {
                id: String::new(),
                command,
                args,
                valid_returns,
                root: String::new(),
            })
        })
        .collect()
}

/// A relative path's parts joined with `/`.
fn slash_path(rel: &Path) -> String {
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The row for the Amazon game `inst` describes.
///
/// The title is the owned list's when `game` is given, else the product
/// id. The executable is the install folder + `exe.command` (either
/// separator); one that is absolute, has a drive letter or climbs out
/// (`..`), or no `fuel.json` at all, leaves it blank, and Game Settings
/// asks the user for one. `native_working_dir` is `exe.working_subdir`
/// when it is a safe relative path, else the executable's folder, relative
/// with `/`. The launch parameters are `exe.args` ([`join_args`]).
/// `wineprefix` is `<install dir>/prefix` off Windows, `None` on Windows.
///
/// `meta.exe_user_pinned` marks the launch source `user`; the caller then
/// carries the user's executable over from the old row. `installed_at` is
/// now; `filesize_bytes` is nile's installed size; `revision` is the
/// installed version id.
pub fn amazon_row(
    inst: &AmazonInstalled,
    game: Option<&AmazonGame>,
    meta: &AmazonMeta,
    exe: Option<&FuelMain>,
    wineprefix: Option<&Path>,
) -> InstalledGame {
    let install_dir = Path::new(inst.path.trim());
    let dir = install_dir.to_string_lossy().into_owned();
    let rel = exe.and_then(|exe| relative_path(&exe.command));
    let executable = rel
        .as_ref()
        .map(|rel| install_dir.join(rel).to_string_lossy().into_owned())
        .unwrap_or_default();
    let working_dir = exe
        .filter(|exe| !exe.working_subdir.is_empty())
        .and_then(|exe| relative_path(&exe.working_subdir))
        .map(|sub| slash_path(&sub))
        .or_else(|| rel.as_ref().and_then(|rel| rel.parent()).map(slash_path))
        .unwrap_or_default();
    let launch_parameters = exe.map(|exe| join_args(&exe.args)).unwrap_or_default();
    let store_id = inst.id.trim().to_string();
    let title = game
        .map(|game| game.title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(&store_id)
        .to_string();
    let launch_source = if meta.exe_user_pinned {
        LAUNCH_SOURCE_USER
    } else {
        LAUNCH_SOURCE_SIDECAR
    };

    let mut row = InstalledGame {
        title,
        platform: StoreKind::Amazon.platform_name().to_string(),
        rom_id: None,
        extracted_dir: dir.clone(),
        native_game_dir: dir,
        native_executable_path: executable,
        native_launch_parameters: launch_parameters,
        native_working_dir: working_dir,
        native_launch_source: launch_source.to_string(),
        native_wineprefix: wineprefix
            .map(|prefix| prefix.to_string_lossy().into_owned())
            .unwrap_or_default(),
        platform_slug: "win".to_string(),
        revision: meta.version.clone(),
        filesize_bytes: i64::try_from(inst.size).unwrap_or(i64::MAX),
        installed_at: chrono::Utc::now().timestamp(),
        source: GameSource::Store(StoreKind::Amazon),
        store_id,
        store_meta: meta.to_json(),
        ..Default::default()
    };
    if let Some(game) = game {
        row.description = game.description.clone();
        row.companies = game.developer.trim().to_string();
        row.first_release_date = game.year.clone();
        row.genres = game.genres.join(", ");
        row.screenshot_urls = game.screenshots.join("\n");
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ID: &str = "amzn1.adg.product.11111111-2222-3333-4444-555555555555";

    fn installed() -> AmazonInstalled {
        AmazonInstalled {
            id: ID.into(),
            version: "fake-version-id".into(),
            path: "/games/Amazon Games/Fogline".into(),
            size: 7_000_000_000,
        }
    }

    fn fuel() -> FuelMain {
        FuelMain {
            command: "bin\\Fogline.exe".into(),
            args: vec!["-windowed".into()],
            working_subdir: String::new(),
        }
    }

    #[test]
    fn a_row_launches_fuels_executable_through_the_compat_tool() {
        let meta = AmazonMeta {
            product_id: ID.into(),
            version: "fake-version-id".into(),
            exe_user_pinned: false,
            ..Default::default()
        };
        let game = AmazonGame {
            product_id: ID.into(),
            entitlement_id: "ent-1".into(),
            title: "Fogline".into(),
            description: "Fog.".into(),
            developer: "Lowlight".into(),
            year: "2023".into(),
            genres: vec!["Adventure".into(), "Puzzle".into()],
            screenshots: vec!["https://m.media-amazon.com/images/I/a.jpg".into()],
            ..Default::default()
        };
        let prefix = Path::new("/games/Amazon Games/Fogline/prefix");
        let row = amazon_row(
            &installed(),
            Some(&game),
            &meta,
            Some(&fuel()),
            Some(prefix),
        );
        let dir = Path::new("/games/Amazon Games/Fogline");
        assert_eq!(row.title, "Fogline");
        assert_eq!(row.platform, "Amazon Games");
        assert_eq!(row.platform_slug, "win");
        assert_eq!(row.source, GameSource::Store(StoreKind::Amazon));
        assert_eq!(row.store_id, ID);
        assert_eq!(row.rom_id, None);
        assert_eq!(row.extracted_dir, dir.to_string_lossy());
        assert_eq!(row.native_game_dir, dir.to_string_lossy());
        assert_eq!(
            PathBuf::from(&row.native_executable_path),
            dir.join("bin").join("Fogline.exe")
        );
        assert_eq!(row.native_working_dir, "bin");
        assert_eq!(row.native_launch_parameters, "-windowed");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_SIDECAR);
        assert_eq!(row.native_wineprefix, prefix.to_string_lossy());
        assert_eq!(row.revision, "fake-version-id");
        assert_eq!(row.filesize_bytes, 7_000_000_000);
        assert_eq!(row.companies, "Lowlight");
        assert_eq!(row.description, "Fog.");
        assert_eq!(row.first_release_date, "2023");
        assert_eq!(row.genres, "Adventure, Puzzle");
        assert_eq!(
            row.screenshot_urls,
            "https://m.media-amazon.com/images/I/a.jpg"
        );
        assert_eq!(AmazonMeta::from_json(&row.store_meta), meta);
        assert!(row.is_native());
        assert!(row.is_windows());
        assert!(row.installed_at > 0);
    }

    #[test]
    fn the_working_subdir_override_wins_and_args_survive_the_split() {
        let exe = FuelMain {
            command: "Binaries/Win64/Game.exe".into(),
            args: vec![
                "-lang=en".into(),
                "two words".into(),
                "C:\\path\\x".into(),
                "it's".into(),
                String::new(),
            ],
            working_subdir: "Binaries\\Data".into(),
        };
        let row = amazon_row(&installed(), None, &AmazonMeta::default(), Some(&exe), None);
        assert_eq!(row.native_working_dir, "Binaries/Data");
        let split = crate::launch::template::split_template(&row.native_launch_parameters).unwrap();
        assert_eq!(split, exe.args);
    }

    #[test]
    fn without_the_owned_list_or_fuel_the_row_is_still_whole() {
        let meta = AmazonMeta {
            exe_user_pinned: true,
            ..Default::default()
        };
        let row = amazon_row(&installed(), None, &meta, None, None);
        assert_eq!(row.title, ID, "the product id stands in for the title");
        assert_eq!(row.native_executable_path, "");
        assert_eq!(row.native_working_dir, "");
        assert_eq!(row.native_launch_parameters, "");
        assert_eq!(row.native_launch_source, LAUNCH_SOURCE_USER);
        assert_eq!(row.native_wineprefix, "");
        assert_eq!(row.companies, "");
    }

    #[test]
    fn an_unsafe_executable_or_subdir_is_not_used() {
        for bad in ["..\\..\\evil.exe", "/bin/sh", "C:\\x.exe"] {
            let exe = FuelMain {
                command: bad.into(),
                ..fuel()
            };
            let row = amazon_row(&installed(), None, &AmazonMeta::default(), Some(&exe), None);
            assert_eq!(row.native_executable_path, "", "{bad:?}");
            assert_eq!(row.native_working_dir, "", "{bad:?}");
        }
        let exe = FuelMain {
            working_subdir: "..\\up".into(),
            ..fuel()
        };
        let row = amazon_row(&installed(), None, &AmazonMeta::default(), Some(&exe), None);
        assert_eq!(
            row.native_working_dir, "bin",
            "falls back to the exe's folder"
        );
        let top = FuelMain {
            command: "Game.exe".into(),
            ..fuel()
        };
        let row = amazon_row(&installed(), None, &AmazonMeta::default(), Some(&top), None);
        assert_eq!(row.native_working_dir, "", "the install folder itself");
    }

    #[test]
    fn fuel_json_reads_json5_comments_and_trailing_commas() {
        // The shape spike 3 fed nile 1.2.0, which accepted it.
        let text = "\u{feff}{\n  // a comment\n  \"SchemaVersion\": \"2\",\n  /* block\n comment */\n  \"Main\": {\n    \"Command\": \"bin\\\\Fogline.exe\",\n    \"Args\": [\"-windowed\", \"-url=http://x/y\", \"a,]\",],\n    \"WorkingSubdirOverride\": \"bin\",\n  },\n  \"PostInstall\": [],\n}\n";
        let main = parse_fuel(text).unwrap();
        assert_eq!(main.command, "bin\\Fogline.exe");
        assert_eq!(main.args, ["-windowed", "-url=http://x/y", "a,]"]);
        assert_eq!(main.working_subdir, "bin");

        let plain = parse_fuel(r#"{"Main": {"Command": "Game.exe", "Args": null}}"#).unwrap();
        assert_eq!(plain.command, "Game.exe");
        assert!(plain.args.is_empty());
        assert_eq!(plain.working_subdir, "");

        for bad in [
            "",
            "{",
            "{}",
            r#"{"Main": {}}"#,
            r#"{"Main": {"Command": "  "}}"#,
            r#"{"Main": null}"#,
        ] {
            assert_eq!(parse_fuel(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn post_install_steps_read_in_order_with_json5_and_odd_values() {
        // Hotline Miami's shape: OpenAL, then DirectX.
        let text = r#"{
  // a comment
  "Main": {"Command": "HotlineGL.exe"},
  "PostInstall": [
    {"Command": "oalinst.exe", "Args": ["/silent"],},
    {"Command": "DirectX\\DXSETUP.exe", "Args": ["/silent"], "ValidReturns": [0, 3010], "AlwaysRun": false},
    {"Command": "  "},
    {"Args": ["/x"]},
    {"Command": "v.exe", "Args": ["/q", 2, true, null, {}]},
  ],
}"#;
        assert_eq!(
            parse_post_install(text),
            vec![
                FuelStep {
                    id: String::new(),
                    command: "oalinst.exe".into(),
                    args: vec!["/silent".into()],
                    valid_returns: vec![],
                    root: String::new(),
                },
                FuelStep {
                    id: String::new(),
                    command: "DirectX\\DXSETUP.exe".into(),
                    args: vec!["/silent".into()],
                    valid_returns: vec![0, 3010],
                    root: String::new(),
                },
                FuelStep {
                    id: String::new(),
                    command: "v.exe".into(),
                    args: vec!["/q".into(), "2".into(), "true".into()],
                    valid_returns: vec![],
                    root: String::new(),
                },
            ]
        );
        for none in [
            "",
            "{",
            "{}",
            r#"{"PostInstall": []}"#,
            r#"{"PostInstall": null}"#,
            r#"{"PostInstall": {"Command": "a.exe"}}"#,
        ] {
            assert!(parse_post_install(none).is_empty(), "{none:?}");
        }
        let dir = tempfile::tempdir().unwrap();
        assert!(read_post_install(dir.path()).is_empty());
        std::fs::write(
            dir.path().join(FUEL_FILE),
            r#"{"PostInstall": [{"Command": "a.exe", "Args": ["/S",],},],}"#,
        )
        .unwrap();
        assert_eq!(read_post_install(dir.path())[0].args, ["/S"]);
    }

    #[test]
    fn read_fuel_reads_the_install_folders_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_fuel(dir.path()), None);
        std::fs::write(
            dir.path().join(FUEL_FILE),
            r#"{"Main": {"Command": "a.exe", "Args": ["-x",],},}"#,
        )
        .unwrap();
        let main = read_fuel(dir.path()).unwrap();
        assert_eq!(main.command, "a.exe");
        assert_eq!(main.args, ["-x"]);
    }
}
