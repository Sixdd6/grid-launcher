//! The Amazon store against a fake nile (a `sh` script that answers like
//! nile 1.2.0 does, exit codes included): login (the stale login cleared,
//! a refused code, Amazon unreachable), the account, the owned list and an
//! expired login, install / update / verify with nile's progress and the
//! success rule, the update list, the launch dry run, uninstall (also when
//! nile forgot the game) and Disconnect.
//!
//! The fake prints `FAKE-STORE-TOKEN-not-real` on both streams in every
//! command and keeps it in its token file, so the tests can check that no
//! error GRID makes carries it. Marker files under nile's folder make it
//! fail the ways nile does; like nile it exits 0 for most failures and
//! writes its files 0644.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grid_core::stores::offline::exec::{
    InstallerCommand, InstallerProcess, InstallerSpawner, Watch,
};
use grid_core::stores::redist::{HostProbe, RedistRunner, REDIST_CANCELLED};

use grid_core::library::queue::{JobPhase, StoreJob, StoreOp, StoreProgressSink};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::library::LibraryError;
use grid_core::stores::amazon::meta::AmazonMeta;
use grid_core::stores::amazon::{
    auth, install, launch, AmazonEndpoints, AmazonInstallRequest, AmazonJobCtx, AmazonService,
};
use grid_core::stores::{GameSource, StoreError, StoreKind};
use secrecy::SecretString;

const TOKEN: &str = "FAKE-STORE-TOKEN-not-real";
const FOGLINE: &str = "amzn1.adg.product.aaaa";

const LIBRARY: &str = r#"[
 {"id": "ent-fog", "fake_session": "FAKE-STORE-TOKEN-not-real",
  "product": {"id": "amzn1.adg.product.aaaa", "sku": "SKU-FOG", "asin": "B0FOG", "title": "Fogline",
   "productDetail": {"iconUrl": "https://m.media-amazon.com/images/I/fog.png",
    "details": {"shortDescription": "Fog and lines.", "developer": "Dune Owl", "genres": ["Adventure"],
     "releaseDate": "2021-03-04T00:00:00Z", "screenshots": ["https://m.media-amazon.com/images/I/s1.jpg"]}}}},
 {"id": "ent-kettle", "product": {"id": "amzn1.adg.product.bbbb", "asin": "B0KET", "title": "Kettle Drum"}}
]"#;

/// The fake nile. `$NILE_CONFIG_PATH/nile` is its state folder (nile
/// appends `/nile`); markers live there too.
const FAKE: &str = r#"#!/bin/sh
C="$NILE_CONFIG_PATH/nile"
mkdir -p "$C"
[ -f "$C/installed.json" ] || { echo '[]' > "$C/installed.json"; chmod 644 "$C/installed.json"; }
echo "$*" >> "$NILE_CONFIG_PATH/../argv.log"
echo "$TOKEN_LINE" >&2
echo "FAKE-STORE-TOKEN-not-real"
log() { printf '%s [%s]:\t %s\n' "$1" "$2" "$3" >&2; }
progress() {
  log INFO PROGRESS "= Progress: 50.00 1500/3000, Running for: 00:00:01, ETA: 00:00:01"
  log INFO PROGRESS " + Download	- 4.00 MiB/s"
  log INFO PROGRESS "= Progress: 100.00 3000/3000, Running for: 00:00:02, ETA: 00:00:00"
}
installed_path() { sed 's/.*"path": "\([^"]*\)".*/\1/' "$C/installed.json"; }
write_installed() { # id version dir
  printf '[{"id": "%s", "version": "%s", "path": "%s", "size": 3000}]' "$1" "$2" "$3" > "$C/installed.json"
  chmod 644 "$C/installed.json"
}
case "$1" in
  auth)
    case "$2" in
      --login)
        if [ -f "$C/current_user.json" ]; then log ERROR CLI "You are already logged in"; exit 0; fi
        printf '{"client_id": "c0ffee", "code_verifier": "FAKE-STORE-TOKEN-not-real", "serial": "5e71a1", "url": "https://amazon.com/ap/signin?openid.assoc_handle=amzn_sonic_games_launcher&openid.oa2.client_id=device:c0ffee"}\n'
        exit 0;;
      --status)
        if [ -f "$C/current_user.json" ]; then printf '{"Username": "sam.plays", "LoggedIn": true, "fake_session": "FAKE-STORE-TOKEN-not-real"}\n'
        else printf '{"Username": "<not logged in>", "LoggedIn": false}\n'; fi
        exit 0;;
      --logout)
        if [ -f "$C/logout-fail" ]; then
          echo "Traceback (most recent call last):" >&2
          echo "requests.exceptions.ConnectionError: x" >&2
          exit 1
        fi
        rm -f "$C/current_user.json" "$C"/*.enc
        log INFO AUTH_MANAGER "Successfully deregistered a device"
        exit 0;;
    esac;;
  register)
    if [ -f "$C/current_user.json" ]; then log ERROR CLI "You are already logged in"; exit 0; fi
    log INFO AUTH_MANAGER "Registerring a device. ID: c0ffee"
    if [ -f "$C/offline" ]; then
      echo "Traceback (most recent call last):" >&2
      echo "requests.exceptions.ProxyError: HTTPSConnectionPool(host='api.amazon.com', port=443)" >&2
      exit 1
    fi
    code=""; verifier=""
    while [ $# -gt 0 ]; do
      case "$1" in --code) code="$2"; shift;; --code-verifier) verifier="$2"; shift;; esac
      shift
    done
    if [ "$code" != "good-code" ] || [ "$verifier" != "FAKE-STORE-TOKEN-not-real" ]; then
      log ERROR AUTH_MANAGER "Failed to register a device"
      echo "b'{\"error\": \"FAKE-STORE-TOKEN-not-real\"}'"
      exit 0
    fi
    log INFO AUTH_MANAGER "Succesfully registered a device"
    echo "{'response': {'success': {'tokens': {'bearer': {'access_token': 'FAKE-STORE-TOKEN-not-real'}}}}}"
    printf '{"name": "sam.plays", "user_id": "amzn1.account.FAKE"}' > "$C/current_user.json"
    echo "FAKE-STORE-TOKEN-not-real" > "$C/0123abcd.enc"
    chmod 644 "$C/current_user.json" "$C/0123abcd.enc"
    cp "$NILE_CONFIG_PATH/../library.json" "$C/library.json"
    exit 0;;
  library)
    if [ ! -f "$C/current_user.json" ]; then log ERROR CLI "User not logged in"; exit 1; fi
    if [ -f "$C/expired" ]; then
      log ERROR AUTH_MANAGER "Failed to refresh the token <Response [400]>"
      log ERROR LIBRARY "There was an error syncing library"
      exit 0
    fi
    if [ -f "$C/offline" ]; then
      log ERROR AUTH_MANAGER "Failed to refresh the token HTTPSConnectionPool(host='api.amazon.com', port=443)"
      log ERROR LIBRARY "There was an error syncing library"
      exit 0
    fi
    cp "$NILE_CONFIG_PATH/../library.json" "$C/library.json"; chmod 644 "$C/library.json"
    log INFO LIBRARY "Successfully synced the library"
    exit 0;;
  install|update|verify)
    if [ "$2" = "--info" ]; then printf '{"download_size": 3000, "fake_session": "FAKE-STORE-TOKEN-not-real"}\n'; exit 0; fi
    if [ "$2" = "--path" ]; then dir="$3"; id="$4"; else id="$2"; dir=$(installed_path); fi
    log INFO CLI "Found: Fogline"
    if [ -f "$C/no-space" ]; then
      log ERROR DOWNLOAD "Not enough space available"; log INFO CLI "Download complete"; exit 0
    fi
    if [ "$1" = verify ]; then
      if [ ! -f "$C/manifests/$id.raw" ]; then log ERROR DOWNLOAD "Unable to load manifest"; log INFO CLI "Download complete"; exit 0; fi
      progress; log INFO CLI "Download complete"; exit 0
    fi
    if [ "$1" = update ] && [ -f "$C/up-to-date" ]; then
      log INFO DOWNLOAD "Game is up to date"; log INFO CLI "Download complete"; exit 0
    fi
    progress
    if [ -f "$C/silent-fail" ]; then log INFO CLI "Download complete"; exit 0; fi
    if [ -f "$C/slow" ]; then mkdir -p "$dir/bin"; sleep 20; fi
    if [ -f "$C/network-once" ]; then
      rm -f "$C/network-once"
      log ERROR AUTH_MANAGER "Failed to refresh the token HTTPSConnectionPool(host='api.amazon.com', port=443)"
    fi
    mkdir -p "$dir/bin" "$C/manifests"
    echo game > "$dir/bin/Fogline.exe"
    cat > "$dir/fuel.json" <<'FUEL'
{
  // Amazon's fuel.json is JSON5.
  "SchemaVersion": "2",
  "Main": {"Command": "bin\\Fogline.exe", "Args": ["-windowed"],},
  "PostInstall": [{"Command": "redist\\vc.exe", "Args": ["/quiet"]}],
}
FUEL
    mkdir -p "$dir/redist"; echo vc > "$dir/redist/vc.exe"; echo oal > "$dir/redist/oalinst.exe"
    if [ -f "$C/post-two" ]; then
      cat > "$dir/fuel.json" <<'FUEL'
{"Main": {"Command": "bin\\Fogline.exe", "Args": ["-windowed"]},
 "PostInstall": [{"Command": "redist\\vc.exe", "Args": ["/quiet"]}, {"Command": "redist\\oalinst.exe", "Args": ["/silent"]}]}
FUEL
    fi
    echo pb > "$C/manifests/$id.raw"
    if [ "$1" = update ]; then write_installed "$id" v2 "$dir"; else write_installed "$id" v1 "$dir"; fi
    log INFO CLI "Download complete"
    exit 0;;
  uninstall)
    id="$2"
    if ! grep -q "$id" "$C/installed.json"; then log ERROR UNINSTALL "Game isn't installed"; exit 0; fi
    if [ ! -f "$C/manifests/$id.raw" ]; then
      echo "Traceback (most recent call last):" >&2
      echo "AttributeError: 'NoneType' object has no attribute 'packages'" >&2
      exit 1
    fi
    dir=$(installed_path)
    rm -f "$dir/bin/Fogline.exe" "$dir/fuel.json"; rmdir "$dir/bin" 2>/dev/null
    echo '[]' > "$C/installed.json"; rm -f "$C/manifests/$id.raw"
    log INFO UNINSTALL "Game removed successfully"
    exit 0;;
  list-updates)
    if ! grep -q '"id"' "$C/installed.json"; then echo '[]'; log ERROR CLI "No games installed"; exit 0; fi
    if [ -f "$C/update" ]; then printf '["amzn1.adg.product.aaaa", "amzn1.adg.product.zzzz"]\n'; else echo '[]'; fi
    exit 0;;
  launch)
    id="$4"
    if [ ! -f "$C/library.json" ]; then echo "Traceback (most recent call last):" >&2; exit 1; fi
    if ! grep -q '"id"' "$C/installed.json"; then log ERROR CLI "No game is installed"; exit 0; fi
    dir=$(installed_path)
    name=""; [ -f "$C/current_user.json" ] && name="sam.plays"
    S="$C/SDK/Amazon Games Services"
    printf '{"command": {"instruction": "%s/bin/Fogline.exe", "arguments": ["-windowed"]}, "game_directory": "%s", "env": {"FUEL_DIR": "%s/Legacy", "AMAZON_GAMES_SDK_PATH": "%s/AmazonGamesSDK", "AMAZON_GAMES_FUEL_ENTITLEMENT_ID": "ent-fog", "AMAZON_GAMES_FUEL_PRODUCT_SKU": "SKU-FOG", "AMAZON_GAMES_FUEL_DISPLAY_NAME": "%s"}, "fake_session": "FAKE-STORE-TOKEN-not-real"}\n' "$dir" "$dir" "$S" "$S" "$name"
    exit 0;;
esac
log ERROR CLI "unknown command"
exit 2
"#;

/// The fake runner for the `PostInstall` steps: records every command and
/// answers with the next scripted exit (`None`: runs until killed), 0 when
/// the script is empty.
#[derive(Default)]
struct Steps {
    exits: Mutex<Vec<Option<i32>>>,
    spawned: Mutex<Vec<InstallerCommand>>,
}

struct StepProc {
    exit: Option<i32>,
    polls: usize,
}

impl InstallerProcess for StepProc {
    fn try_wait(&mut self) -> std::io::Result<Option<Option<i32>>> {
        if self.exit.is_none() {
            return Ok(None);
        }
        if self.polls == 0 {
            return Ok(Some(self.exit));
        }
        self.polls -= 1;
        Ok(None)
    }
    fn kill(&mut self) {}
}

impl InstallerSpawner for Steps {
    fn spawn(&self, cmd: &InstallerCommand) -> std::io::Result<Box<dyn InstallerProcess>> {
        self.spawned.lock().unwrap().push(cmd.clone());
        let mut exits = self.exits.lock().unwrap();
        let exit = if exits.is_empty() {
            Some(0)
        } else {
            exits.remove(0)
        };
        Ok(Box::new(StepProc { exit, polls: 1 }))
    }
}

struct Fixture {
    root: tempfile::TempDir,
    svc: AmazonService,
    registry: Arc<Registry>,
    steps: Arc<Steps>,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let script = root.path().join("nile.sh");
        fs::write(
            &script,
            FAKE.replace(
                "$TOKEN_LINE",
                "DEBUG [AUTH]: token FAKE-STORE-TOKEN-not-real",
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let svc = AmazonService::new(
            data.clone(),
            reqwest::Client::new(),
            AmazonEndpoints::real(),
        )
        .with_tool_override(script);
        let steps = Arc::new(Steps::default());
        let svc = svc.with_redist_runner(RedistRunner {
            spawner: steps.clone(),
            which: Arc::new(|name: &str| {
                (name == "umu-run").then(|| PathBuf::from("/usr/bin/umu-run"))
            }),
            host: "linux".into(),
            watch: Watch {
                stall: Duration::from_secs(60),
                poll: Duration::from_millis(5),
            },
            timeout: Duration::from_secs(60),
            probe: Arc::new(HostProbe),
            system_root: None,
        });
        let stores = data.join("stores");
        fs::create_dir_all(&stores).unwrap();
        fs::write(stores.join("library.json"), LIBRARY).unwrap();
        let registry = Arc::new(Registry::open(&root.path().join("registry.db")).unwrap());
        Fixture {
            root,
            svc,
            registry,
            steps,
        }
    }

    /// The `PostInstall` commands run so far.
    fn spawned(&self) -> Vec<InstallerCommand> {
        self.steps.spawned.lock().unwrap().clone()
    }

    fn set(&self, name: &str) {
        fs::create_dir_all(self.svc.config_dir()).unwrap();
        fs::write(self.svc.config_dir().join(name), "").unwrap();
    }

    fn clear(&self, name: &str) {
        let _ = fs::remove_file(self.svc.config_dir().join(name));
    }

    fn argv(&self) -> String {
        fs::read_to_string(self.root.path().join("data/stores/argv.log")).unwrap_or_default()
    }

    fn ctx(&self) -> AmazonJobCtx {
        AmazonJobCtx {
            registry: self.registry.clone(),
            library: self.root.path().join("library"),
            default_compat_tool: String::new(),
        }
    }

    fn game_dir(&self) -> PathBuf {
        self.root.path().join("library/Amazon Games/Fogline")
    }

    fn request(&self) -> AmazonInstallRequest {
        AmazonInstallRequest {
            product_id: FOGLINE.into(),
            title: "Fogline".into(),
            install_dir: self.game_dir(),
            compat_tool: "proton-ge".into(),
        }
    }

    async fn sign_in(&self) {
        let start = self.svc.start_login().await.unwrap();
        self.svc
            .complete_login(start, SecretString::from("good-code"))
            .await
            .unwrap();
    }

    fn row(&self) -> InstalledGame {
        self.registry
            .find_store(StoreKind::Amazon, FOGLINE)
            .unwrap()
            .unwrap()
    }
}

#[derive(Default)]
struct Sink;

impl StoreProgressSink for Sink {
    fn progress(&mut self, _done: u64, _total: u64) {}
    fn phase(&mut self, _phase: JobPhase) {}
}

type Reports = Vec<(u64, u64, f64)>;

/// Runs a job's download and finalize halves on the blocking pool.
async fn run_job(job: StoreJob) -> (Result<String, String>, Reports) {
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
        let result = job.runner.finalize(&mut Sink).map_err(|e| e.to_string());
        (result, reports)
    })
    .await
    .unwrap()
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn no_token(text: &str) {
    assert!(!text.contains(TOKEN), "{text}");
    assert!(!text.contains("c0ffee"), "{text}");
}

#[tokio::test]
async fn login_clears_a_stale_login_and_keeps_its_values_off_every_error() {
    let f = Fixture::new();
    assert!(f.svc.tool_installed());
    assert_eq!(f.svc.saved_account().await.unwrap(), None);

    // An old (expired) login: nile would refuse `auth --login`.
    f.set("current_user.json");
    fs::write(
        f.svc.config_dir().join(auth::CURRENT_USER_FILE),
        r#"{"name": "old", "user_id": "amzn1.account.OLD"}"#,
    )
    .unwrap();
    let start = f.svc.start_login().await.unwrap();
    assert_eq!(start.url.host_str(), Some("amazon.com"));
    let printed = format!("{start:?}");
    no_token(&printed);
    assert!(!f.svc.has_login(), "the stale login is gone");

    let refused = f
        .svc
        .complete_login(start, SecretString::from("wrong-code"))
        .await
        .unwrap_err();
    assert_eq!(refused.to_string(), auth::LOGIN_REFUSED);
    no_token(&refused.to_string());

    f.set("offline");
    let start = f.svc.start_login().await.unwrap();
    let offline = f
        .svc
        .complete_login(start, SecretString::from("good-code"))
        .await
        .unwrap_err();
    assert!(matches!(offline, StoreError::Network(_)), "{offline}");
    no_token(&offline.to_string());
    f.clear("offline");

    let start = f.svc.start_login().await.unwrap();
    let account = f
        .svc
        .complete_login(start, SecretString::from("good-code"))
        .await
        .unwrap();
    assert_eq!(account.username, "sam.plays");
    let argv = f.argv();
    assert!(argv.contains("auth --login --non-interactive"), "{argv}");
    assert!(
        argv.contains("register --code good-code --code-verifier FAKE-STORE-TOKEN-not-real --serial 5e71a1 --client-id c0ffee"),
        "{argv}"
    );
    assert!(!argv.split_whitespace().any(|a| a == "-d"), "{argv}");

    let dir = f.svc.tool_dir();
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&f.svc.config_dir()), 0o700);
    for file in [
        "current_user.json",
        "0123abcd.enc",
        "installed.json",
        "library.json",
    ] {
        assert_eq!(mode(&f.svc.config_dir().join(file)), 0o600, "{file}");
    }
    assert_eq!(
        f.svc.saved_account().await.unwrap().unwrap().username,
        "sam.plays"
    );
}

#[tokio::test]
async fn the_owned_list_and_an_expired_or_offline_sync() {
    let f = Fixture::new();
    assert!(matches!(f.svc.sync().await, Err(StoreError::NotSignedIn)));
    f.sign_in().await;
    let games = f.svc.sync().await.unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].product_id, FOGLINE);
    assert_eq!(games[0].title, "Fogline");
    assert_eq!(games[0].year, "2021");
    assert_eq!(
        games[0].cover,
        "https://m.media-amazon.com/images/I/fog.png"
    );
    assert_eq!(f.svc.owned_local(), games);

    f.set("expired");
    let expired = f.svc.sync().await.unwrap_err();
    assert!(matches!(expired, StoreError::LoginExpired), "{expired}");
    f.clear("expired");
    f.set("offline");
    let offline = f.svc.sync().await.unwrap_err();
    assert!(matches!(offline, StoreError::Network(_)), "{offline}");
    f.clear("offline");
    // The token file gone: the login is no good.
    fs::remove_file(f.svc.config_dir().join("0123abcd.enc")).unwrap();
    assert!(matches!(f.svc.sync().await, Err(StoreError::LoginExpired)));
    assert_eq!(f.svc.owned_local().len(), 2, "the list stays readable");
}

#[tokio::test]
async fn install_update_verify_launch_and_uninstall() {
    let f = Fixture::new();
    f.sign_in().await;
    assert_eq!(f.svc.download_size(FOGLINE).await.unwrap(), 3000);
    assert!(
        f.svc.updates().await.unwrap().is_empty(),
        "nothing installed"
    );

    let job = f.svc.install_job(f.request(), f.ctx());
    assert_eq!((job.source, job.op), (StoreKind::Amazon, StoreOp::Install));
    let (result, reports) = run_job(job).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(reports.first().map(|r| (r.0, r.1)), Some((0, 3000)));
    assert!(
        reports.iter().any(|r| r.0 == 1500 && r.1 == 3000),
        "{reports:?}"
    );
    assert!(
        reports.iter().any(|r| r.2 == 4.0 * 1024.0 * 1024.0),
        "{reports:?}"
    );
    assert_eq!(reports.last().map(|r| (r.0, r.1)), Some((3000, 3000)));
    let dir = f.game_dir();
    assert!(f.argv().contains(&format!(
        "install --path {} {FOGLINE}",
        dir.to_string_lossy()
    )));
    assert!(!f.argv().contains("--base-path"));

    let row = f.row();
    assert_eq!(row.source, GameSource::Store(StoreKind::Amazon));
    assert_eq!(row.title, "Fogline");
    assert_eq!(row.platform, "Amazon Games");
    assert_eq!(row.platform_slug, "win");
    assert_eq!(row.native_game_dir, dir.to_string_lossy());
    assert_eq!(
        row.native_executable_path,
        dir.join("bin/Fogline.exe").to_string_lossy()
    );
    assert_eq!(row.native_working_dir, "bin");
    assert_eq!(row.native_launch_parameters, "-windowed");
    assert_eq!(row.native_compat_tool, "proton-ge");
    assert_eq!(row.native_wineprefix, dir.join("prefix").to_string_lossy());
    assert!(dir.join("prefix").is_dir());
    assert_eq!(row.companies, "Dune Owl");
    assert_eq!(row.filesize_bytes, 3000);
    let meta = AmazonMeta::from_json(&row.store_meta);
    assert_eq!(meta.version, "v1");
    assert_eq!(meta.product_id, FOGLINE);
    no_token(&row.store_meta);

    // fuel.json's PostInstall step ran once, silently, under umu in the
    // game's prefix, from the game folder, with no FUEL variables.
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 1);
    assert_eq!(
        spawned[0].argv,
        vec![
            "/usr/bin/umu-run".to_string(),
            dir.join("redist")
                .join("vc.exe")
                .to_string_lossy()
                .into_owned(),
            "/quiet".to_string()
        ]
    );
    assert_eq!(spawned[0].cwd, dir);
    assert!(spawned[0]
        .env
        .contains(&("PROTONPATH".into(), "proton-ge".into())));
    assert!(spawned[0].env.contains(&(
        "WINEPREFIX".into(),
        dir.join("prefix").to_string_lossy().into_owned()
    )));
    assert!(!spawned[0]
        .env
        .iter()
        .any(|(k, _)| k.starts_with("AMAZON_GAMES") || k == "FUEL_DIR"));
    assert_eq!(meta.post_install_ran.len(), 1, "recorded as run");
    assert_eq!(meta.post_install_ran[0].command, "redist\\vc.exe");

    // The launch dry run: nile's exe, no doubled args, the five FUEL vars.
    let prep = f.svc.launch_prep(&row).await.unwrap();
    assert_eq!(prep.executable, dir.join("bin/Fogline.exe"));
    assert!(prep.game_args.is_empty());
    assert!(prep.secret_args.is_empty());
    let keys: Vec<&str> = prep.env.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, launch::FUEL_ENV);
    let printed = format!("{prep:?}");
    assert!(!printed.contains("sam.plays"), "{printed}");
    assert!(f
        .argv()
        .contains(&format!("launch --json --no-wine {FOGLINE}")));

    // Updates: installed ids only.
    f.set("update");
    let updates = f.svc.updates().await.unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].product_id, FOGLINE);
    f.clear("update");

    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert!(f.argv().contains(&format!("update {FOGLINE}")));
    let row = f.row();
    assert_eq!(AmazonMeta::from_json(&row.store_meta).version, "v2");

    // Already current passes; an update that leaves the version fails.
    f.set("up-to-date");
    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    f.clear("up-to-date");
    f.set("silent-fail");
    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    let error = result.unwrap_err();
    assert!(error.contains("before it finished"), "{error}");
    no_token(&error);
    f.clear("silent-fail");

    let verify = f.svc.verify_job(&row, f.ctx());
    assert_eq!(verify.op, StoreOp::Repair);
    assert_eq!(verify.runner.download_phase(), JobPhase::Verifying);
    let runner = verify.runner.clone();
    let (result, reports) = run_job(verify).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(runner.detail(), "", "\"Verified\", no count");
    assert!(reports.iter().any(|r| r.0 == 1500), "{reports:?}");
    assert!(f.argv().contains(&format!("verify {FOGLINE}")));
    assert_eq!(f.spawned().len(), 1, "an unchanged list never runs again");

    f.svc.uninstall(&f.registry, FOGLINE).unwrap();
    assert!(!dir.exists(), "the folder and its prefix go");
    assert!(f
        .registry
        .find_store(StoreKind::Amazon, FOGLINE)
        .unwrap()
        .is_none());
    assert!(f.argv().contains(&format!("uninstall {FOGLINE}")));
    assert!(f.svc.installed().is_empty());
    assert!(f.svc.uninstall(&f.registry, FOGLINE).is_err());
}

/// A sink whose cancel turns on after `polls` questions.
struct CancelSink(std::cell::Cell<usize>);

impl StoreProgressSink for CancelSink {
    fn progress(&mut self, _done: u64, _total: u64) {}
    fn phase(&mut self, _phase: JobPhase) {}
    fn cancelled(&self) -> bool {
        let left = self.0.get();
        if left == 0 {
            return true;
        }
        self.0.set(left - 1);
        false
    }
}

#[tokio::test]
async fn post_install_steps_warn_never_fail_and_rerun_when_their_list_changes() {
    let f = Fixture::new();
    f.sign_in().await;

    // A failing step: the install still succeeds, with a warning, and the
    // list doesn't count as run.
    f.steps.exits.lock().unwrap().push(Some(1));
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(
        result.unwrap(),
        "Couldn't run vc.exe: it exited with code 1"
    );
    let row = f.row();
    assert!(AmazonMeta::from_json(&row.store_meta)
        .post_install_ran
        .is_empty());

    // A verify runs it again (it never worked), and now it counts.
    let (result, _) = run_job(f.svc.verify_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(f.spawned().len(), 2);
    let row = f.row();
    assert_eq!(
        AmazonMeta::from_json(&row.store_meta)
            .post_install_ran
            .len(),
        1
    );

    // An update whose fuel.json lists one more step: only the new one runs
    // (GRID's prefix marker skips vc.exe, which already worked there).
    f.set("post-two");
    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 3);
    assert!(spawned[2].argv[1].ends_with("oalinst.exe"));
    assert_eq!(spawned[2].argv[2], "/silent");
    let row = f.row();
    assert_eq!(
        AmazonMeta::from_json(&row.store_meta)
            .post_install_ran
            .len(),
        2
    );

    // No compat tool anywhere: a warning in GRID's words, nothing runs.
    // (GRID's prefix marker would skip both steps; forget it.)
    fs::remove_file(
        f.game_dir()
            .join("prefix")
            .join(grid_core::stores::redist::PREFIX_MARKER),
    )
    .unwrap();
    let mut bare = row.clone();
    bare.native_compat_tool = String::new();
    let mut meta = AmazonMeta::from_json(&bare.store_meta);
    meta.post_install_ran.clear();
    bare.store_meta = meta.to_json();
    let (result, _) = run_job(f.svc.verify_job(&bare, f.ctx())).await;
    let warning = result.unwrap();
    assert!(
        warning.starts_with("Couldn't run vc.exe: choose a compatibility tool"),
        "{warning}"
    );
    assert_eq!(warning.lines().count(), 2, "{warning}");
    no_token(&warning);
    assert_eq!(f.spawned().len(), 3);
}

#[tokio::test]
async fn the_setup_steps_job_runs_only_the_steps_and_never_nile() {
    let f = Fixture::new();
    f.sign_in().await;
    f.steps.exits.lock().unwrap().push(Some(1));
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(
        result.unwrap(),
        "Couldn't run vc.exe: it exited with code 1"
    );
    let row = f.row();
    let nile_before = f.argv();

    // Signed out: the setup steps need no login.
    f.clear("current_user.json");
    assert!(!f.svc.has_login());
    let job = f.svc.setup_steps_job(&row, f.ctx()).unwrap();
    assert_eq!(job.op, StoreOp::SetupSteps);
    assert_eq!(job.op.kind(), "store_setup");
    let (result, reports) = run_job(job).await;
    assert_eq!(result.unwrap(), "");
    assert!(reports.is_empty(), "no download: {reports:?}");
    assert_eq!(f.argv(), nile_before, "no nile call");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 2);
    assert!(spawned[1].argv[1].ends_with("vc.exe"));
    assert_eq!(
        AmazonMeta::from_json(&f.row().store_meta)
            .post_install_ran
            .len(),
        1,
        "the list counts once every step worked"
    );

    // A game whose fuel.json lists no steps is refused.
    let mut bare = row.clone();
    bare.native_game_dir = f
        .root
        .path()
        .join("elsewhere")
        .to_string_lossy()
        .into_owned();
    bare.extracted_dir = bare.native_game_dir.clone();
    let refusal = f.svc.setup_steps_job(&bare, f.ctx()).err().unwrap();
    assert_eq!(refusal.to_string(), "Fogline has no setup steps to run.");
}

#[tokio::test]
async fn a_cancel_during_post_install_steps_leaves_the_game_installed() {
    let f = Fixture::new();
    f.sign_in().await;
    f.steps.exits.lock().unwrap().push(None);
    let job = f.svc.install_job(f.request(), f.ctx());
    let result = tokio::task::spawn_blocking(move || {
        job.runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .unwrap();
        job.runner
            .finalize(&mut CancelSink(std::cell::Cell::new(5)))
            .map_err(|e| e.to_string())
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap(), REDIST_CANCELLED);
    let row = f.row();
    assert!(AmazonMeta::from_json(&row.store_meta)
        .post_install_ran
        .is_empty());
    assert!(f.game_dir().join("bin/Fogline.exe").is_file());
}

#[tokio::test]
async fn failed_installs_say_why_and_write_no_row() {
    let f = Fixture::new();
    f.sign_in().await;

    f.set("no-space");
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    let error = result.unwrap_err();
    assert_eq!(error, "nile stopped: there isn't enough disk space");
    f.clear("no-space");

    // nile prints "Download complete" but installed nothing.
    f.set("silent-fail");
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert!(result.unwrap_err().contains("before it finished"));
    f.clear("silent-fail");
    assert!(f
        .registry
        .find_store(StoreKind::Amazon, FOGLINE)
        .unwrap()
        .is_none());

    // nile #51: a dropped connection that still ended "complete" runs
    // verify once, which decides.
    f.set("network-once");
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert!(f.argv().contains(&format!("verify {FOGLINE}")));
    assert!(f
        .registry
        .find_store(StoreKind::Amazon, FOGLINE)
        .unwrap()
        .is_some());

    // Signed out, no job starts nile.
    f.svc.disconnect().await.unwrap();
    let (result, _) = run_job(f.svc.update_job(&f.row(), f.ctx())).await;
    assert_eq!(result.unwrap_err(), "not signed in");
}

#[tokio::test]
async fn a_cancelled_first_install_leaves_no_folder() {
    let f = Fixture::new();
    f.sign_in().await;
    f.set("slow");
    let job = f.svc.install_job(f.request(), f.ctx());
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let dir = f.game_dir();
    let watcher = std::thread::spawn(move || {
        for _ in 0..100 {
            if dir.join("bin").is_dir() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        flag.store(true, Ordering::Relaxed);
    });
    let result =
        tokio::task::spawn_blocking(move || job.runner.download(&cancel, &mut |_, _, _| {}))
            .await
            .unwrap();
    watcher.join().unwrap();
    assert!(matches!(result, Err(LibraryError::Cancelled)), "{result:?}");
    assert!(!f.game_dir().exists(), "the new folder is removed");
    assert!(f.game_dir().parent().unwrap().exists(), "its parent stays");
}

#[tokio::test]
async fn uninstalling_what_nile_forgot_still_removes_the_files() {
    let f = Fixture::new();
    f.sign_in().await;
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    result.unwrap();
    let dir = f.game_dir();

    // The manifest is gone: nile crashes, GRID forgets the game itself.
    fs::remove_file(f.svc.config_dir().join(format!("manifests/{FOGLINE}.raw"))).unwrap();
    f.svc.uninstall(&f.registry, FOGLINE).unwrap();
    assert!(!dir.exists());
    assert!(f.svc.installed().is_empty(), "dropped from installed.json");

    // After Disconnect (nile's folder gone, even when logout fails) the
    // games stay, and uninstall still works.
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    result.unwrap();
    f.set("logout-fail");
    f.svc.disconnect().await.unwrap();
    assert!(!f.svc.tool_dir().exists());
    assert_eq!(f.svc.saved_account().await.unwrap(), None);
    assert!(dir.exists(), "installed games stay");
    f.svc.uninstall(&f.registry, FOGLINE).unwrap();
    assert!(!dir.exists());
}

#[tokio::test]
async fn a_failed_launch_prep_is_a_validation_error_in_grids_words() {
    let f = Fixture::new();
    f.sign_in().await;
    let row = InstalledGame {
        title: "Fogline".into(),
        store_id: FOGLINE.into(),
        source: GameSource::Store(StoreKind::Amazon),
        ..Default::default()
    };
    let error = f.svc.launch_prep(&row).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("doesn't have this game installed"),
        "{error}"
    );
    let text = launch::launch_error(&row.title, &error).to_string();
    assert!(
        text.starts_with("Couldn\u{2019}t start Fogline through nile: "),
        "{text}"
    );
    no_token(&text);
    assert_eq!(
        install::folder_name("Fogline™: Redux", FOGLINE),
        "Fogline Redux"
    );
}
