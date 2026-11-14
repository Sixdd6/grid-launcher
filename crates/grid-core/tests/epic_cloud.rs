//! Epic cloud saves and launch tickets through a fake legendary (a shell
//! script): GRID's own direction rule over `list-saves`, forced
//! `sync-saves` runs, the save-folder copy before a download, the row's
//! `last_sync` / `save_path`, and `launch --json` with its offline retry.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::launch::launch_ticket_error;
use grid_core::stores::epic::meta::EpicMeta;
use grid_core::stores::epic::{EpicEndpoints, EpicService};
use grid_core::stores::{CloudState, GameSource, StoreError, StoreKind, SyncMode};

const FAKE: &str = "FAKE-STORE-TOKEN-not-real";
const APP: &str = "Starlight";

const SCRIPT: &str = r#"#!/bin/sh
cfg="$LEGENDARY_CONFIG_PATH"
echo "FAKE-STORE-TOKEN-not-real"
echo "[cli] INFO: FAKE-STORE-TOKEN-not-real" >&2
cmd="$1"; shift
case "$cmd" in
list-saves)
  app="$1"
  if [ -f "$cfg/signed-out" ]; then
    echo "Traceback (most recent call last):" >&2
    echo "ValueError: No saved credentials" >&2
    exit 1
  fi
  echo "Save games:"
  if [ -f "$cfg/cloud/$app.stamp" ]; then
    echo "- Starlight Courier (\"$app\")"
    echo " + $(cat "$cfg/cloud/$app.stamp").manifest"
  fi
  ;;
sync-saves)
  app="$1"; shift
  path=""; dir=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --save-path) path="$2"; shift ;;
      --force-upload) dir=up ;;
      --force-download) dir=down ;;
    esac
    shift
  done
  echo "$dir" >> "$cfg/syncs"
  mkdir -p "$cfg/cloud"
  if [ "$dir" = up ]; then
    rm -rf "$cfg/cloud/$app"; mkdir -p "$cfg/cloud/$app"
    cp -R "$path/." "$cfg/cloud/$app/"
    date -u +%Y.%m.%d-%H.%M.%S > "$cfg/cloud/$app.stamp"
  else
    rm -rf "$path"; mkdir -p "$path"
    cp -R "$cfg/cloud/$app/." "$path/"
  fi
  ;;
launch)
  app="$1"; shift
  offline=no
  for a in "$@"; do
    [ "$a" = --offline ] && offline=yes
    [ "$a" = --dry-run ] && exit 9
  done
  echo "$*" >> "$cfg/launches"
  if [ -f "$cfg/no-network" ] && [ "$offline" = no ]; then
    echo "[Core] ERROR: Logging in failed with ConnectionError(nope)" >&2
    echo "[cli] ERROR: Login failed, cannot continue!" >&2
    exit 1
  fi
  code="FAKE-STORE-TOKEN-not-real"
  [ "$offline" = yes ] && code=""
  printf '{"game_executable": "Binaries/Game.exe", "game_directory": "%s", "working_directory": "%s/Binaries", "game_parameters": ["-nolauncher", "-fromlegendary"], "user_parameters": [], "egl_parameters": ["-AUTH_LOGIN=unused", "-AUTH_PASSWORD=%s", "-AUTH_TYPE=exchangecode", "-epicapp=%s"], "launch_command": ["wine"], "environment": {}, "pre_launch_command": ""}\n' "$(cat "$cfg/game_dir")" "$(cat "$cfg/game_dir")" "$code" "$app"
  ;;
*)
  echo "[cli] ERROR: unknown command" >&2
  exit 1
  ;;
esac
"#;

struct Fixture {
    _root: tempfile::TempDir,
    data: PathBuf,
    game: PathBuf,
    svc: EpicService,
    registry: Registry,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let game = root.path().join("Epic Games").join("Starlight Courier");
        fs::create_dir_all(&game).unwrap();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let script = bin.join("legendary");
        fs::write(&script, SCRIPT).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let svc = EpicService::new(data.clone(), reqwest::Client::new(), EpicEndpoints::real())
            .with_tool_override(script);
        let tool_dir = svc.tool_dir();
        fs::create_dir_all(&tool_dir).unwrap();
        fs::write(tool_dir.join("game_dir"), game.to_string_lossy().as_bytes()).unwrap();
        let registry = Registry::open(&root.path().join("registry.db")).unwrap();
        Fixture {
            _root: root,
            data,
            game,
            svc,
            registry,
        }
    }

    fn tool(&self, name: &str) -> PathBuf {
        self.svc.tool_dir().join(name)
    }

    fn saves(&self) -> PathBuf {
        self.game.join("Saved")
    }

    fn row(&self) -> InstalledGame {
        self.registry
            .find_store(StoreKind::Epic, APP)
            .unwrap()
            .unwrap()
    }

    fn meta(&self) -> EpicMeta {
        EpicMeta::from_json(&self.row().store_meta)
    }

    fn set_meta(&self, edit: impl FnOnce(&mut EpicMeta)) {
        let mut row = self.row();
        let mut meta = EpicMeta::from_json(&row.store_meta);
        edit(&mut meta);
        row.store_meta = meta.to_json();
        self.registry.upsert_store(&row).unwrap();
    }

    fn insert_row(&self, cloud_saves: bool, can_run_offline: bool) {
        let dir = self.game.to_string_lossy().into_owned();
        self.registry
            .upsert_store(&InstalledGame {
                title: "Starlight Courier".into(),
                platform: StoreKind::Epic.platform_name().into(),
                platform_slug: "win".into(),
                source: GameSource::Store(StoreKind::Epic),
                store_id: APP.into(),
                extracted_dir: dir.clone(),
                native_game_dir: dir.clone(),
                native_executable_path: format!("{dir}/Binaries/Game.exe"),
                native_launch_parameters: "-nolauncher".into(),
                // A named tool, so the user's config is never read.
                native_compat_tool: "GE-Proton".into(),
                store_meta: EpicMeta {
                    app_name: APP.into(),
                    version: "1.0".into(),
                    platform: "Windows".into(),
                    cloud_saves,
                    cloud_save_folder: "{InstallDir}/Saved".into(),
                    can_run_offline,
                    ..Default::default()
                }
                .to_json(),
                installed_at: 1,
                ..Default::default()
            })
            .unwrap();
    }

    fn syncs(&self) -> Vec<String> {
        fs::read_to_string(self.tool("syncs"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn stamp(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .unwrap()
        .format("%Y.%m.%d-%H.%M.%S")
        .to_string()
}

fn set_mtime(path: &Path, at: i64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(at as u64))
        .unwrap();
}

#[tokio::test]
async fn cloud_saves_follow_grids_direction_rule() {
    let fx = Fixture::new();

    // Cloud saves off: nothing runs.
    fx.insert_row(false, true);
    assert_eq!(
        fx.svc.cloud_state(&fx.row()).await.unwrap(),
        CloudState::Off
    );
    assert!(fx.syncs().is_empty());

    // Nothing on either side.
    fx.insert_row(true, true);
    assert_eq!(
        fx.svc.cloud_state(&fx.row()).await.unwrap(),
        CloudState::None
    );

    // Local only: an automatic sync uploads (forced), and the row records
    // the time and the save folder.
    fs::create_dir_all(fx.saves()).unwrap();
    fs::write(fx.saves().join("slot1.sav"), "local-1").unwrap();
    let before = now();
    let state = fx
        .svc
        .cloud_sync(&fx.registry, &fx.row(), SyncMode::Auto)
        .await
        .unwrap();
    let CloudState::Synced { at } = state else {
        panic!("{state:?}");
    };
    assert!(at >= before);
    assert_eq!(fx.syncs(), ["up"]);
    assert_eq!(fx.meta().last_sync, at);
    assert_eq!(Path::new(&fx.meta().save_path), fx.saves());
    assert_eq!(
        fs::read_to_string(fx.tool("cloud").join(APP).join("slot1.sav")).unwrap(),
        "local-1"
    );
    // In sync now: the state says so and a sync runs nothing.
    assert_eq!(
        fx.svc.cloud_state(&fx.row()).await.unwrap(),
        CloudState::Synced { at }
    );
    fx.svc
        .cloud_sync(&fx.registry, &fx.row(), SyncMode::Auto)
        .await
        .unwrap();
    assert_eq!(fx.syncs(), ["up"]);

    // The cloud changed since the last sync, the local side didn't: a
    // forced download, after the local folder is copied aside.
    let t0 = now() - 1000;
    fx.set_meta(|m| m.last_sync = t0);
    set_mtime(&fx.saves().join("slot1.sav"), t0 - 100);
    fs::write(fx.tool("cloud").join(APP).join("slot1.sav"), "cloud-2").unwrap();
    fs::write(
        fx.tool("cloud").join(format!("{APP}.stamp")),
        stamp(now() - 500),
    )
    .unwrap();
    let state = fx
        .svc
        .cloud_sync(&fx.registry, &fx.row(), SyncMode::Auto)
        .await
        .unwrap();
    assert!(matches!(state, CloudState::Synced { .. }), "{state:?}");
    assert_eq!(fx.syncs(), ["up", "down"]);
    assert_eq!(
        fs::read_to_string(fx.saves().join("slot1.sav")).unwrap(),
        "cloud-2"
    );
    let backups = fx.data.join("stores/save-backups/epic").join(APP);
    let copies: Vec<PathBuf> = fs::read_dir(&backups)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(copies.len(), 1);
    assert_eq!(
        fs::read_to_string(copies[0].join("slot1.sav")).unwrap(),
        "local-1"
    );

    // Both sides changed: Auto stops at a conflict (no file count or size
    // for the cloud side), and the user's choice forces a direction.
    fx.set_meta(|m| m.last_sync = t0);
    set_mtime(&fx.saves().join("slot1.sav"), now() - 200);
    let state = fx
        .svc
        .cloud_sync(&fx.registry, &fx.row(), SyncMode::Auto)
        .await
        .unwrap();
    let CloudState::Conflict(conflict) = state else {
        panic!("{state:?}");
    };
    assert_eq!(conflict.local_files, 1);
    assert_eq!(conflict.cloud_files, None);
    assert_eq!(conflict.cloud_bytes, None);
    assert_eq!(fx.syncs(), ["up", "down"], "a conflict syncs nothing");
    assert!(matches!(
        fx.svc.cloud_state(&fx.row()).await.unwrap(),
        CloudState::Conflict(_)
    ));
    let state = fx
        .svc
        .cloud_sync(&fx.registry, &fx.row(), SyncMode::PreferUpload)
        .await
        .unwrap();
    assert!(matches!(state, CloudState::Synced { .. }), "{state:?}");
    assert_eq!(fx.syncs(), ["up", "down", "up"]);

    // legendary's folder is made private after every run.
    let mode = fs::metadata(fx.svc.tool_dir())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o077, 0, "{mode:o}");
}

#[tokio::test]
async fn a_signed_out_cloud_check_says_login_expired() {
    let fx = Fixture::new();
    fx.insert_row(true, true);
    fs::write(fx.tool("signed-out"), "").unwrap();
    let error = fx.svc.cloud_state(&fx.row()).await.unwrap_err();
    assert!(matches!(error, StoreError::LoginExpired), "{error:?}");
    assert!(!error.to_string().contains(FAKE));
}

#[tokio::test]
async fn the_launch_ticket_carries_the_code_as_redacted_arguments() {
    let fx = Fixture::new();
    fx.insert_row(true, true);

    let prep = fx.svc.launch_prep(&fx.row(), false).await.unwrap();
    assert_eq!(prep.executable, fx.game.join("Binaries/Game.exe"));
    assert_eq!(prep.working_dir, fx.game.join("Binaries"));
    // The row carries `-nolauncher`; legendary's copy is dropped.
    assert_eq!(prep.game_args, ["-fromlegendary"]);
    assert!(prep.secret_args.contains(&format!("-AUTH_PASSWORD={FAKE}")));
    assert!(!format!("{prep:?}").contains(FAKE));
    let launches = fs::read_to_string(fx.tool("launches")).unwrap();
    assert_eq!(launches.trim(), "--json --skip-version-check");

    // Epic unreachable: `launch_prep` never retries by itself (the app's
    // launch hook owns the one offline retry); asked for `--offline`, it
    // gets a ticket with no exchange code.
    fs::write(fx.tool("no-network"), "").unwrap();
    let error = fx.svc.launch_prep(&fx.row(), false).await.unwrap_err();
    assert!(matches!(error, StoreError::Network(_)), "{error:?}");
    let launches = fs::read_to_string(fx.tool("launches")).unwrap();
    assert_eq!(launches.lines().count(), 2, "{launches}");
    let prep = fx.svc.launch_prep(&fx.row(), true).await.unwrap();
    assert!(prep.secret_args.contains(&"-AUTH_PASSWORD=".to_string()));
    let launches = fs::read_to_string(fx.tool("launches")).unwrap();
    assert!(launches
        .trim_end()
        .ends_with("--json --skip-version-check --offline"));
    let words = launch_ticket_error(&error).to_string();
    assert!(
        words.starts_with("Couldn\u{2019}t get a launch ticket from Epic Games: "),
        "{words}"
    );
    assert!(!words.contains(FAKE));
}
