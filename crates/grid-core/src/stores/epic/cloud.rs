//! Epic cloud saves through legendary's `list-saves` and `sync-saves`.
//!
//! legendary decides a sync's direction from file times alone (the newer
//! side wins), so GRID decides it itself, as for GOG
//! (`stores::gog::cloud::decide`): the newest cloud save (the manifest
//! name `list-saves` prints is its time) and the newest local file against
//! `meta.last_sync`. Every sync GRID runs forces a direction
//! (`--force-upload` / `--force-download`), or stops at a conflict.
//!
//! A legendary download cleans the save folder first, so GRID copies it to
//! `<data_dir>/stores/save-backups/epic/<app>/<epoch-ms>/` before every
//! forced download (the newest five copies are kept).
//!
//! The save folder is resolved by GRID ([`resolve_save_path`]) from Epic's
//! `CloudSaveFolder` template, stored in `meta.save_path`, and always given
//! to legendary as `--save-path`.

use std::fs;
use std::path::{Component, Path, PathBuf};

use super::launch::{legendary_command, run, tool_error};
use super::meta::EpicMeta;
use super::EpicService;
use crate::cloud::native::{compat_kind, prefix_user, CompatKind, STEAM_USER};
use crate::library::registry::{InstalledGame, Registry};
use crate::stores::gog::cloud::{decide, direction, local_side, CloudSide, Decision, LocalSide};
use crate::stores::paths;
use crate::stores::runner::ToolSignal;
use crate::stores::{CloudState, SaveConflict, StoreError, SyncMode};

/// How many save copies GRID keeps per game.
pub const KEEP_BACKUPS: usize = 5;

/// The folder an Epic game's Wine prefix gets when the row names none
/// (`epic_row`, Phase 1 integration item 9).
const PREFIX_DIR: &str = "prefix";

/// Where the user folders of a save template are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveHost {
    /// A Windows computer: its known folders.
    Windows {
        /// `%USERPROFILE%`.
        profile: PathBuf,
        /// `%LOCALAPPDATA%`.
        local_appdata: PathBuf,
        /// `%APPDATA%`.
        roaming: PathBuf,
        /// The Documents folder (Shell-resolved when it is redirected).
        documents: PathBuf,
    },
    /// Anywhere else: the game's Wine prefix, `drive_c/users/<user>`.
    Wine,
}

impl SaveHost {
    /// This computer's host, from the user's environment on Windows.
    pub fn this_computer() -> SaveHost {
        if !cfg!(windows) {
            return SaveHost::Wine;
        }
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let profile = var("USERPROFILE").unwrap_or_default();
        SaveHost::Windows {
            local_appdata: var("LOCALAPPDATA")
                .unwrap_or_else(|| profile.join("AppData").join("Local")),
            roaming: var("APPDATA").unwrap_or_else(|| profile.join("AppData").join("Roaming")),
            documents: crate::platform::windows_documents_dir()
                .unwrap_or_else(|| profile.join("Documents")),
            profile,
        }
    }
}

/// The save folder Epic's `CloudSaveFolder` template names, or `None`
/// when GRID can't place it (yet).
///
/// - Tokens match without case, as whole path parts: `{InstallDir}` (the
///   game's folder), `{EpicID}` (the account id; `None` while unknown),
///   `{AppData}` (Local AppData), `{UserDir}` (Documents),
///   `{UserSavedGames}`, `{UserProfile}`, and the legacy `{LocalLow}` and
///   `{Roaming}`. Any other `{…}` gives `None`. A template must start with
///   a folder token.
/// - Either separator. `..` climbs (Epic writes `{AppData}/../LocalLow/…`)
///   but never out of the user folder (or the game folder for
///   `{InstallDir}`).
/// - [`SaveHost::Wine`]: the user folder is `prefix/drive_c/users/<user>`
///   with `prefix_user`; a user token gives `None` without a prefix or a
///   user (a prefix not made yet, whose user GRID doesn't know). Existing
///   parts are then matched without case ([`case_insensitive_walk`]).
pub fn resolve_save_path(
    folder: &str,
    install_dir: &Path,
    epic_id: &str,
    prefix: Option<&Path>,
    host: &SaveHost,
    prefix_user: Option<&str>,
) -> Option<PathBuf> {
    let folder = folder.trim().replace('\\', "/");
    let mut parts = folder.split('/').filter(|p| !p.is_empty() && *p != ".");
    let first = token(parts.next()?)?;

    let user_root = || -> Option<(PathBuf, PathBuf)> {
        // (the user folder, the bound `..` may not leave)
        match host {
            SaveHost::Windows { profile, .. } => Some((profile.clone(), profile.clone())),
            SaveHost::Wine => {
                let user = prefix_user.map(str::trim).filter(|u| !u.is_empty())?;
                let home = prefix?.join("drive_c").join("users").join(user);
                Some((home.clone(), home))
            }
        }
    };
    let within = |base: PathBuf, bound: PathBuf| {
        let bound = if base.starts_with(&bound) {
            bound
        } else {
            base.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| base.clone())
        };
        (base, bound)
    };
    let (mut path, bound) = match first.as_str() {
        "installdir" => {
            let dir = install_dir.to_path_buf();
            if dir.as_os_str().is_empty() {
                return None;
            }
            (dir.clone(), dir)
        }
        "appdata" | "userdir" | "usersavedgames" | "userprofile" | "locallow" | "roaming" => {
            let (home, bound) = user_root()?;
            let base = match (host, first.as_str()) {
                (SaveHost::Windows { local_appdata, .. }, "appdata") => local_appdata.clone(),
                (SaveHost::Windows { roaming, .. }, "roaming") => roaming.clone(),
                (SaveHost::Windows { documents, .. }, "userdir") => documents.clone(),
                (_, "appdata") => home.join("AppData").join("Local"),
                (_, "roaming") => home.join("AppData").join("Roaming"),
                (_, "locallow") => home.join("AppData").join("LocalLow"),
                (_, "userdir") => home.join("Documents"),
                (_, "usersavedgames") => home.join("Saved Games"),
                _ => home,
            };
            within(base, bound)
        }
        _ => return None,
    };

    for part in parts {
        if part == ".." {
            if !path.pop() || !path.starts_with(&bound) {
                return None;
            }
            continue;
        }
        if part.starts_with('{') {
            match token(part)?.as_str() {
                "epicid" if !epic_id.trim().is_empty() => path.push(epic_id.trim()),
                _ => return None,
            }
            continue;
        }
        if Path::new(part)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        path.push(part);
    }
    if !path.starts_with(&bound) {
        return None;
    }
    Some(match host {
        SaveHost::Wine => case_insensitive_walk(&path),
        SaveHost::Windows { .. } => path,
    })
}

/// A `{Name}` part as its lowercase name; `None` for any other part.
fn token(part: &str) -> Option<String> {
    let name = part.strip_prefix('{')?.strip_suffix('}')?;
    (!name.is_empty()).then(|| name.to_ascii_lowercase())
}

/// `path` with each part that exists under another case spelled as it is
/// on the disk (legendary's `case_insensitive_path_search`). Parts past the
/// first that matches nothing are kept as they are.
pub fn case_insensitive_walk(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut searching = true;
    for part in path.components() {
        let Component::Normal(name) = part else {
            out.push(part.as_os_str());
            continue;
        };
        if searching && !out.join(name).exists() {
            let wanted = name.to_string_lossy().to_lowercase();
            let found = fs::read_dir(if out.as_os_str().is_empty() {
                Path::new(".")
            } else {
                out.as_path()
            })
            .ok()
            .and_then(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name())
                    .find(|n| n.to_string_lossy().to_lowercase() == wanted)
            });
            match found {
                Some(real) => {
                    out.push(real);
                    continue;
                }
                None => searching = false,
            }
        }
        out.push(name);
    }
    out
}

/// The prefix user for saves, `None` when GRID can't know it yet: an
/// existing prefix says it ([`prefix_user`]); a prefix not made yet gets
/// `steamuser` only when Proton (umu) will make it. Plain Wine's user is
/// left unknown there, so the pre-launch download is skipped and the
/// after-session sync covers it.
pub fn save_prefix_user(prefix: &Path, kind: CompatKind) -> Option<String> {
    if prefix.join("drive_c").join("users").is_dir() {
        return Some(prefix_user(prefix, kind).name()).filter(|u| !u.is_empty());
    }
    (kind == CompatKind::Proton).then(|| STEAM_USER.to_string())
}

/// The newest cloud save in `list-saves` output (its manifest name
/// `YYYY.MM.DD-HH.MM.SS.manifest`, UTC) and how many there are, for `app`
/// (other games' saves, under their own `- Title ("app")` heading, are
/// skipped).
pub fn parse_list_saves(stdout: &str, app: &str) -> (i64, u32) {
    let mut newest = 0i64;
    let mut count = 0u32;
    let mut ours = true;
    let heading_tail = format!("(\"{}\")", app.trim());
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(heading) = line.strip_prefix("- ") {
            ours = heading.ends_with(&heading_tail);
            continue;
        }
        let Some(name) = line.strip_prefix("+ ") else {
            continue;
        };
        if !ours {
            continue;
        }
        let Some(stamp) = name.trim().strip_suffix(".manifest") else {
            continue;
        };
        let Ok(at) = chrono::NaiveDateTime::parse_from_str(stamp, "%Y.%m.%d-%H.%M.%S") else {
            continue;
        };
        count += 1;
        newest = newest.max(at.and_utc().timestamp());
    }
    (newest, count)
}

/// `sync-saves <app> -y --save-path <path> --force-upload|--force-download`.
pub fn sync_saves_args(app: &str, save_path: &Path, upload: bool) -> Vec<String> {
    vec![
        "sync-saves".to_string(),
        app.trim().to_string(),
        "-y".to_string(),
        "--save-path".to_string(),
        save_path.to_string_lossy().into_owned(),
        if upload {
            "--force-upload"
        } else {
            "--force-download"
        }
        .to_string(),
    ]
}

/// The Epic account id (`{EpicID}`) from legendary's `user.json`; nothing
/// else in that file is read.
fn account_id(tool_dir: &Path) -> String {
    #[derive(serde::Deserialize)]
    struct User {
        #[serde(default)]
        account_id: String,
    }
    fs::read_to_string(tool_dir.join("user.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<User>(&text).ok())
        .map(|user| user.account_id)
        .unwrap_or_default()
}

/// The game's folder.
fn install_dir(row: &InstalledGame) -> PathBuf {
    let dir = row.native_game_dir.trim();
    PathBuf::from(if dir.is_empty() {
        row.extracted_dir.trim()
    } else {
        dir
    })
}

/// The row's Wine prefix: its own, else `<install>/prefix`.
fn row_prefix(row: &InstalledGame) -> PathBuf {
    let prefix = row.native_wineprefix.trim();
    if prefix.is_empty() {
        install_dir(row).join(PREFIX_DIR)
    } else {
        PathBuf::from(prefix)
    }
}

/// What runs the row, as the launcher picks it. The config's default is
/// read only when the row names no tool.
fn row_compat_kind(row: &InstalledGame) -> CompatKind {
    let default_tool = if row.native_compat_tool.trim().is_empty() && !cfg!(windows) {
        crate::config::Config::load(&crate::config::Config::default_path())
            .map(|config| config.default_compat_tool)
            .unwrap_or_default()
    } else {
        String::new()
    };
    compat_kind(
        &row.native_compat_tool,
        &default_tool,
        row.is_windows() || row.platform_slug.trim().is_empty(),
        cfg!(windows),
    )
}

/// The save folder for `row` on this computer: [`resolve_save_path`], else
/// the folder recorded at the last sync.
pub fn save_path_for(svc: &EpicService, row: &InstalledGame, meta: &EpicMeta) -> Option<PathBuf> {
    let host = SaveHost::this_computer();
    let prefix = row_prefix(row);
    let user = match host {
        SaveHost::Wine => save_prefix_user(&prefix, row_compat_kind(row)),
        SaveHost::Windows { .. } => None,
    };
    resolve_save_path(
        &meta.cloud_save_folder,
        &install_dir(row),
        &account_id(&svc.tool_dir()),
        Some(&prefix),
        &host,
        user.as_deref(),
    )
    .or_else(|| {
        let saved = meta.save_path.trim();
        (!saved.is_empty()).then(|| PathBuf::from(saved))
    })
}

/// Both sides of one game.
struct Context {
    meta: EpicMeta,
    save_path: Option<PathBuf>,
    local: LocalSide,
    cloud: CloudSide,
}

async fn context(svc: &EpicService, row: &InstalledGame) -> Result<Option<Context>, StoreError> {
    let meta = EpicMeta::from_json(&row.store_meta);
    if !meta.cloud_saves || meta.cloud_save_folder.trim().is_empty() {
        return Ok(None);
    }
    let save_path = save_path_for(svc, row, &meta);
    let local = save_path
        .as_ref()
        .map(|path| local_side(std::slice::from_ref(path)))
        .unwrap_or_default();
    let app = row.store_id.trim();
    let exit = run(legendary_command(
        svc,
        vec!["list-saves".to_string(), app.to_string()],
    )?)
    .await?;
    if !exit.success()
        || exit.has(&ToolSignal::OtherError)
        || exit.has(&ToolSignal::NotLoggedIn)
        || exit.has(&ToolSignal::LoginFailed)
        || exit.has(&ToolSignal::Network)
    {
        return Err(tool_error(&exit));
    }
    let (newest, files) = exit.read_stdout(|out| parse_list_saves(out, app));
    Ok(Some(Context {
        meta,
        save_path,
        local,
        cloud: CloudSide {
            newest,
            files,
            bytes: None,
        },
    }))
}

fn state_for(ctx: &Context, decision: Decision) -> CloudState {
    match decision {
        Decision::Conflict => CloudState::Conflict(SaveConflict {
            local_at: ctx.local.newest,
            local_files: ctx.local.files,
            local_bytes: ctx.local.bytes,
            cloud_at: ctx.cloud.newest,
            // `list-saves` names saves, not files or sizes.
            cloud_files: None,
            cloud_bytes: None,
        }),
        _ if ctx.meta.last_sync > 0 => CloudState::Synced {
            at: ctx.meta.last_sync,
        },
        _ => CloudState::None,
    }
}

/// [`EpicService::cloud_state`]: the game's cloud save state, without
/// syncing.
pub(crate) async fn cloud_state(
    svc: &EpicService,
    row: &InstalledGame,
) -> Result<CloudState, StoreError> {
    let Some(ctx) = context(svc, row).await? else {
        return Ok(CloudState::Off);
    };
    Ok(state_for(
        &ctx,
        decide(ctx.meta.last_sync, ctx.local, ctx.cloud),
    ))
}

/// [`EpicService::cloud_sync`]: syncs in the direction `mode` gives, then
/// records the sync time and the save folder in the row. A conflict under
/// [`SyncMode::Auto`] syncs nothing and comes back as
/// [`CloudState::Conflict`]. With no save folder yet (a Wine prefix not
/// made, whose user GRID doesn't know), an automatic download is skipped.
pub(crate) async fn cloud_sync(
    svc: &EpicService,
    registry: &Registry,
    row: &InstalledGame,
    mode: SyncMode,
) -> Result<CloudState, StoreError> {
    let Some(ctx) = context(svc, row).await? else {
        return Ok(CloudState::Off);
    };
    let decision = decide(ctx.meta.last_sync, ctx.local, ctx.cloud);
    let Some(direction) = direction(mode, decision) else {
        return Ok(state_for(&ctx, decision));
    };
    let upload = direction == Decision::Upload;
    let Some(save_path) = ctx.save_path.clone() else {
        if mode == SyncMode::Auto || upload {
            return Ok(state_for(&ctx, decision));
        }
        return Err(StoreError::Refused(
            "Start the game once so its Windows folder exists; then GRID can download its cloud saves."
                .to_string(),
        ));
    };
    if upload {
        if ctx.local.files == 0 {
            return Ok(state_for(&ctx, decision));
        }
    } else {
        if ctx.local.files > 0 {
            backup_saves(&svc.data_dir, &row.store_id, &save_path)?;
        }
        fs::create_dir_all(&save_path).map_err(|e| {
            StoreError::Refused(format!(
                "Couldn\u{2019}t make the save folder ({}).",
                e.kind()
            ))
        })?;
    }
    let exit = run(legendary_command(
        svc,
        sync_saves_args(&row.store_id, &save_path, upload),
    )?)
    .await?;
    if !exit.success()
        || exit.has(&ToolSignal::OtherError)
        || exit.has(&ToolSignal::NotLoggedIn)
        || exit.has(&ToolSignal::LoginFailed)
        || exit.has(&ToolSignal::Network)
        || exit.has(&ToolSignal::NotInstalled)
    {
        return Err(tool_error(&exit));
    }
    // After the run, so the save legendary just wrote or uploaded is not
    // newer than the sync.
    let synced_at = chrono::Utc::now()
        .timestamp()
        .max(local_side(std::slice::from_ref(&save_path)).newest);
    let mut meta = ctx.meta.clone();
    meta.last_sync = synced_at;
    meta.save_path = save_path.to_string_lossy().into_owned();
    let mut updated = row.clone();
    updated.store_meta = meta.to_json();
    registry.upsert_store(&updated)?;
    Ok(CloudState::Synced { at: synced_at })
}

/// Copies the save folder to
/// `<data_dir>/stores/save-backups/epic/<app>/<epoch-ms>/` and keeps the
/// newest [`KEEP_BACKUPS`] copies.
pub fn backup_saves(data_dir: &Path, app: &str, save_path: &Path) -> Result<PathBuf, StoreError> {
    let game_root = paths::save_backups_dir(data_dir)
        .join("epic")
        .join(sanitize(app));
    let dest = game_root.join(chrono::Utc::now().timestamp_millis().to_string());
    copy_tree(save_path, &dest).map_err(|e| {
        StoreError::Refused(format!(
            "Couldn\u{2019}t copy the local saves aside ({}), so GRID didn\u{2019}t download the cloud saves.",
            e.kind()
        ))
    })?;
    prune(&game_root, KEEP_BACKUPS);
    Ok(dest)
}

fn sanitize(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "_".to_string()
    } else {
        clean
    }
}

/// Copies files and folders (links are not followed).
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Deletes all but the `keep` newest copies (by folder name, an epoch).
fn prune(root: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut copies: Vec<(i64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let stamp = e.file_name().to_string_lossy().parse::<i64>().ok()?;
            Some((stamp, e.path()))
        })
        .collect();
    copies.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (_, path) in copies.into_iter().skip(keep) {
        let _ = fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> SaveHost {
        SaveHost::Windows {
            profile: PathBuf::from("C:/Users/sam"),
            local_appdata: PathBuf::from("C:/Users/sam/AppData/Local"),
            roaming: PathBuf::from("C:/Users/sam/AppData/Roaming"),
            documents: PathBuf::from("D:/Docs"),
        }
    }

    fn wine(folder: &str, user: Option<&str>) -> Option<PathBuf> {
        resolve_save_path(
            folder,
            Path::new("/nonexistent-grid/g/Game"),
            "acc123",
            Some(Path::new("/nonexistent-grid/g/Game/prefix")),
            &SaveHost::Wine,
            user,
        )
    }

    const HOME: &str = "/nonexistent-grid/g/Game/prefix/drive_c/users";

    #[test]
    fn every_token_resolves_under_umu() {
        let home = Path::new(HOME).join("steamuser");
        let user = Some("steamuser");
        assert_eq!(
            wine("{AppData}/Game/Saves", user).unwrap(),
            home.join("AppData/Local/Game/Saves")
        );
        assert_eq!(
            wine("{UserDir}\\My Games\\X", user).unwrap(),
            home.join("Documents/My Games/X")
        );
        assert_eq!(
            wine("{UserSavedGames}/X", user).unwrap(),
            home.join("Saved Games/X")
        );
        assert_eq!(wine("{UserProfile}/X", user).unwrap(), home.join("X"));
        assert_eq!(
            wine("{LocalLow}/Co/G", user).unwrap(),
            home.join("AppData/LocalLow/Co/G")
        );
        assert_eq!(
            wine("{Roaming}/Co/G", user).unwrap(),
            home.join("AppData/Roaming/Co/G")
        );
        assert_eq!(
            wine("{InstallDir}/Saved/{EpicID}", user).unwrap(),
            Path::new("/nonexistent-grid/g/Game/Saved/acc123")
        );
        // Tokens match without case, and Epic's climb out of Local works.
        assert_eq!(
            wine("{appdata}/../LocalLow/Co/G", user).unwrap(),
            home.join("AppData/LocalLow/Co/G")
        );
    }

    #[test]
    fn plain_wine_uses_the_host_user_and_needs_one() {
        assert_eq!(
            wine("{AppData}/G", Some("sam")).unwrap(),
            Path::new(HOME).join("sam/AppData/Local/G")
        );
        // A prefix whose user GRID doesn't know yet: user folders wait,
        // the game's own folder does not.
        assert!(wine("{AppData}/G", None).is_none());
        assert!(wine("{InstallDir}/Saves", None).is_some());
    }

    #[test]
    fn unsafe_or_unknown_templates_are_refused() {
        let user = Some("steamuser");
        assert!(wine("{AppData}/../../../../etc", user).is_none());
        assert!(wine("{InstallDir}/../Other", user).is_none());
        assert!(wine("{Unknown}/x", user).is_none());
        assert!(wine("plain/path", user).is_none());
        assert!(wine("", user).is_none());
        assert!(wine("{AppData}/{Unknown}", user).is_none());
        assert!(resolve_save_path(
            "{InstallDir}/{EpicID}",
            Path::new("/g"),
            "",
            None,
            &SaveHost::Wine,
            None
        )
        .is_none());
    }

    #[test]
    fn a_windows_host_uses_its_known_folders() {
        let at = |folder: &str| {
            resolve_save_path(folder, Path::new("C:/Games/G"), "acc", None, &win(), None)
        };
        assert_eq!(
            at("{AppData}/G").unwrap(),
            Path::new("C:/Users/sam/AppData/Local/G")
        );
        assert_eq!(at("{UserDir}/G").unwrap(), Path::new("D:/Docs/G"));
        assert_eq!(
            at("{UserSavedGames}/G").unwrap(),
            Path::new("C:/Users/sam/Saved Games/G")
        );
        assert_eq!(
            at("{Roaming}/G").unwrap(),
            Path::new("C:/Users/sam/AppData/Roaming/G")
        );
        assert_eq!(
            at("{LocalLow}/G").unwrap(),
            Path::new("C:/Users/sam/AppData/LocalLow/G")
        );
        assert_eq!(
            at("{AppData}/../LocalLow/G").unwrap(),
            Path::new("C:/Users/sam/AppData/LocalLow/G")
        );
        assert_eq!(
            at("{InstallDir}/S/{EpicID}").unwrap(),
            Path::new("C:/Games/G/S/acc")
        );
        // A redirected Documents folder may not be climbed out of.
        assert!(at("{UserDir}/../../x").is_none());
    }

    #[test]
    fn existing_parts_match_without_case() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let real = prefix.join("drive_c/users/steamuser/AppData/Local/MyCo");
        fs::create_dir_all(&real).unwrap();
        let path = resolve_save_path(
            "{AppData}/myco/Saves",
            dir.path(),
            "",
            Some(&prefix),
            &SaveHost::Wine,
            Some("steamuser"),
        )
        .unwrap();
        assert_eq!(path, real.join("Saves"));
        assert_eq!(
            case_insensitive_walk(&dir.path().join("PREFIX/Drive_C/none/X")),
            prefix.join("drive_c/none/X")
        );
    }

    #[test]
    fn the_prefix_user_rule() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        // Not made yet: only Proton's user is known.
        assert_eq!(
            save_prefix_user(&prefix, CompatKind::Proton).as_deref(),
            Some("steamuser")
        );
        assert_eq!(save_prefix_user(&prefix, CompatKind::Wine), None);
        // Made: the prefix says.
        fs::create_dir_all(prefix.join("drive_c/users/steamuser")).unwrap();
        assert_eq!(
            save_prefix_user(&prefix, CompatKind::Wine).as_deref(),
            Some("steamuser")
        );
    }

    #[test]
    fn list_saves_gives_the_newest_manifest() {
        let out = "FAKE-STORE-TOKEN-not-real\nSave games:\n- Other (\"Other\")\n + 2030.01.01-00.00.00.manifest\n\
                   - Starlight Courier (\"Starlight\")\n + 2026.11.01-10.00.00.manifest\n + 2026.11.02-12.30.15.manifest\n + junk\n";
        let (newest, count) = parse_list_saves(out, "Starlight");
        assert_eq!(count, 2);
        assert_eq!(
            newest,
            chrono::NaiveDate::from_ymd_opt(2026, 11, 2)
                .unwrap()
                .and_hms_opt(12, 30, 15)
                .unwrap()
                .and_utc()
                .timestamp()
        );
        assert_eq!(parse_list_saves("Save games:\n", "Starlight"), (0, 0));
    }

    #[test]
    fn sync_argv_forces_a_direction() {
        assert_eq!(
            sync_saves_args("Starlight", Path::new("/s"), true),
            [
                "sync-saves",
                "Starlight",
                "-y",
                "--save-path",
                "/s",
                "--force-upload"
            ]
        );
        assert_eq!(
            sync_saves_args("Starlight", Path::new("/s"), false)
                .last()
                .map(String::as_str),
            Some("--force-download")
        );
    }

    fn ctx(last_sync: i64, local: (i64, u32), cloud: (i64, u32)) -> Context {
        Context {
            meta: EpicMeta {
                last_sync,
                ..Default::default()
            },
            save_path: Some(PathBuf::from("/s")),
            local: LocalSide {
                newest: local.0,
                files: local.1,
                bytes: u64::from(local.1) * 10,
            },
            cloud: CloudSide {
                newest: cloud.0,
                files: cloud.1,
                bytes: None,
            },
        }
    }

    #[test]
    fn the_direction_and_state_rules() {
        // Each side alone goes to the other; both changed is a conflict.
        let c = ctx(100, (200, 2), (90, 1));
        assert_eq!(decide(100, c.local, c.cloud), Decision::Upload);
        let c = ctx(100, (90, 2), (200, 1));
        let d = decide(100, c.local, c.cloud);
        assert_eq!(d, Decision::Download);
        assert_eq!(direction(SyncMode::Auto, d), Some(Decision::Download));
        let c = ctx(100, (200, 2), (200, 1));
        let d = decide(100, c.local, c.cloud);
        assert_eq!(d, Decision::Conflict);
        assert_eq!(direction(SyncMode::Auto, d), None);
        assert_eq!(
            direction(SyncMode::PreferDownload, d),
            Some(Decision::Download)
        );
        match state_for(&c, d) {
            CloudState::Conflict(conflict) => {
                assert_eq!(conflict.local_files, 2);
                assert_eq!(conflict.local_bytes, 20);
                assert_eq!(conflict.cloud_at, 200);
                assert_eq!(conflict.cloud_files, None);
                assert_eq!(conflict.cloud_bytes, None);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            state_for(&ctx(100, (0, 0), (0, 0)), Decision::Nothing),
            CloudState::Synced { at: 100 }
        );
        assert_eq!(
            state_for(&ctx(0, (0, 0), (0, 0)), Decision::Nothing),
            CloudState::None
        );
    }

    #[test]
    fn saves_are_copied_aside_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves");
        fs::create_dir_all(saves.join("sub")).unwrap();
        fs::write(saves.join("sub/a.sav"), "1").unwrap();
        let root = paths::save_backups_dir(dir.path())
            .join("epic")
            .join("Starlight");
        for stamp in 1..=7 {
            fs::create_dir_all(root.join(stamp.to_string())).unwrap();
        }
        let copy = backup_saves(dir.path(), "Starlight", &saves).unwrap();
        assert_eq!(fs::read_to_string(copy.join("sub/a.sav")).unwrap(), "1");
        assert_eq!(fs::read_dir(&root).unwrap().count(), KEEP_BACKUPS);
        assert!(copy.exists());
    }
}
