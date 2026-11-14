//! An Epic game's prerequisite (legendary's `prereq_info`) against a fake
//! legendary and a fake step runner: it runs last in a first install, in
//! the game's prefix from its own folder with its arguments split; an
//! update runs it again only when it changed; `null` or legendary's
//! `installed` mark runs nothing; a failed step is a warning, and the
//! setup-steps job (Retry) runs only the prerequisite, with no legendary
//! call.
//!
//! The fake keeps one installed game: `<config>/entry.json` (what
//! `list-installed` prints) and legendary's own `installed.json`. Its
//! prerequisite is `<stores>/prereq.json` (`null` when missing).
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grid_core::library::queue::{JobPhase, StoreJob, StoreOp, StoreProgressSink};
use grid_core::library::registry::{InstalledGame, Registry};
use grid_core::stores::epic::meta::EpicMeta;
use grid_core::stores::epic::{
    EpicEndpoints, EpicGame, EpicInstallRequest, EpicJobCtx, EpicService,
};
use grid_core::stores::offline::exec::{
    InstallerCommand, InstallerProcess, InstallerSpawner, Watch,
};
use grid_core::stores::redist::{HostProbe, RedistRunner, PREFIX_MARKER};
use grid_core::stores::StoreKind;

const UE: &str = "Engine/Extras/Redist/en-us/UE4PrereqSetup_x64.exe";

/// The fake legendary. `$LEGENDARY_CONFIG_PATH` is its state folder; its
/// shared files (`prereq.json`, `latest`, `argv.log`) sit beside it.
const FAKE: &str = r#"#!/bin/sh
C="$LEGENDARY_CONFIG_PATH"
S="$C/.."
mkdir -p "$C"
echo "$*" >> "$S/argv.log"
echo "[Core] DEBUG: token FAKE-STORE-TOKEN-not-real" >&2
prereq() { if [ -f "$S/prereq.json" ]; then cat "$S/prereq.json"; else printf null; fi; }
case "$1" in
  list-installed)
    if [ -f "$C/entry.json" ]; then printf '['; cat "$C/entry.json"; printf ']\n'; else echo '[]'; fi
    exit 0;;
  info)
    printf '{"game": {"app_name": "%s"}, "install": null, "manifest": {"download_size": 10, "disk_size": 20, "prerequisites": %s}}\n' "$2" "$(prereq)"
    exit 0;;
  install|update|repair)
    app="$2"; cmd="$1"
    if [ "$cmd" = install ]; then
      base=""; folder=""
      while [ $# -gt 0 ]; do
        case "$1" in --base-path) base="$2"; shift;; --game-folder) folder="$2"; shift;; esac
        shift
      done
      dir="$base/$folder"; version=1.0.3
    else
      dir=$(cat "$C/dir"); version=$(cat "$C/version")
      [ "$cmd" = update ] && version=$(cat "$S/latest")
    fi
    mkdir -p "$dir/Engine/Extras/Redist/en-us" "$dir/Redist"
    echo game > "$dir/Game.exe"
    echo ue-prereqs > "$dir/Engine/Extras/Redist/en-us/UE4PrereqSetup_x64.exe"
    echo vc-redist > "$dir/Redist/vc_redist.x64.exe"
    echo "$dir" > "$C/dir"; echo "$version" > "$C/version"
    echo "[cli] INFO: Download size: 1.00 MiB" >&2
    printf '{"app_name": "%s", "title": "Starlight Courier", "version": "%s", "install_path": "%s", "install_size": 4096, "executable": "Game.exe", "launch_parameters": "", "is_dlc": false, "can_run_offline": true, "save_path": null, "prereq_info": %s}' "$app" "$version" "$dir" "$(prereq)" > "$C/entry.json"
    { printf '{"%s": ' "$app"; cat "$C/entry.json"; printf '}'; } > "$C/installed.json"
    if [ "$(prereq)" != null ]; then
      echo "This game lists the following prerequisites to be installed:"
      echo "[cli] INFO: Automatic installation not available on Linux." >&2
    fi
    exit 0;;
esac
echo "[cli] ERROR: unknown command" >&2
exit 2
"#;

/// The fake step runner: records every command and answers with the next
/// scripted exit, 0 when the script is empty.
#[derive(Default)]
struct Steps {
    exits: Mutex<Vec<i32>>,
    spawned: Mutex<Vec<InstallerCommand>>,
}

struct StepProc(i32);

impl InstallerProcess for StepProc {
    fn try_wait(&mut self) -> std::io::Result<Option<Option<i32>>> {
        Ok(Some(Some(self.0)))
    }
    fn kill(&mut self) {}
}

impl InstallerSpawner for Steps {
    fn spawn(&self, cmd: &InstallerCommand) -> std::io::Result<Box<dyn InstallerProcess>> {
        self.spawned.lock().unwrap().push(cmd.clone());
        let mut exits = self.exits.lock().unwrap();
        let exit = if exits.is_empty() { 0 } else { exits.remove(0) };
        Ok(Box::new(StepProc(exit)))
    }
}

struct Fixture {
    root: tempfile::TempDir,
    svc: EpicService,
    registry: Arc<Registry>,
    steps: Arc<Steps>,
}

impl Fixture {
    fn new() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let script = root.path().join("legendary.sh");
        fs::write(&script, FAKE).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let steps = Arc::new(Steps::default());
        let svc = EpicService::new(data.clone(), reqwest::Client::new(), EpicEndpoints::real())
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
        let stores = data.join("stores");
        fs::create_dir_all(&stores).unwrap();
        fs::write(stores.join("latest"), "1.0.4").unwrap();
        let registry = Arc::new(Registry::open(&root.path().join("registry.db")).unwrap());
        Fixture {
            root,
            svc,
            registry,
            steps,
        }
    }

    /// Sets the game's prerequisite (`None`: `null`).
    fn prereq(&self, json: Option<&str>) {
        let file = self.root.path().join("data/stores/prereq.json");
        match json {
            Some(json) => fs::write(file, json).unwrap(),
            None => {
                let _ = fs::remove_file(file);
            }
        }
    }

    fn spawned(&self) -> Vec<InstallerCommand> {
        self.steps.spawned.lock().unwrap().clone()
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

    fn game_dir(&self) -> PathBuf {
        self.root.path().join("library/Epic Games/Starlight")
    }

    fn request(&self) -> EpicInstallRequest {
        EpicInstallRequest {
            app_name: "Starlight".into(),
            title: "Starlight Courier".into(),
            dlc: Vec::new(),
            install_dir: self.game_dir(),
            compat_tool: "proton-ge".into(),
        }
    }

    fn row(&self) -> InstalledGame {
        self.registry
            .find_store(StoreKind::Epic, "Starlight")
            .unwrap()
            .unwrap()
    }

    fn ran(&self) -> usize {
        EpicMeta::from_json(&self.row().store_meta).prereq_ran.len()
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

/// Runs a job's download and finalize halves on the blocking pool: the
/// result, and the phases finalize showed.
async fn run_job(job: StoreJob) -> (Result<String, String>, Vec<JobPhase>) {
    tokio::task::spawn_blocking(move || {
        job.runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .map_err(|e| e.to_string())?;
        let mut sink = Sink::default();
        let result = job.runner.finalize(&mut sink).map_err(|e| e.to_string());
        Ok::<_, String>((result, sink.phases))
    })
    .await
    .unwrap()
    .unwrap_or_else(|error| (Err(error), Vec::new()))
}

fn ue_prereq(args: &str) -> String {
    format!(
        r#"{{"ids": ["uepre"], "name": "UE4 Prerequisites (x64)", "path": "{UE}", "args": "{args}"}}"#
    )
}

#[tokio::test]
async fn an_install_runs_the_prerequisite_last_in_the_prefix_from_its_own_folder() {
    let f = Fixture::new();
    f.prereq(Some(&ue_prereq("/quiet /norestart")));
    let (result, phases) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(
        phases,
        vec![JobPhase::Installing, JobPhase::InstallerSilent],
        "the prerequisite is the last step"
    );
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 1);
    let cmd = &spawned[0];
    assert_eq!(cmd.argv[0], "/usr/bin/umu-run");
    assert!(
        cmd.argv[1].ends_with("UE4PrereqSetup_x64.exe"),
        "{:?}",
        cmd.argv
    );
    assert_eq!(&cmd.argv[2..], ["/quiet", "/norestart"]);
    let dir = f.game_dir();
    assert_eq!(cmd.cwd, dir.join("Engine/Extras/Redist/en-us"));
    assert_eq!(cmd.prefix.as_deref(), Some(dir.join("prefix").as_path()));
    let row = f.row();
    assert_eq!(row.native_wineprefix, dir.join("prefix").to_string_lossy());
    let meta = EpicMeta::from_json(&row.store_meta);
    assert_eq!(meta.prereq_ran.len(), 1);
    assert_eq!(meta.prereq_ran[0].command, UE);
    assert!(meta.prereq_ran[0].in_exe_folder);
    assert!(dir.join("prefix").join(PREFIX_MARKER).is_file());
    // GRID never writes legendary's own mark.
    let installed = fs::read_to_string(f.svc.tool_dir().join("installed.json")).unwrap();
    assert!(!installed.contains("\"installed\""), "{installed}");
    assert!(!row.store_meta.contains("FAKE-STORE-TOKEN"));

    // An update with the same prerequisite runs nothing.
    let (result, _) = run_job(f.svc.update_job(&row, f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(f.spawned().len(), 1);
    assert_eq!(EpicMeta::from_json(&f.row().store_meta).version, "1.0.4");

    // An update whose prerequisite changed runs the new one.
    fs::write(f.root.path().join("data/stores/latest"), "1.0.5").unwrap();
    f.prereq(Some(
        r#"{"ids": ["vc"], "name": "VC++", "path": "Redist\\vc_redist.x64.exe", "args": "/install /quiet /norestart"}"#,
    ));
    let (result, _) = run_job(f.svc.update_job(&f.row(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 2);
    assert!(spawned[1].argv[1].ends_with("vc_redist.x64.exe"));
    assert_eq!(&spawned[1].argv[2..], ["/install", "/quiet", "/norestart"]);
    assert_eq!(spawned[1].cwd, dir.join("Redist"));
    assert_eq!(
        EpicMeta::from_json(&f.row().store_meta).prereq_ran[0].id,
        "vc"
    );

    // A game that no longer lists one forgets it, and runs nothing.
    f.prereq(None);
    let (result, _) = run_job(f.svc.repair_job(&f.row(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(f.spawned().len(), 2);
    assert_eq!(f.ran(), 0);
}

#[tokio::test]
async fn null_or_an_installed_prerequisite_runs_nothing() {
    let f = Fixture::new();
    let (result, phases) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(phases, vec![JobPhase::Installing], "no setup phase");
    assert!(f.spawned().is_empty());
    assert_eq!(f.ran(), 0);
    let refusal = f.svc.setup_steps_job(&f.row(), f.ctx()).err().unwrap();
    assert_eq!(
        refusal.to_string(),
        "Starlight Courier has no setup steps to run."
    );

    let f = Fixture::new();
    f.prereq(Some(&format!(
        r#"{{"ids": ["uepre"], "name": "UE", "path": "{UE}", "args": "/quiet", "installed": true}}"#
    )));
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(result.unwrap(), "");
    assert!(f.spawned().is_empty(), "legendary marked it installed");
}

#[tokio::test]
async fn a_failed_prerequisite_is_a_warning_and_retry_runs_only_it() {
    let f = Fixture::new();
    f.prereq(Some(&ue_prereq("/quiet /norestart")));
    f.steps.exits.lock().unwrap().push(1);
    let (result, _) = run_job(f.svc.install_job(f.request(), f.ctx())).await;
    assert_eq!(
        result.unwrap(),
        "Couldn't run UE4PrereqSetup_x64.exe: it exited with code 1"
    );
    assert!(f.game_dir().join("Game.exe").is_file(), "still installed");
    assert_eq!(f.ran(), 0, "it didn't work");

    // Retry: the setup-steps job, no legendary call.
    let legendary_before = f.argv();
    let job = f.svc.setup_steps_job(&f.row(), f.ctx()).unwrap();
    assert_eq!(job.op, StoreOp::SetupSteps);
    assert_eq!(job.source, StoreKind::Epic);
    let (result, _) = run_job(job).await;
    assert_eq!(result.unwrap(), "");
    assert_eq!(f.argv(), legendary_before, "no legendary call");
    let spawned = f.spawned();
    assert_eq!(spawned.len(), 2);
    assert_eq!(&spawned[1].argv[2..], ["/quiet", "/norestart"]);
    assert_eq!(
        spawned[1].cwd,
        f.game_dir().join("Engine/Extras/Redist/en-us")
    );
    assert_eq!(f.ran(), 1, "recorded once it worked");

    // A game another launcher manages has none to run.
    f.svc.set_owned(vec![EpicGame {
        app_name: "Starlight".into(),
        third_party: Some("Ubisoft Connect".into()),
        ..Default::default()
    }]);
    let refusal = f.svc.setup_steps_job(&f.row(), f.ctx()).err().unwrap();
    assert_eq!(
        refusal.to_string(),
        "Starlight Courier has no setup steps to run."
    );
}
