//! A gogdl Windows install's setup steps against a fake gogdl (a `sh`
//! script: `info`, `update`/`repair` writing the game and its build
//! manifest with `scriptInterpreter` and `dependencies`, and `redist`
//! writing `__redist/<ID>/<exe>` and `.gogdl-redist-manifest`) and a fake
//! runner that records every step's command: the script then the
//! redistributables after an install, the full set in `gogdl redist
//! --ids`, the script alone after an update, nothing after a repair, the
//! new DLC's script after a DLC add, a failed download as a warning, and
//! Retry (the setup-steps job) running only what didn't work.
//!
//! The fake prints `FAKE-STORE-TOKEN-not-real` on stderr and puts it in the
//! manifest's client secret, so the tests check no warning or meta has it.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grid_core::library::queue::{JobPhase, StoreJob, StoreProgressSink};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::gog::meta::{GogMeta, GogPlatform};
use grid_core::stores::gog::setup::{REDIST_MANIFEST, SCRIPT_ID};
use grid_core::stores::gog::{GogEndpoints, GogInstallRequest, GogJobCtx, GogService};
use grid_core::stores::offline::exec::{
    InstallerCommand, InstallerProcess, InstallerSpawner, Watch,
};
use grid_core::stores::redist::{read_marker, HostProbe, RedistRunner};
use grid_core::stores::setup_steps::NO_SETUP_STEPS;
use grid_core::stores::StoreKind;

const TOKEN: &str = "FAKE-STORE-TOKEN-not-real";
const TERN: &str = "100";

/// The fake gogdl. `$GOGDL_CONFIG_PATH` is GRID's gogdl folder; `build`,
/// `deps` and `redist-fail` files there change its answers.
const FAKE: &str = r#"#!/bin/sh
C="$GOGDL_CONFIG_PATH"
mkdir -p "$C"
echo "$*" >> "$C/argv.log"
echo "[AUTH] DEBUG: token FAKE-STORE-TOKEN-not-real" >&2
shift 2
cmd="$1"; shift
build=$(cat "$C/build" 2>/dev/null || echo 77)
case "$cmd" in
  info)
    printf '{"size": {"*": {"download_size": 10, "disk_size": 20}}, "buildId": "%s", "versionName": "1.0", "languages": ["en-US"], "dlcs": [{"id": "201", "title": "Bonus"}], "dependencies": []}\n' "$build"
    exit 0;;
  update|repair)
    id="$1"; shift; dir=""; dlcs=""
    while [ $# -gt 0 ]; do
      case "$1" in --path) dir="$2"; shift;; --dlcs) dlcs="$2"; shift;; esac
      shift
    done
    mkdir -p "$dir" "$C/heroic_gogdl/manifests"
    printf '{"gameId": "%s", "rootGameId": "%s", "name": "Tern", "buildId": "%s", "playTasks": [{"isPrimary": true, "category": "game", "path": "tern.exe"}]}' "$id" "$id" "$build" > "$dir/goggame-$id.info"
    echo MZ > "$dir/tern.exe"
    products="{\"productId\": \"$id\", \"temp_executable\": \"\"}"
    [ -n "$dlcs" ] && products="$products, {\"productId\": \"$dlcs\", \"temp_executable\": \"\"}"
    deps=$(cat "$C/deps" 2>/dev/null || echo '["MSVC2019_x64", "openAL", "DOSBox074", "QuickTime"]')
    printf '{"version": 2, "buildId": "%s", "scriptInterpreter": true, "dependencies": %s, "products": [%s], "clientSecret": "FAKE-STORE-TOKEN-not-real"}' "$build" "$deps" "$products" > "$C/heroic_gogdl/manifests/$id"
    echo "[PROGRESS] INFO: = Progress: 100.00 10/10, Running for: 00:00:01, ETA: 00:00:00" >&2
    exit 0;;
  redist)
    ids=""; out=""
    while [ $# -gt 0 ]; do
      case "$1" in --ids) ids="$2"; shift;; --path) out="$2"; shift;; esac
      shift
    done
    if [ -f "$C/redist-fail" ]; then echo "Unable to proceed, Not enough disk space"; exit 2; fi
    mkdir -p "$out"
    for item in ISI:scriptinterpreter.exe MSVC2019_x64:VC_redist.x64.exe openAL:oalinst.exe DirectX:DXSETUP.exe QuickTime:QuickTimeInstaller.exe; do
      id="${item%%:*}"; exe="${item#*:}"
      case ",$ids," in *",$id,"*) mkdir -p "$out/__redist/$id"; echo "MZ $id" > "$out/__redist/$id/$exe";; esac
    done
    cat > "$out/.gogdl-redist-manifest" <<'REPO'
{"build_id": "601", "HGLInstalled": [], "depots": [
 {"dependencyId": "ISI", "executable": {"path": "__redist/ISI/scriptinterpreter.exe", "arguments": ""}, "internal": true},
 {"dependencyId": "MSVC2019_x64", "executable": {"path": "__redist/MSVC2019_x64/VC_redist.x64.exe", "arguments": "/install /quiet /norestart"}, "internal": false},
 {"dependencyId": "openAL", "executable": {"path": "__redist/openAL/oalinst.exe", "arguments": "/S"}, "internal": false},
 {"dependencyId": "DirectX", "executable": {"path": "__redist/DirectX/DXSETUP.exe", "arguments": "/silent"}, "internal": false},
 {"dependencyId": "QuickTime", "executable": {"path": "__redist/QuickTime/QuickTimeInstaller.exe", "arguments": ""}, "internal": true},
 {"dependencyId": "DOSBox074", "executable": {"path": "", "arguments": ""}, "internal": true}
]}
REPO
    echo "[PROGRESS] INFO: = Progress: 100.00 3/3, Running for: 00:00:01, ETA: 00:00:00" >&2
    exit 0;;
esac
echo "unknown command" >&2
exit 2
"#;

/// The fake step runner: records every command and answers with the next
/// scripted exit, 0 when the script is empty.
#[derive(Default)]
struct Steps {
    exits: Mutex<Vec<Option<i32>>>,
    spawned: Mutex<Vec<InstallerCommand>>,
}

struct StepProc(Option<i32>);

impl InstallerProcess for StepProc {
    fn try_wait(&mut self) -> std::io::Result<Option<Option<i32>>> {
        Ok(Some(self.0))
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
        Ok(Box::new(StepProc(exit)))
    }
}

struct Fixture {
    root: tempfile::TempDir,
    svc: GogService,
    registry: Arc<Registry>,
    steps: Arc<Steps>,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("gogdl.sh");
        fs::write(&script, FAKE).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let steps = Arc::new(Steps::default());
        let svc = GogService::new(
            root.path().join("data"),
            reqwest::Client::new(),
            GogEndpoints::real(),
        )
        .with_tool_override(script)
        .with_redist_runner(RedistRunner {
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
        let registry = Arc::new(Registry::open(&root.path().join("registry.db")).unwrap());
        Fixture {
            root,
            svc,
            registry,
            steps,
        }
    }

    fn tool_dir(&self) -> PathBuf {
        self.root.path().join("data/stores/gogdl")
    }

    fn set(&self, name: &str, text: &str) {
        let file = self.tool_dir().join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn argv(&self) -> String {
        fs::read_to_string(self.tool_dir().join("argv.log")).unwrap_or_default()
    }

    fn redist_calls(&self) -> Vec<String> {
        self.argv()
            .lines()
            .filter(|l| l.contains(" redist "))
            .map(str::to_string)
            .collect()
    }

    /// The commands run so far: (program, arguments).
    fn spawned(&self) -> Vec<(String, Vec<String>)> {
        self.steps
            .spawned
            .lock()
            .unwrap()
            .iter()
            .map(|cmd| {
                // umu-run <program> <args…>
                (cmd.argv[1].clone(), cmd.argv[2..].to_vec())
            })
            .collect()
    }

    fn ctx(&self) -> GogJobCtx {
        GogJobCtx {
            registry: self.registry.clone(),
            library: self.root.path().join("library"),
            default_compat_tool: String::new(),
        }
    }

    fn game_dir(&self) -> PathBuf {
        self.root.path().join("library/GOG/Tern")
    }

    fn request(&self) -> GogInstallRequest {
        GogInstallRequest {
            store_id: TERN.into(),
            title: "Tern".into(),
            platform: GogPlatform::Windows,
            language: "en-US".into(),
            dlc_ids: Vec::new(),
            install_dir: self.game_dir(),
            compat_tool: "proton-ge".into(),
        }
    }

    fn row(&self) -> InstalledGame {
        self.registry
            .find_store(StoreKind::Gog, TERN)
            .unwrap()
            .unwrap()
    }

    fn meta(&self) -> GogMeta {
        GogMeta::from_json(&self.row().store_meta)
    }
}

#[derive(Default)]
struct Sink;

impl StoreProgressSink for Sink {
    fn progress(&mut self, _done: u64, _total: u64) {}
    fn phase(&mut self, _phase: JobPhase) {}
}

/// Runs a job's download and finalize halves on the blocking pool.
async fn run_job(job: StoreJob) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        job.runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .map_err(|e| e.to_string())?;
        job.runner.finalize(&mut Sink).map_err(|e| e.to_string())
    })
    .await
    .unwrap()
}

fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap()
}

#[tokio::test]
async fn an_install_runs_the_script_then_each_redistributable_in_the_prefix() {
    let f = Fixture::new();
    // Another installed gogdl game: its dependencies stay in the shared
    // folder, so the call lists them too.
    f.set(
        "heroic_gogdl/manifests/999",
        r#"{"version": 2, "dependencies": ["DirectX"], "products": []}"#,
    );

    let warning = run_job(f.svc.install_job(f.request(), f.ctx()))
        .await
        .unwrap();
    assert_eq!(warning, "");

    let redist = f.tool_dir().join("redist");
    let calls = f.redist_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        calls[0].ends_with(&format!(
            "redist --ids ISI,MSVC2019_x64,openAL,DOSBox074,QuickTime,DirectX --path {}",
            redist.display()
        )),
        "the full set: {}",
        calls[0]
    );
    assert!(calls[0].starts_with("--auth-config-path "));
    assert!(redist.join(REDIST_MANIFEST).is_file());

    let spawned = f.spawned();
    let names: Vec<&str> = spawned.iter().map(|(p, _)| name(p)).collect();
    assert_eq!(
        names,
        ["scriptinterpreter.exe", "VC_redist.x64.exe", "oalinst.exe"],
        "no DOSBox, no QuickTime"
    );
    let (isi, args) = &spawned[0];
    assert_eq!(
        PathBuf::from(isi),
        redist.join("__redist/ISI/scriptinterpreter.exe")
    );
    let game = f.game_dir();
    let win = |p: &std::path::Path| format!("Z:{}", p.display().to_string().replace('/', "\\"));
    assert_eq!(
        args,
        &[
            "/VERYSILENT".to_string(),
            format!("/DIR={}", win(&game)),
            "/Language=English".into(),
            "/LANG=English".into(),
            "/ProductId=100".into(),
            "/galaxyclient".into(),
            "/buildId=77".into(),
            "/versionName=1.0".into(),
            "/lang-code=en-US".into(),
            format!("/supportDir={}", win(&f.tool_dir().join("support/100"))),
            "/nodesktopshorctut".into(),
            "/nodesktopshortcut".into(),
        ]
    );
    assert_eq!(spawned[1].1, ["/install", "/quiet", "/norestart"]);
    assert_eq!(spawned[2].1, ["/S"]);
    for cmd in f.steps.spawned.lock().unwrap().iter() {
        assert_eq!(cmd.prefix.as_deref(), Some(game.join("prefix").as_path()));
        assert_eq!(cmd.cwd, game);
    }

    let meta = f.meta();
    let ids: Vec<&str> = meta.setup_ran.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [&format!("{SCRIPT_ID}100:77"), "MSVC2019_x64", "openAL"]
    );
    assert_eq!(read_marker(&game.join("prefix")).len(), 3);
    let row = f.row();
    assert!(!row.store_meta.contains(TOKEN), "{}", row.store_meta);
}

#[tokio::test]
async fn an_update_reruns_only_the_script_a_repair_nothing_and_a_dlc_its_own_script() {
    let f = Fixture::new();
    run_job(f.svc.install_job(f.request(), f.ctx()))
        .await
        .unwrap();
    assert_eq!(f.spawned().len(), 3);

    // An update to build 78: the script again, with the new build.
    f.set("build", "78");
    let warning = run_job(f.svc.update_job(&f.row(), f.ctx())).await.unwrap();
    assert_eq!(warning, "");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 4);
    assert_eq!(name(&spawned[3].0), "scriptinterpreter.exe");
    assert!(spawned[3].1.contains(&"/buildId=78".to_string()));
    assert_eq!(f.redist_calls().len(), 1, "everything was on disk already");
    let ids: Vec<String> = f.meta().setup_ran.iter().map(|s| s.id.clone()).collect();
    assert_eq!(
        ids,
        [
            format!("{SCRIPT_ID}100:78"),
            "MSVC2019_x64".into(),
            "openAL".into()
        ]
    );

    // A repair runs none.
    run_job(f.svc.repair_job(&f.row(), f.ctx())).await.unwrap();
    assert_eq!(f.spawned().len(), 4);

    // A DLC: its own script; the base game's and the redistributables
    // already ran in this prefix.
    let warning = run_job(f.svc.dlc_job(&f.row(), "201", f.ctx()))
        .await
        .unwrap();
    assert_eq!(warning, "");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 5, "{spawned:?}");
    assert!(spawned[4].1.contains(&"/ProductId=201".to_string()));
}

#[tokio::test]
async fn a_failed_step_is_a_warning_and_retry_runs_only_what_didnt_work() {
    let f = Fixture::new();
    f.steps
        .exits
        .lock()
        .unwrap()
        .extend([Some(0), Some(1603), Some(0)]);
    let warning = run_job(f.svc.install_job(f.request(), f.ctx()))
        .await
        .unwrap();
    assert_eq!(
        warning,
        "Couldn't run VC_redist.x64.exe: it exited with code 1603"
    );
    assert!(f.meta().setup_ran.is_empty(), "nothing recorded");

    // Retry: no game download, no new redist call, only the failed step.
    let argv_before = f.argv();
    let job = f.svc.setup_steps_job(&f.row(), f.ctx()).unwrap();
    assert_eq!(run_job(job).await.unwrap(), "");
    assert_eq!(f.argv(), argv_before, "no gogdl run at all");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 4);
    assert_eq!(name(&spawned[3].0), "VC_redist.x64.exe");
    assert_eq!(f.meta().setup_ran.len(), 3);

    // With the shared folder gone (a Disconnect deletes gogdl's folder),
    // Retry fetches it again first.
    fs::remove_dir_all(f.tool_dir().join("redist")).unwrap();
    let job = f.svc.setup_steps_job(&f.row(), f.ctx()).unwrap();
    assert_eq!(run_job(job).await.unwrap(), "");
    assert_eq!(f.redist_calls().len(), 2);
}

#[tokio::test]
async fn a_failed_download_of_the_redistributables_is_a_warning_never_a_failed_install() {
    let f = Fixture::new();
    f.set("redist-fail", "");
    let warning = run_job(f.svc.install_job(f.request(), f.ctx()))
        .await
        .unwrap();
    assert_eq!(
        warning,
        "Couldn't run its setup steps: GRID couldn't download them \
         (gogdl stopped: there isn't enough disk space)"
    );
    assert!(!warning.contains(TOKEN));
    assert!(f.spawned().is_empty(), "nothing to run without the files");
    assert!(f.meta().setup_ran.is_empty());
    assert_eq!(f.row().title, "Tern", "the game is installed");

    // Retry with the download still failing: the job fails, saying why.
    let job = f.svc.setup_steps_job(&f.row(), f.ctx()).unwrap();
    let error = run_job(job).await.unwrap_err();
    assert!(error.contains("GRID couldn't download them"), "{error}");
    assert!(!error.contains(TOKEN));
}

#[tokio::test]
async fn game_folder_dependencies_never_run_and_other_games_refuse_retry() {
    let f = Fixture::new();
    // Only a game-folder dependency (gogdl already put DOSBox in the game
    // folder): the script runs, DOSBox never does.
    f.set("deps", "[\"DOSBox074\"]");
    let warning = run_job(f.svc.install_job(f.request(), f.ctx()))
        .await
        .unwrap();
    assert_eq!(warning, "");
    let names: Vec<String> = f
        .spawned()
        .iter()
        .map(|(p, _)| name(p).to_string())
        .collect();
    assert_eq!(names, ["scriptinterpreter.exe"]);
    assert_eq!(
        f.redist_calls()[0]
            .split(" --ids ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap(),
        "ISI,DOSBox074"
    );

    // A Linux build (or an offline install) has no setup steps.
    let mut linux = f.row();
    let mut meta = GogMeta::from_json(&linux.store_meta);
    meta.platform = GogPlatform::Linux;
    linux.store_meta = meta.to_json();
    let refusal = f.svc.setup_steps_job(&linux, f.ctx()).err().unwrap();
    assert_eq!(refusal.to_string(), NO_SETUP_STEPS);
}
