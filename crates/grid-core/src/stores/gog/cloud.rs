//! GOG cloud saves through `gogdl save-sync`.
//!
//! gogdl's own conflict handling reports nothing structured and exits 0
//! either way, so GRID decides the direction itself: the newest local file
//! time and the newest cloudstorage time against `meta.last_sync`. Every
//! sync it runs forces a direction (`--force-upload` / `--force-download`).
//! A forced download deletes local files the cloud does not have, so GRID
//! copies the local saves aside first
//! (`<data_dir>/stores/save-backups/gog/<id>/<epoch>/`).
//!
//! Save locations come from the Galaxy remote config for the game's client
//! id (Windows builds only: GOG has no Linux cloud saves), with
//! `<?INSTALL?>`, `<?SAVED_GAMES?>`, `<?APPLICATION_DATA_LOCAL?>`,
//! `<?APPLICATION_DATA_LOCAL_LOW?>`, `<?APPLICATION_DATA_ROAMING?>` and
//! `<?DOCUMENTS?>` resolved on Windows, or inside the game's Wine prefix.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use secrecy::SecretString;

use super::api;
use super::auth;
use super::install::{game_dir, game_info};
use super::meta::{GogMeta, GogPlatform, SaveLocation};
use super::{tool_exit_error, CloudState, GogService, SaveConflict, SyncMode};
use crate::library::registry::{InstalledGame, Registry};
use crate::stores::paths;
use crate::stores::runner::run_tool;
use crate::stores::StoreError;

/// Clock slack, in seconds, between file times and the sync time.
const SLACK: i64 = 2;
/// How many save copies GRID keeps per game.
const KEEP_BACKUPS: usize = 5;

/// The folders the save-location variables stand for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveRoots {
    pub install: PathBuf,
    pub saved_games: Option<PathBuf>,
    pub local: Option<PathBuf>,
    pub local_low: Option<PathBuf>,
    pub roaming: Option<PathBuf>,
    pub documents: Option<PathBuf>,
}

impl SaveRoots {
    /// The roots on a Windows computer, from the user's environment.
    pub fn windows_host(install: &Path) -> SaveRoots {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let profile = var("USERPROFILE");
        SaveRoots {
            install: install.to_path_buf(),
            saved_games: profile.as_ref().map(|p| p.join("Saved Games")),
            local: var("LOCALAPPDATA")
                .or_else(|| profile.as_ref().map(|p| p.join("AppData").join("Local"))),
            local_low: profile.as_ref().map(|p| p.join("AppData").join("LocalLow")),
            roaming: var("APPDATA")
                .or_else(|| profile.as_ref().map(|p| p.join("AppData").join("Roaming"))),
            documents: profile.as_ref().map(|p| p.join("Documents")),
        }
    }

    /// The roots inside a Wine prefix: `drive_c/users/<user>` where the
    /// user is `steamuser` (Proton) when that folder exists, else `user`.
    pub fn wine_prefix(install: &Path, prefix: &Path, user: &str) -> SaveRoots {
        let users = prefix.join("drive_c").join("users");
        let name = if users.join("steamuser").is_dir() {
            "steamuser".to_string()
        } else {
            user.to_string()
        };
        let home = users.join(name);
        let documents = ["Documents", "My Documents"]
            .iter()
            .map(|d| home.join(d))
            .find(|d| d.is_dir())
            .unwrap_or_else(|| home.join("Documents"));
        SaveRoots {
            install: install.to_path_buf(),
            saved_games: Some(home.join("Saved Games")),
            local: Some(home.join("AppData").join("Local")),
            local_low: Some(home.join("AppData").join("LocalLow")),
            roaming: Some(home.join("AppData").join("Roaming")),
            documents: Some(documents),
        }
    }

    /// The roots for an installed row.
    pub fn for_row(row: &InstalledGame) -> SaveRoots {
        let install = game_dir(row);
        if cfg!(windows) {
            return SaveRoots::windows_host(&install);
        }
        let prefix = row.native_wineprefix.trim();
        let prefix = if prefix.is_empty() {
            install.join(super::install::PREFIX_DIR)
        } else {
            PathBuf::from(prefix)
        };
        let user = std::env::var("USER")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "steamuser".to_string());
        SaveRoots::wine_prefix(&install, &prefix, &user)
    }
}

/// A location template as a path. `None` when it names a variable GRID
/// does not know (or one with no folder here), or climbs out with `..`.
pub fn resolve_location(template: &str, roots: &SaveRoots) -> Option<PathBuf> {
    let template = template.trim();
    let stripped = template.strip_prefix("<?")?;
    let (name, rest) = stripped.split_once("?>")?;
    let base = match name {
        "INSTALL" => Some(roots.install.clone()),
        "SAVED_GAMES" => roots.saved_games.clone(),
        "APPLICATION_DATA_LOCAL" => roots.local.clone(),
        "APPLICATION_DATA_LOCAL_LOW" => roots.local_low.clone(),
        "APPLICATION_DATA_ROAMING" => roots.roaming.clone(),
        "DOCUMENTS" => roots.documents.clone(),
        _ => None,
    }?;
    let mut path = base;
    for part in rest.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            part if part.contains("<?") => return None,
            part => path.push(part),
        }
    }
    Some(path)
}

/// The local side of one sync: the newest file time, the file count and
/// bytes, over every location.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalSide {
    pub newest: i64,
    pub files: u32,
    pub bytes: u64,
}

/// Walks `dirs` (links not followed).
pub fn local_side(dirs: &[PathBuf]) -> LocalSide {
    fn walk(dir: &Path, side: &mut LocalSide) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(&entry.path(), side);
            } else if kind.is_file() {
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                side.files += 1;
                side.bytes += meta.len();
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                side.newest = side.newest.max(modified);
            }
        }
    }
    let mut side = LocalSide::default();
    for dir in dirs {
        walk(dir, &mut side);
    }
    side
}

/// The cloud side: the newest file time, the file count, and the bytes
/// (`None` when cloudstorage's listing leaves a file's size out).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloudSide {
    pub newest: i64,
    pub files: u32,
    pub bytes: Option<u64>,
}

impl Default for CloudSide {
    fn default() -> Self {
        CloudSide {
            newest: 0,
            files: 0,
            bytes: Some(0),
        }
    }
}

impl CloudSide {
    /// Adds one save location's cloud files.
    pub fn add(&mut self, location: &api::CloudLocation) {
        self.newest = self.newest.max(location.newest);
        self.files += location.files;
        self.bytes = self
            .bytes
            .zip(location.bytes)
            .map(|(sum, size)| sum.saturating_add(size));
    }
}

/// What a sync should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Nothing,
    Upload,
    Download,
    Conflict,
}

/// GRID's direction rule. Files only on one side go to the other; both
/// sides changed since `last_sync` (or never synced and both hold files)
/// is a conflict; one side changed goes that way.
pub fn decide(last_sync: i64, local: LocalSide, cloud: CloudSide) -> Decision {
    match (local.files > 0, cloud.files > 0) {
        (false, false) => Decision::Nothing,
        (true, false) => Decision::Upload,
        (false, true) => Decision::Download,
        (true, true) => {
            if last_sync <= 0 {
                return Decision::Conflict;
            }
            let local_changed = local.newest > last_sync + SLACK;
            let cloud_changed = cloud.newest > last_sync + SLACK;
            match (local_changed, cloud_changed) {
                (false, false) => Decision::Nothing,
                (true, false) => Decision::Upload,
                (false, true) => Decision::Download,
                (true, true) => Decision::Conflict,
            }
        }
    }
}

/// The direction a [`SyncMode`] takes for a decision; `None` = do nothing
/// (in sync, or a conflict left for the user).
pub fn direction(mode: SyncMode, decision: Decision) -> Option<Decision> {
    match mode {
        SyncMode::PreferUpload => Some(Decision::Upload),
        SyncMode::PreferDownload => Some(Decision::Download),
        SyncMode::Auto => match decision {
            Decision::Upload | Decision::Download => Some(decision),
            Decision::Nothing | Decision::Conflict => None,
        },
    }
}

/// `save-sync <path> <id> --os windows --ts <last> --name <location>
/// --force-upload|--force-download`.
pub fn save_sync_args(
    path: &Path,
    id: &str,
    last_sync: i64,
    name: &str,
    upload: bool,
) -> Vec<String> {
    vec![
        "save-sync".to_string(),
        path.to_string_lossy().into_owned(),
        id.trim().to_string(),
        "--os".to_string(),
        "windows".to_string(),
        "--ts".to_string(),
        format!("{last_sync}.0"),
        "--name".to_string(),
        name.to_string(),
        if upload {
            "--force-upload"
        } else {
            "--force-download"
        }
        .to_string(),
    ]
}

/// The timestamp `save-sync` printed (`1700000000.123`), as epoch seconds.
pub fn parse_sync_timestamp(stdout: &str) -> Option<i64> {
    stdout
        .split_whitespace()
        .rev()
        .find_map(|word| word.parse::<f64>().ok())
        .filter(|t| t.is_finite() && *t > 0.0)
        .map(|t| t as i64)
}

/// Everything a state check or a sync needs about one game.
struct Context {
    meta: GogMeta,
    locations: Vec<(SaveLocation, PathBuf)>,
    local: LocalSide,
    cloud: CloudSide,
}

/// The game's client id: its `goggame-<id>.info`, else gogdl's manifest.
fn client_id(svc: &GogService, row: &InstalledGame) -> Option<String> {
    let dir = game_dir(row);
    if let Some(info) = game_info(&dir, &row.store_id, GogPlatform::Windows, &row.title) {
        if !info.client_id.trim().is_empty() {
            return Some(info.client_id.trim().to_string());
        }
    }
    let bytes = fs::read(svc.manifests_dir().join(row.store_id.trim())).ok()?;
    api::parse_build_meta(&bytes).map(|(id, _)| id)
}

/// The game client's secret: gogdl's manifest, else the default Windows
/// build's meta.
async fn client_secret(
    svc: &GogService,
    row: &InstalledGame,
    client_id: &str,
    bearer: &SecretString,
) -> Result<SecretString, StoreError> {
    if let Ok(bytes) = fs::read(svc.manifests_dir().join(row.store_id.trim())) {
        if let Some((id, secret)) = api::parse_build_meta(&bytes) {
            if id == client_id {
                return Ok(secret);
            }
        }
    }
    match api::game_client(svc.http(), svc.endpoints(), bearer, &row.store_id).await? {
        Some((_, secret)) => Ok(secret),
        None => Err(StoreError::Refused(
            "GOG has no cloud saves for this game.".to_string(),
        )),
    }
}

/// Gathers both sides; `Ok(None)` when cloud saves are off for the game.
async fn context(svc: &GogService, row: &InstalledGame) -> Result<Option<Context>, StoreError> {
    let meta = GogMeta::from_json(&row.store_meta);
    if !meta.cloud_saves || meta.platform != GogPlatform::Windows {
        return Ok(None);
    }
    let Some(client) = client_id(svc, row) else {
        return Ok(None);
    };
    let mut locations = meta.save_locations.clone();
    if locations.is_empty() {
        locations = api::save_locations(svc.http(), svc.endpoints(), &client).await?;
    }
    if locations.is_empty() {
        return Ok(None);
    }
    let roots = SaveRoots::for_row(row);
    let resolved: Vec<(SaveLocation, PathBuf)> = locations
        .into_iter()
        .filter_map(|loc| resolve_location(&loc.path, &roots).map(|path| (loc, path)))
        .collect();
    if resolved.is_empty() {
        return Ok(None);
    }
    let dirs: Vec<PathBuf> = resolved.iter().map(|(_, p)| p.clone()).collect();
    let local = local_side(&dirs);

    let token = svc.bearer().await?;
    let secret = client_secret(svc, row, &client, &token.access).await?;
    let game = svc.game_bearer(&client, &secret).await?;
    let user_id = if game.user_id.is_empty() {
        token.user_id.clone()
    } else {
        game.user_id.clone()
    };
    let files =
        api::cloud_files(svc.http(), svc.endpoints(), &game.access, &user_id, &client).await?;
    let by_location = api::cloud_by_location(&files);
    let mut cloud = CloudSide::default();
    for (loc, _) in &resolved {
        if let Some(location) = by_location.get(&loc.name) {
            cloud.add(location);
        }
    }
    Ok(Some(Context {
        meta,
        locations: resolved,
        local,
        cloud,
    }))
}

fn state_for(ctx: &Context, decision: Decision) -> CloudState {
    match decision {
        Decision::Conflict => CloudState::Conflict(SaveConflict {
            local_at: ctx.local.newest,
            local_files: ctx.local.files,
            local_bytes: ctx.local.bytes,
            cloud_at: ctx.cloud.newest,
            cloud_files: Some(ctx.cloud.files),
            cloud_bytes: ctx.cloud.bytes,
        }),
        _ if ctx.meta.last_sync > 0 => CloudState::Synced {
            at: ctx.meta.last_sync,
        },
        _ => CloudState::None,
    }
}

/// The game's cloud save state, without syncing.
pub async fn cloud_state(svc: &GogService, row: &InstalledGame) -> Result<CloudState, StoreError> {
    let Some(ctx) = context(svc, row).await? else {
        return Ok(CloudState::Off);
    };
    let decision = decide(ctx.meta.last_sync, ctx.local, ctx.cloud);
    Ok(state_for(&ctx, decision))
}

/// Syncs the game's saves in the direction `mode` gives, then records the
/// sync time and locations in the row. A conflict under
/// [`SyncMode::Auto`] syncs nothing and comes back as
/// [`CloudState::Conflict`].
pub async fn cloud_sync(
    svc: &GogService,
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
    if !upload && ctx.local.files > 0 {
        backup_local(svc.data_dir(), &row.store_id, &ctx.locations)?;
    }
    let mut synced_at = 0i64;
    for (loc, path) in &ctx.locations {
        if upload && !path.is_dir() {
            continue;
        }
        let cmd = svc.gogdl(save_sync_args(
            path,
            &row.store_id,
            ctx.meta.last_sync,
            &loc.name,
            upload,
        ))?;
        let exit = tokio::task::spawn_blocking(move || {
            run_tool(&cmd, &AtomicBool::new(false), &mut |_| {})
        })
        .await
        .map_err(|_| tool_exit_error(None))??;
        if !exit.success() {
            return Err(tool_exit_error(exit.code));
        }
        if let Some(at) = auth::read_stdout(&exit.stdout, parse_sync_timestamp) {
            synced_at = synced_at.max(at);
        }
    }
    if synced_at == 0 {
        synced_at = chrono::Utc::now().timestamp();
    }
    let mut meta = ctx.meta.clone();
    meta.last_sync = synced_at;
    meta.save_locations = ctx.locations.iter().map(|(loc, _)| loc.clone()).collect();
    let mut updated = row.clone();
    updated.store_meta = meta.to_json();
    registry.upsert_store(&updated)?;
    Ok(CloudState::Synced { at: synced_at })
}

/// Copies every location's files to
/// `<data_dir>/stores/save-backups/gog/<id>/<epoch>/<location>/` and keeps
/// the newest [`KEEP_BACKUPS`] copies.
pub fn backup_local(
    data_dir: &Path,
    store_id: &str,
    locations: &[(SaveLocation, PathBuf)],
) -> Result<PathBuf, StoreError> {
    let game_root = paths::save_backups_dir(data_dir)
        .join("gog")
        .join(sanitize(store_id));
    let dest = game_root.join(chrono::Utc::now().timestamp_millis().to_string());
    let fail = |e: std::io::Error| {
        StoreError::Refused(format!(
            "Couldn't copy the local saves aside ({}), so GRID didn't download the cloud saves.",
            e.kind()
        ))
    };
    for (loc, path) in locations {
        if path.is_dir() {
            copy_tree(path, &dest.join(sanitize(&loc.name))).map_err(fail)?;
        }
    }
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

    fn local(newest: i64, files: u32) -> LocalSide {
        LocalSide {
            newest,
            files,
            bytes: u64::from(files) * 10,
        }
    }

    fn cloud(newest: i64, files: u32) -> CloudSide {
        CloudSide {
            newest,
            files,
            bytes: Some(u64::from(files) * 10),
        }
    }

    #[test]
    fn the_cloud_side_sums_its_locations() {
        let mut side = CloudSide::default();
        assert_eq!(side.bytes, Some(0));
        side.add(&api::CloudLocation {
            newest: 50,
            files: 2,
            bytes: Some(2_000),
        });
        side.add(&api::CloudLocation {
            newest: 40,
            files: 1,
            bytes: Some(100),
        });
        assert_eq!((side.newest, side.files, side.bytes), (50, 3, Some(2_100)));
        // One location without sizes makes the total unknown; the count stays.
        side.add(&api::CloudLocation {
            newest: 60,
            files: 1,
            bytes: None,
        });
        assert_eq!((side.newest, side.files, side.bytes), (60, 4, None));
    }

    #[test]
    fn the_direction_rule() {
        assert_eq!(decide(0, local(0, 0), cloud(0, 0)), Decision::Nothing);
        assert_eq!(decide(0, local(5, 1), cloud(0, 0)), Decision::Upload);
        assert_eq!(decide(0, local(0, 0), cloud(5, 1)), Decision::Download);
        assert_eq!(decide(0, local(5, 1), cloud(5, 1)), Decision::Conflict);
        assert_eq!(
            decide(100, local(90, 1), cloud(101, 1)),
            Decision::Nothing,
            "slack"
        );
        assert_eq!(decide(100, local(200, 1), cloud(90, 1)), Decision::Upload);
        assert_eq!(decide(100, local(90, 1), cloud(200, 1)), Decision::Download);
        assert_eq!(
            decide(100, local(200, 1), cloud(200, 1)),
            Decision::Conflict
        );
    }

    #[test]
    fn modes_force_a_direction() {
        assert_eq!(direction(SyncMode::Auto, Decision::Conflict), None);
        assert_eq!(direction(SyncMode::Auto, Decision::Nothing), None);
        assert_eq!(
            direction(SyncMode::Auto, Decision::Upload),
            Some(Decision::Upload)
        );
        assert_eq!(
            direction(SyncMode::PreferDownload, Decision::Conflict),
            Some(Decision::Download)
        );
        assert_eq!(
            direction(SyncMode::PreferUpload, Decision::Nothing),
            Some(Decision::Upload)
        );
    }

    #[test]
    fn locations_resolve_inside_the_prefix() {
        let roots = SaveRoots::wine_prefix(Path::new("/g/W"), Path::new("/g/W/prefix"), "sam");
        assert_eq!(
            resolve_location("<?SAVED_GAMES?>\\CD Projekt\\saves", &roots).unwrap(),
            Path::new("/g/W/prefix/drive_c/users/sam/Saved Games/CD Projekt/saves")
        );
        assert_eq!(
            resolve_location("<?INSTALL?>/save", &roots).unwrap(),
            Path::new("/g/W/save")
        );
        assert_eq!(
            resolve_location("<?APPLICATION_DATA_LOCAL_LOW?>/x", &roots).unwrap(),
            Path::new("/g/W/prefix/drive_c/users/sam/AppData/LocalLow/x")
        );
        assert!(resolve_location("<?INSTALL?>/../escape", &roots).is_none());
        assert!(resolve_location("<?APPLICATION_SUPPORT?>/x", &roots).is_none());
        assert!(resolve_location("/abs/path", &roots).is_none());
    }

    #[test]
    fn proton_prefixes_use_steamuser() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/users/steamuser")).unwrap();
        let roots = SaveRoots::wine_prefix(dir.path(), &prefix, "sam");
        assert!(roots
            .documents
            .unwrap()
            .starts_with(prefix.join("drive_c/users/steamuser")));
    }

    #[test]
    fn argv_and_timestamp() {
        let args = save_sync_args(Path::new("/s"), "123", 1700, "saves", false);
        assert_eq!(
            args,
            [
                "save-sync",
                "/s",
                "123",
                "--os",
                "windows",
                "--ts",
                "1700.0",
                "--name",
                "saves",
                "--force-download"
            ]
        );
        assert_eq!(parse_sync_timestamp("1700000000.25"), Some(1_700_000_000));
        assert_eq!(parse_sync_timestamp(""), None);
        assert_eq!(parse_sync_timestamp("nope"), None);
    }

    #[test]
    fn local_saves_are_copied_aside_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves");
        fs::create_dir_all(saves.join("sub")).unwrap();
        fs::write(saves.join("sub").join("a.sav"), "1").unwrap();
        let side = local_side(std::slice::from_ref(&saves));
        assert_eq!((side.files, side.bytes), (1, 1));
        assert!(side.newest > 0);
        let locations = vec![(
            SaveLocation {
                name: "saves".into(),
                path: "<?INSTALL?>/saves".into(),
            },
            saves.clone(),
        )];
        let root = paths::save_backups_dir(dir.path()).join("gog").join("123");
        for stamp in 1..=7 {
            fs::create_dir_all(root.join(stamp.to_string())).unwrap();
        }
        let copy = backup_local(dir.path(), "123", &locations).unwrap();
        assert_eq!(
            fs::read_to_string(copy.join("saves/sub/a.sav")).unwrap(),
            "1"
        );
        let left = fs::read_dir(&root).unwrap().count();
        assert_eq!(left, KEEP_BACKUPS);
        assert!(copy.exists(), "the newest copy is kept");
    }
}
