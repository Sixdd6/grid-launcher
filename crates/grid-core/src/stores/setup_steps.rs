//! The "run only the setup steps" job (`StoreOp::SetupSteps`, Downloads
//! kind `store_setup`), shared by every store that has setup steps
//! (Amazon's fuel.json `PostInstall` now; GOG's scripts and redists and
//! Epic's prerequisites next).
//!
//! Retry on a store row whose warning came from its setup steps (a
//! declined Windows prompt, a failed or timed-out step, no compat tool)
//! queues this job instead of a Verify and repair (user decision
//! 2026-11-14): it reads the game's steps again, skips what is already
//! installed or already ran in the prefix ([`run_steps`]), runs the rest,
//! and records the list as run only when every step worked. No network,
//! no login, no store tool.
//!
//! A store supplies the steps and the record through [`SetupStepsStore`];
//! [`setup_steps_job`] builds the job from the game's row.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::library::queue::{JobPhase, StoreJob, StoreJobRunner, StoreOp, StoreProgressSink};
use crate::library::registry::InstalledGame;
use crate::library::LibraryError;
use crate::stores::gog::install::game_dir;
use crate::stores::redist::{run_steps, RedistRunner, RedistStep, RedistTarget};
use crate::stores::StoreKind;

/// The refusal for a store with no setup steps (the default of the app's
/// `StoreBackend::setup_steps_job`).
pub const NO_SETUP_STEPS: &str = "This store has no setup steps.";

/// The job's failure when the game's folder is gone.
pub const SETUP_FOLDER_GONE: &str =
    "GRID can't run its setup steps: its folder is gone. Reinstall it.";

/// What one store gives the setup-steps job for one game.
pub trait SetupStepsStore: Send + Sync {
    /// The game's setup steps now, re-read from the store's own files when
    /// the job runs (Amazon: the game's fuel.json). Commands resolve
    /// against the game folder.
    fn steps(&self) -> Result<Vec<RedistStep>, LibraryError>;
    /// Records `ran` as the list that last worked (Amazon:
    /// `AmazonMeta::post_install_ran`). Called only when every step
    /// worked or was already satisfied, never after a warning or a cancel.
    fn record(&self, ran: &[RedistStep]) -> Result<(), LibraryError>;
}

/// The job's runner.
struct SetupStepsJob {
    game_dir: PathBuf,
    compat: String,
    prefix: Option<PathBuf>,
    store: Arc<dyn SetupStepsStore>,
    runner: RedistRunner,
}

/// The setup-steps job for the installed `row` of `source`: its folder
/// (`native_game_dir`, else `extracted_dir`), its prefix
/// (`native_wineprefix`; blank on Windows) and its compat tool (the row's,
/// else `default_compat`).
pub fn setup_steps_job(
    source: StoreKind,
    row: &InstalledGame,
    default_compat: &str,
    store: Arc<dyn SetupStepsStore>,
    runner: RedistRunner,
) -> StoreJob {
    let compat = match row.native_compat_tool.trim() {
        "" => default_compat.trim().to_string(),
        chosen => chosen.to_string(),
    };
    let prefix = Some(row.native_wineprefix.trim())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    StoreJob {
        source,
        store_id: row.store_id.trim().to_string(),
        op: StoreOp::SetupSteps,
        title: row.title.clone(),
        drive: None,
        runner: Arc::new(SetupStepsJob {
            game_dir: game_dir(row),
            compat,
            prefix,
            store,
            runner,
        }),
    }
}

impl StoreJobRunner for SetupStepsJob {
    /// Nothing to download: only checks the game folder is there.
    fn download(
        &self,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        if self.game_dir.as_os_str().is_empty() || !self.game_dir.is_dir() {
            return Err(LibraryError::Extract(SETUP_FOLDER_GONE.to_string()));
        }
        Ok(())
    }

    fn download_phase(&self) -> JobPhase {
        JobPhase::InstallerSilent
    }

    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        sink.phase(JobPhase::InstallerSilent);
        sink.progress(0, 1);
        let steps = self.store.steps()?;
        let outcome = run_steps(
            &steps,
            RedistTarget {
                game_dir: &self.game_dir,
                compat: &self.compat,
                prefix: self.prefix.as_deref(),
            },
            &self.runner,
            sink,
        );
        if outcome.cancelled {
            return Err(LibraryError::Cancelled);
        }
        if outcome.all_worked {
            self.store.record(&steps)?;
        }
        sink.progress(1, 1);
        Ok(outcome.warning())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::offline::{InstallerCommand, InstallerProcess, InstallerSpawner, Watch};
    use crate::stores::redist::{HostProbe, REDIST_DECLINED};
    use std::sync::Mutex;
    use std::time::Duration;

    /// Scripted exits, 0 when empty; records each command.
    #[derive(Default)]
    struct Spawner {
        exits: Mutex<Vec<Option<i32>>>,
        spawned: Mutex<Vec<InstallerCommand>>,
    }

    struct Proc(Option<i32>);

    impl InstallerProcess for Proc {
        fn try_wait(&mut self) -> std::io::Result<Option<Option<i32>>> {
            Ok(self.0.map(Some))
        }
        fn kill(&mut self) {}
    }

    impl InstallerSpawner for Spawner {
        fn spawn(&self, cmd: &InstallerCommand) -> std::io::Result<Box<dyn InstallerProcess>> {
            self.spawned.lock().unwrap().push(cmd.clone());
            let mut exits = self.exits.lock().unwrap();
            let exit = if exits.is_empty() {
                Some(0)
            } else {
                exits.remove(0)
            };
            Ok(Box::new(Proc(exit)))
        }
    }

    #[derive(Default)]
    struct Store {
        steps: Mutex<Vec<RedistStep>>,
        recorded: Mutex<Vec<Vec<RedistStep>>>,
    }

    impl SetupStepsStore for Store {
        fn steps(&self) -> Result<Vec<RedistStep>, LibraryError> {
            Ok(self.steps.lock().unwrap().clone())
        }
        fn record(&self, ran: &[RedistStep]) -> Result<(), LibraryError> {
            self.recorded.lock().unwrap().push(ran.to_vec());
            Ok(())
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

    fn runner(spawner: Arc<Spawner>, host: &str) -> RedistRunner {
        RedistRunner {
            spawner,
            which: Arc::new(|name: &str| {
                (name == "umu-run").then(|| PathBuf::from("/usr/bin/umu-run"))
            }),
            host: host.into(),
            watch: Watch {
                stall: Duration::from_secs(60),
                poll: Duration::from_millis(1),
            },
            timeout: Duration::from_secs(60),
            probe: Arc::new(HostProbe),
            system_root: None,
        }
    }

    fn step(command: &str) -> RedistStep {
        RedistStep {
            id: String::new(),
            command: command.into(),
            args: vec!["/silent".into()],
            valid_returns: vec![],
            in_exe_folder: false,
        }
    }

    struct Game {
        _root: tempfile::TempDir,
        row: InstalledGame,
    }

    fn game(compat: &str) -> Game {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("Kettle Drum");
        std::fs::create_dir_all(dir.join("prefix")).unwrap();
        std::fs::write(dir.join("oalinst.exe"), b"MZ installer").unwrap();
        let row = InstalledGame {
            title: "Kettle Drum".into(),
            store_id: "kettle".into(),
            native_game_dir: dir.to_string_lossy().into_owned(),
            native_wineprefix: dir.join("prefix").to_string_lossy().into_owned(),
            native_compat_tool: compat.into(),
            ..Default::default()
        };
        Game { _root: root, row }
    }

    fn run(job: &StoreJob, sink: &mut Sink) -> Result<String, LibraryError> {
        job.runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})?;
        job.runner.finalize(sink)
    }

    #[test]
    fn the_job_is_a_store_neutral_setup_steps_row_of_the_game() {
        let g = game("");
        let store = Arc::new(Store::default());
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "GE-Proton9-20",
            store,
            runner(Arc::new(Spawner::default()), "linux"),
        );
        assert_eq!(job.op, StoreOp::SetupSteps);
        assert_eq!(job.op.kind(), "store_setup");
        assert_eq!(job.store_id, "kettle");
        assert_eq!(job.title, "Kettle Drum");
        assert!(job.drive.is_none());
        assert_eq!(job.runner.download_phase(), JobPhase::InstallerSilent);
    }

    #[test]
    fn it_runs_the_steps_under_the_compat_tool_and_records_them_when_all_worked() {
        let g = game("");
        let spawner = Arc::new(Spawner::default());
        let store = Arc::new(Store::default());
        *store.steps.lock().unwrap() = vec![step("oalinst.exe")];
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "GE-Proton9-20",
            store.clone(),
            runner(spawner.clone(), "linux"),
        );
        let mut sink = Sink::default();
        assert_eq!(run(&job, &mut sink).unwrap(), "");
        assert!(sink.phases.contains(&JobPhase::InstallerSilent));
        let spawned = spawner.spawned.lock().unwrap().clone();
        assert_eq!(spawned.len(), 1);
        assert_eq!(spawned[0].argv[0], "/usr/bin/umu-run");
        assert!(spawned[0].argv[1].ends_with("oalinst.exe"));
        assert_eq!(spawned[0].argv[2], "/silent");
        assert_eq!(
            *store.recorded.lock().unwrap(),
            vec![vec![step("oalinst.exe")]]
        );

        // Again: GRID's prefix marker says it already ran there, so nothing
        // runs and the list still counts.
        assert_eq!(run(&job, &mut Sink::default()).unwrap(), "");
        assert_eq!(spawner.spawned.lock().unwrap().len(), 1);
        assert_eq!(store.recorded.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_failed_step_or_no_compat_tool_is_a_warning_and_records_nothing() {
        let g = game("");
        let spawner = Arc::new(Spawner::default());
        let store = Arc::new(Store::default());
        *store.steps.lock().unwrap() = vec![step("oalinst.exe")];

        // No compat tool on the row and no default.
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "",
            store.clone(),
            runner(spawner.clone(), "linux"),
        );
        let warning = run(&job, &mut Sink::default()).unwrap();
        assert!(
            warning.starts_with("Couldn't run oalinst.exe: choose a compatibility tool"),
            "{warning}"
        );
        assert!(spawner.spawned.lock().unwrap().is_empty());

        // A step that exits 1.
        spawner.exits.lock().unwrap().push(Some(1));
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "GE-Proton9-20",
            store.clone(),
            runner(spawner.clone(), "linux"),
        );
        assert_eq!(
            run(&job, &mut Sink::default()).unwrap(),
            "Couldn't run oalinst.exe: it exited with code 1"
        );
        assert!(store.recorded.lock().unwrap().is_empty());
    }

    #[test]
    fn a_declined_windows_prompt_is_a_warning() {
        let mut g = game("");
        g.row.native_wineprefix = String::new();
        let spawner = Arc::new(Spawner::default());
        spawner
            .exits
            .lock()
            .unwrap()
            .push(Some(crate::stores::redist::DECLINED_EXIT));
        let store = Arc::new(Store::default());
        *store.steps.lock().unwrap() = vec![step("oalinst.exe")];
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "",
            store.clone(),
            runner(spawner, "windows"),
        );
        let warning = run(&job, &mut Sink::default()).unwrap();
        assert!(warning.lines().any(|w| w == REDIST_DECLINED), "{warning}");
        assert!(store.recorded.lock().unwrap().is_empty());
    }

    #[test]
    fn a_missing_folder_fails_before_anything_runs() {
        let mut g = game("proton");
        g.row.native_game_dir = g._root.path().join("gone").to_string_lossy().into_owned();
        let store = Arc::new(Store::default());
        *store.steps.lock().unwrap() = vec![step("oalinst.exe")];
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "",
            store.clone(),
            runner(Arc::new(Spawner::default()), "linux"),
        );
        let error = job
            .runner
            .download(&AtomicBool::new(false), &mut |_, _, _| {})
            .unwrap_err();
        assert_eq!(error.to_string(), SETUP_FOLDER_GONE);
        assert!(store.recorded.lock().unwrap().is_empty());
    }

    #[test]
    fn a_cancel_during_a_step_cancels_the_row() {
        struct Cancel;
        impl StoreProgressSink for Cancel {
            fn progress(&mut self, _d: u64, _t: u64) {}
            fn phase(&mut self, _p: JobPhase) {}
            fn cancelled(&self) -> bool {
                true
            }
        }
        let g = game("GE-Proton9-20");
        let store = Arc::new(Store::default());
        *store.steps.lock().unwrap() = vec![step("oalinst.exe")];
        let job = setup_steps_job(
            StoreKind::Amazon,
            &g.row,
            "",
            store.clone(),
            runner(Arc::new(Spawner::default()), "linux"),
        );
        let error = job.runner.finalize(&mut Cancel).unwrap_err();
        assert!(matches!(error, LibraryError::Cancelled));
        assert!(store.recorded.lock().unwrap().is_empty());
    }
}
