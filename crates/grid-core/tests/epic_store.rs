//! The Epic store against a fake legendary (a `sh` script that answers
//! like legendary 0.21.1 does): login with the code on stdin, the account
//! and an expired login, the owned list, install options, the install /
//! update / repair / DLC jobs with legendary's progress and the success
//! rule, DLC removal, uninstall and Disconnect.
//!
//! The fake prints `FAKE-STORE-TOKEN-not-real` on both streams in every
//! command and keeps it in `user.json`, so the tests can check that no
//! error GRID makes carries it. Its "installed games" are one JSON file
//! each under `<config>/inst/`; marker files under `<config>/` make it
//! fail the ways legendary does.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use grid_core::library::queue::{JobPhase, StoreOp, StoreProgressSink};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::meta::EpicMeta;
use grid_core::stores::epic::{list, EpicEndpoints, EpicInstallRequest, EpicJobCtx, EpicService};
use grid_core::stores::{GameSource, StoreError, StoreKind};
use secrecy::SecretString;

const TOKEN: &str = "FAKE-STORE-TOKEN-not-real";

const OWNED: &str = r#"[{"app_name": "Starlight", "app_title": "Starlight Courier",
  "asset_infos": {"Windows": {"app_name": "Starlight", "build_version": "1.0.3"}},
  "metadata": {"description": "Mail across a dying star.", "developer": "Lanternfish",
    "categories": [{"path": "games"}],
    "keyImages": [{"type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/s/tall.jpg"}],
    "customAttributes": {"CloudSaveFolder": {"type": "STRING", "value": "{AppData}/Starlight"},
                         "CanRunOffline": {"type": "STRING", "value": "true"}},
    "releaseInfo": [{"dateAdded": "2021-06-15T16:00:00.000Z"}]},
  "dlcs": [{"app_name": "NightRoute", "app_title": "Night Route", "asset_infos": {"Windows": {"build_version": "1"}},
            "metadata": {"mainGameItem": {"id": "c1"}}}]},
 {"app_name": "IronOrchard", "app_title": "Iron Orchard", "asset_infos": {},
  "metadata": {"customAttributes": {"ThirdPartyManagedApp": {"type": "STRING", "value": "Origin"}}}, "dlcs": []}]"#;

/// The fake legendary. `$LEGENDARY_CONFIG_PATH` is its state folder.
const FAKE: &str = r#"#!/bin/sh
C="$LEGENDARY_CONFIG_PATH"
mkdir -p "$C/inst"
echo "$*" >> "$C/../argv.log"
echo "[Core] DEBUG: token FAKE-STORE-TOKEN-not-real" >&2
inst_json() { # app title version dir is_dlc
  printf '{"app_name": "%s", "title": "%s", "version": "%s", "install_path": "%s", "install_size": 4096, "executable": "Binaries/Win64/Game.exe", "launch_parameters": "-nolauncher", "is_dlc": %s, "can_run_offline": true, "save_path": null}' "$1" "$2" "$3" "$4" "$5"
}
progress() {
  echo "[cli] INFO: Download size: 2.00 MiB (Compression savings: 1.0%)" >&2
  echo "[DLManager] INFO: = Progress: 50.00% (5/10), Running for 00:00:01, ETA: 00:00:01" >&2
  echo "[DLManager] INFO:  + Download	- 4.00 MiB/s (raw) / 8.00 MiB/s (decompressed)" >&2
  echo "[DLManager] INFO: = Progress: 100.00% (10/10), Running for 00:00:02, ETA: 00:00:00" >&2
}
case "$1" in
  auth)
    if [ "$2" = "--delete" ]; then rm -f "$C/user.json"; echo "[cli] INFO: User data deleted." >&2; exit 0; fi
    if [ -f "$C/no-api" ]; then
      echo "Traceback (most recent call last):" >&2
      echo "requests.exceptions.ProxyError: HTTPSConnectionPool(host='api.legendary.gl')" >&2
      exit 1
    fi
    printf 'Please enter the "authorizationCode" value from the JSON response: '
    IFS= read -r code
    if [ "$code" = "FAKE-STORE-TOKEN-not-real" ]; then
      printf '{"access_token": "%s", "displayName": "SamPlays"}' "$code" > "$C/user.json"
      chmod 644 "$C/user.json"
      echo "[cli] INFO: Successfully logged in as \"SamPlays\"" >&2
    else
      echo "[Core] ERROR: Logging in failed with InvalidCredentialsError()" >&2
      echo "[cli] ERROR: Login attempt failed, please see log for details." >&2
    fi
    exit 0;;
  status)
    if [ -f "$C/expired" ] && [ "$3" != "--offline" ]; then
      echo "[Core] ERROR: Stored credentials are no longer valid! Please login again." >&2
      echo "[cli] ERROR: Log in failed!" >&2
      exit 1
    fi
    if [ -f "$C/user.json" ]; then a=SamPlays; else a="<not logged in>"; fi
    printf '{"account": "%s", "games_available": 2, "games_installed": 0, "egl_sync_enabled": false, "config_directory": "%s"}\n' "$a" "$C"
    exit 0;;
  list)
    if [ ! -f "$C/user.json" ]; then echo "ValueError: No saved credentials" >&2; exit 1; fi
    echo "FAKE-STORE-TOKEN-not-real"
    cat "$C/../owned.json"; echo; exit 0;;
  list-installed)
    printf '['; first=1
    for f in "$C"/inst/*.json; do [ -f "$f" ] || continue; [ $first = 1 ] || printf ','; first=0; cat "$f"; done
    printf ']\n'; exit 0;;
  info)
    case "$2" in
      Starlight) printf '{"game": {"app_name": "Starlight", "cloud_save_folder": "{AppData}/Starlight", "owned_dlc": [{"app_name": "NightRoute", "title": "Night Route", "installable": [{}], "id": "d1"}]}, "install": null, "manifest": {"download_size": 3000, "disk_size": 9000, "launch_exe": "Binaries/Win64/Game.exe"}}\n';;
      NightRoute) printf '{"game": {"app_name": "NightRoute", "is_dlc": true, "owned_dlc": []}, "install": null, "manifest": {"download_size": 1000, "disk_size": 2000}}\n';;
      *) printf '{"game": null, "install": null, "manifest": null}\n';;
    esac
    exit 0;;
  install|update)
    app="$2"
    if [ -f "$C/fail-space" ]; then
      echo " ! Failure: Not enough available disk space! 1.00 GiB < 2.00 GiB"
      echo "[cli] CRITICAL: Installation cannot proceed, exiting." >&2
      exit 1
    fi
    if [ -f "$C/memory-once" ] && ! echo "$*" | grep -q -- "--max-shared-memory 5000"; then
      echo "Traceback (most recent call last):" >&2
      echo "MemoryError: Current shared memory cache is smaller than required: 1024 < 2048." >&2
      exit 1
    fi
    progress
    if [ -f "$C/fail-silent" ]; then
      echo "[cli] INFO: Installation failed after 2.00 seconds." >&2
      exit 0
    fi
    if [ "$1" = "update" ]; then
      [ -f "$C/inst/$app.json" ] || { echo "[cli] ERROR: Update requested for \"$app\", but app not installed!" >&2; exit 1; }
      dir=$(sed 's/.*"install_path": "\([^"]*\)".*/\1/' "$C/inst/$app.json")
      if grep -q '"is_dlc": true' "$C/inst/$app.json"; then dlc=true; else dlc=false; fi
      inst_json "$app" "$app" "$(cat "$C/../latest")" "$dir" $dlc > "$C/inst/$app.json"
      exit 0
    fi
    case "$app" in
      NightRoute)
        [ -f "$C/inst/Starlight.json" ] || { echo "[cli] CRITICAL: Base game \"Starlight\" is not installed!" >&2; exit 1; }
        dir=$(sed 's/.*"install_path": "\([^"]*\)".*/\1/' "$C/inst/Starlight.json")
        touch "$dir/NightRoute.pak"
        inst_json NightRoute "Night Route" 1 "$dir" true > "$C/inst/NightRoute.json";;
      *)
        base=""; folder=""
        while [ $# -gt 0 ]; do
          case "$1" in --base-path) base="$2"; shift;; --game-folder) folder="$2"; shift;; esac
          shift
        done
        mkdir -p "$base/$folder/Binaries/Win64"
        echo game > "$base/$folder/Binaries/Win64/Game.exe"
        inst_json "$app" "$app" 1.0.3 "$base/$folder" false > "$C/inst/$app.json";;
    esac
    exit 0;;
  repair)
    printf 'Verification progress: 1/4 (25.0%%) [1.0 MiB/s]\t\r'
    printf 'Verification progress: 4/4 (100.0%%) [2.0 MiB/s]\t\n'
    echo "[cli] ERROR: File does not match hash: \"a.pak\"" >&2
    echo "[cli] ERROR: File is missing: \"b.pak\"" >&2
    echo "[cli] ERROR: Verification failed, 1 file(s) corrupted, 1 file(s) are missing." >&2
    progress
    exit 0;;
  uninstall)
    app="$2"
    if [ ! -f "$C/inst/$app.json" ]; then echo "[cli] ERROR: Game $app not installed, cannot uninstall!" >&2; exit 0; fi
    dir=$(sed 's/.*"install_path": "\([^"]*\)".*/\1/' "$C/inst/$app.json")
    if [ "$app" = NightRoute ]; then rm -f "$dir/NightRoute.pak"; else rm -f "$dir/Binaries/Win64/Game.exe"; rm -f "$C/inst/NightRoute.json"; fi
    rm -f "$C/inst/$app.json"
    echo "[cli] INFO: Game has been uninstalled." >&2
    exit 0;;
esac
echo "[cli] ERROR: unknown command" >&2
exit 2
"#;

struct Fixture {
    root: tempfile::TempDir,
    svc: EpicService,
    registry: Arc<Registry>,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let script = root.path().join("legendary.sh");
        fs::write(&script, FAKE).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let svc = EpicService::new(data.clone(), reqwest::Client::new(), EpicEndpoints::real())
            .with_tool_override(script);
        // The fake's shared files sit beside its config folder.
        let stores = data.join("stores");
        fs::create_dir_all(&stores).unwrap();
        fs::write(stores.join("owned.json"), OWNED).unwrap();
        fs::write(stores.join("latest"), "1.0.4").unwrap();
        let registry = Arc::new(Registry::open(&root.path().join("registry.db")).unwrap());
        Fixture {
            root,
            svc,
            registry,
        }
    }

    fn marker(&self, name: &str) -> PathBuf {
        self.svc.tool_dir().join(name)
    }

    fn set(&self, name: &str) {
        fs::create_dir_all(self.svc.tool_dir()).unwrap();
        fs::write(self.marker(name), "").unwrap();
    }

    fn clear(&self, name: &str) {
        let _ = fs::remove_file(self.marker(name));
    }

    fn argv(&self) -> String {
        fs::read_to_string(self.root.path().join("data/stores/argv.log")).unwrap_or_default()
    }

    fn ctx(&self) -> EpicJobCtx {
        EpicJobCtx {
            registry: self.registry.clone(),
            library: self.root.path().join("library"),
            default_compat_tool: String::new(),
        }
    }

    fn games_dir(&self) -> PathBuf {
        self.root.path().join("library").join("Epic Games")
    }

    async fn sign_in(&self) {
        self.svc
            .complete_login(SecretString::from(TOKEN))
            .await
            .unwrap();
    }

    fn row(&self) -> InstalledGame {
        self.registry
            .find_store(StoreKind::Epic, "Starlight")
            .unwrap()
            .unwrap()
    }
}

#[derive(Default)]
struct Sink {
    phases: Vec<JobPhase>,
}

impl StoreProgressSink for Sink {
    fn progress(&mut self, _done: u64, _total: u64) {}
    fn phase(&mut self, phase: JobPhase) {
        self.phases.push(phase);
    }
}

type Reports = Vec<(u64, u64, f64)>;

/// Runs a job's download and finalize halves on the blocking pool.
async fn run_job(job: grid_core::library::queue::StoreJob) -> (Result<String, String>, Reports) {
    tokio::task::spawn_blocking(move || {
        let mut reports = Vec::new();
        let downloaded = job
            .runner
            .download(&AtomicBool::new(false), &mut |d, t, s| {
                reports.push((d, t, s))
            });
        if let Err(error) = downloaded {
            return (Err(error.to_string()), reports);
        }
        let result = job
            .runner
            .finalize(&mut Sink::default())
            .map_err(|e| e.to_string());
        (result, reports)
    })
    .await
    .unwrap()
}

fn install_request(f: &Fixture, dlc: &[&str]) -> EpicInstallRequest {
    EpicInstallRequest {
        app_name: "Starlight".into(),
        title: "Starlight Courier".into(),
        dlc: dlc.iter().map(|d| d.to_string()).collect(),
        install_dir: f.games_dir().join("Starlight Courier"),
        compat_tool: "proton-ge".into(),
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test]
async fn login_reads_the_code_from_stdin_and_makes_the_folder_private() {
    let f = Fixture::new();
    assert!(f.svc.tool_installed());
    assert_eq!(f.svc.account().await.unwrap(), None, "no user.json");

    let refused = f
        .svc
        .complete_login(SecretString::from("wrong-code"))
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("didn't accept"), "{refused}");

    let account = f
        .svc
        .complete_login(SecretString::from(TOKEN))
        .await
        .unwrap();
    assert_eq!(account.username, "SamPlays");
    let argv = f.argv();
    assert!(argv.contains("auth --disable-webview"), "{argv}");
    assert!(!argv.contains("--code"), "{argv}");
    assert!(!argv.contains(TOKEN), "the code never reaches argv: {argv}");

    let dir = f.svc.tool_dir();
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("user.json")), 0o600);
    assert_eq!(mode(&dir.join("config.ini")), 0o600);
    assert!(fs::read_to_string(dir.join("config.ini"))
        .unwrap()
        .contains("disable_update_notice = true"));

    assert_eq!(f.svc.account().await.unwrap().unwrap().username, "SamPlays");
    assert_eq!(
        f.svc.saved_account().await.unwrap().unwrap().username,
        "SamPlays"
    );
    f.set("expired");
    let expired = f.svc.account().await.unwrap_err();
    assert!(matches!(expired, StoreError::LoginExpired), "{expired}");
    assert!(
        f.svc.saved_account().await.unwrap().is_some(),
        "offline status doesn't ask Epic"
    );
}

#[tokio::test]
async fn a_login_without_legendarys_api_is_a_network_failure() {
    let f = Fixture::new();
    f.set("no-api");
    let error = f
        .svc
        .complete_login(SecretString::from(TOKEN))
        .await
        .unwrap_err();
    assert!(matches!(error, StoreError::Network(_)), "{error}");
    assert!(!error.to_string().contains(TOKEN));
    assert!(!f.svc.tool_dir().join("user.json").exists());
}

#[tokio::test]
async fn the_owned_list_and_install_options() {
    let f = Fixture::new();
    assert!(matches!(f.svc.owned().await, Err(StoreError::NotSignedIn)));
    f.sign_in().await;
    let games = f.svc.owned().await.unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].app_name, "Starlight");
    assert_eq!(games[0].year, "2021");
    assert_eq!(games[0].dlc[0].app_name, "NightRoute");
    assert_eq!(games[1].third_party.as_deref(), Some("the EA app"));
    assert_eq!(f.svc.game("Starlight").unwrap().title, "Starlight Courier");

    list::write_owned_cache(f.root.path(), &games, 7).unwrap();
    assert_eq!(list::read_owned_cache(f.root.path()).unwrap().games, games);
    let cache = fs::read_to_string(list::owned_cache_path(f.root.path())).unwrap();
    assert!(!cache.contains(TOKEN), "the cache holds no token");

    let options = f.svc.install_options("Starlight").await.unwrap();
    assert_eq!((options.download_bytes, options.disk_bytes), (3000, 9000));
    assert_eq!(options.dlc.len(), 1);
    assert_eq!(options.dlc[0].0.title, "Night Route");
    assert_eq!(options.dlc[0].1, 1000);

    let refused = f.svc.install_options("IronOrchard").await.unwrap_err();
    assert!(
        refused.to_string().contains("Needs the EA app"),
        "{refused}"
    );
    assert!(f.svc.install_options("Nope").await.is_err());
}

#[tokio::test]
async fn install_with_a_dlc_then_update_repair_and_uninstall() {
    let f = Fixture::new();
    f.sign_in().await;
    f.svc.owned().await.unwrap();

    let job = f
        .svc
        .install_job(install_request(&f, &["NightRoute"]), f.ctx());
    assert_eq!(job.source, StoreKind::Epic);
    assert_eq!(job.op, StoreOp::Install);
    let (result, reports) = run_job(job).await;
    assert_eq!(result.unwrap(), "");
    // Bytes from info's sizes: 3000 for the game, then 1000 for the DLC.
    assert_eq!(reports.first().map(|r| (r.0, r.1)), Some((0, 4000)));
    assert!(
        reports.iter().any(|r| r.0 == 1500 && r.1 == 4000),
        "{reports:?}"
    );
    assert!(
        reports.iter().any(|r| r.0 == 3500 && r.1 == 4000),
        "{reports:?}"
    );
    assert!(
        reports.iter().any(|r| r.2 == 4.0 * 1024.0 * 1024.0),
        "{reports:?}"
    );
    assert_eq!(reports.last().map(|r| (r.0, r.1)), Some((4000, 4000)));
    let argv = f.argv();
    assert!(
        argv.contains("install Starlight -y --skip-sdl --skip-dlcs --platform Windows --base-path"),
        "{argv}"
    );
    assert!(
        argv.contains("install NightRoute -y --skip-sdl --skip-dlcs"),
        "{argv}"
    );

    let row = f.row();
    let dir = f.games_dir().join("Starlight Courier");
    assert_eq!(row.source, GameSource::Store(StoreKind::Epic));
    assert_eq!(row.title, "Starlight Courier");
    assert_eq!(row.platform, "Epic Games");
    assert_eq!(row.native_game_dir, dir.to_string_lossy());
    assert_eq!(
        row.native_executable_path,
        dir.join("Binaries/Win64/Game.exe").to_string_lossy()
    );
    assert_eq!(row.native_launch_parameters, "-nolauncher");
    assert_eq!(row.native_compat_tool, "proton-ge");
    assert_eq!(row.native_wineprefix, dir.join("prefix").to_string_lossy());
    assert!(dir.join("prefix").is_dir());
    assert_eq!(row.companies, "Lanternfish");
    let meta = EpicMeta::from_json(&row.store_meta);
    assert_eq!(meta.version, "1.0.3");
    assert_eq!(meta.platform, "Windows");
    assert!(meta.cloud_saves);
    assert!(meta.can_run_offline);
    assert_eq!(meta.cloud_save_folder, "{AppData}/Starlight");
    assert_eq!(meta.dlc.len(), 1);
    assert_eq!(meta.dlc[0].title, "Night Route");

    // The owned list says 1.0.3; Epic now has 1.0.4.
    assert!(f
        .svc
        .updates(std::slice::from_ref(&row), &f.svc.owned().await.unwrap())
        .is_empty());
    let (result, reports) = run_job(f.svc.update_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(
        reports.first().map(|r| r.1),
        Some(20_000),
        "no sizes: a fraction"
    );
    assert!(f
        .argv()
        .contains("update Starlight -y --skip-sdl --skip-dlcs"));
    assert!(f
        .argv()
        .contains("update NightRoute -y --skip-sdl --skip-dlcs"));
    let row = f.row();
    assert_eq!(EpicMeta::from_json(&row.store_meta).version, "1.0.4");

    // An update that exits 0 but leaves the old build is a failure.
    fs::write(f.root.path().join("data/stores/latest"), "1.0.4").unwrap();
    f.set("fail-silent");
    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    let error = result.unwrap_err();
    assert!(error.contains("legendary stopped"), "{error}");
    assert!(!error.contains(TOKEN));
    f.clear("fail-silent");

    let repair = f.svc.repair_job(&row, f.ctx());
    assert_eq!(repair.runner.download_phase(), JobPhase::Verifying);
    let runner = repair.runner.clone();
    let (result, reports) = run_job(repair).await;
    // The repair's result is the Downloads row's detail, not a warning.
    assert_eq!(result.unwrap(), "");
    assert_eq!(
        runner.detail(),
        "4 files repaired",
        "both transfers' counts"
    );
    assert!(
        reports.iter().any(|r| r.0 == 2500 && r.1 == 20_000),
        "{reports:?}"
    );

    let row = f.row();
    f.svc.remove_dlc(&f.registry, &row, "NightRoute").unwrap();
    assert!(!dir.join("NightRoute.pak").exists());
    assert!(EpicMeta::from_json(&f.row().store_meta).dlc.is_empty());
    let error = f
        .svc
        .remove_dlc(&f.registry, &f.row(), "NightRoute")
        .unwrap_err();
    assert!(error.to_string().contains("isn't installed"));

    let (result, _) = run_job(f.svc.dlc_job(&f.row(), "NightRoute", f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(EpicMeta::from_json(&f.row().store_meta).dlc.len(), 1);

    f.svc.uninstall(&f.registry, "Starlight").unwrap();
    assert!(!dir.exists(), "the folder and its prefix go");
    assert!(f
        .registry
        .find_store(StoreKind::Epic, "Starlight")
        .unwrap()
        .is_none());
    assert!(f
        .argv()
        .contains("uninstall Starlight -y --skip-uninstaller"));
    assert!(f.svc.uninstall(&f.registry, "Starlight").is_err());
}

#[tokio::test]
async fn failed_installs_say_why_and_write_no_row() {
    let f = Fixture::new();
    f.sign_in().await;
    f.svc.owned().await.unwrap();

    f.set("fail-space");
    let (result, _) = run_job(f.svc.install_job(install_request(&f, &[]), f.ctx())).await;
    let error = result.unwrap_err();
    assert!(error.contains("there isn't enough disk space"), "{error}");
    f.clear("fail-space");

    // legendary exits 0 but never installed it.
    f.set("fail-silent");
    let (result, _) = run_job(f.svc.install_job(install_request(&f, &[]), f.ctx())).await;
    assert!(result.unwrap_err().contains("before it finished"));
    f.clear("fail-silent");
    assert!(f
        .registry
        .find_store(StoreKind::Epic, "Starlight")
        .unwrap()
        .is_none());

    // A third-party title never starts a transfer.
    let mut req = install_request(&f, &[]);
    req.app_name = "IronOrchard".into();
    let (result, _) = run_job(f.svc.install_job(req, f.ctx())).await;
    assert!(result.unwrap_err().contains("Needs the EA app"));

    // A MemoryError is retried once with more shared memory.
    f.set("memory-once");
    let (result, _) = run_job(f.svc.install_job(install_request(&f, &[]), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert!(f.argv().contains("--max-shared-memory 5000"));

    // Signed out: legendary's "No saved credentials" traceback.
    fs::remove_file(f.svc.tool_dir().join("user.json")).unwrap();
    assert!(matches!(f.svc.owned().await, Err(StoreError::NotSignedIn)));
}

#[tokio::test]
async fn uninstalling_what_legendary_forgot_still_removes_the_files() {
    let f = Fixture::new();
    f.sign_in().await;
    f.svc.owned().await.unwrap();
    let (result, _) = run_job(f.svc.install_job(install_request(&f, &[]), f.ctx())).await;
    result.unwrap();
    // A Disconnect deletes legendary's folder and with it installed.json.
    f.svc.disconnect().unwrap();
    assert!(!f.svc.tool_dir().join("user.json").exists());
    assert!(!f.svc.tool_dir().join("inst").exists());
    assert_eq!(f.svc.account().await.unwrap(), None);
    let dir = f.games_dir().join("Starlight Courier");
    assert!(dir.exists(), "installed games stay");
    f.svc.uninstall(&f.registry, "Starlight").unwrap();
    assert!(!dir.exists());
}
