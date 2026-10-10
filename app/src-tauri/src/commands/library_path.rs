//! Settings › Library (Q2): check a new library folder, preview what Start
//! fresh › Delete would remove, and run Start fresh (Leave or Delete).
//! Thin wrappers — every rule lives in `grid_core::library::path_change`
//! and `InstallService`. Payloads carry paths, titles and counts only.
//!
//! Phase 9b adds "Move existing files" as a background job. It reuses
//! [`library_activity`], `path_change::validate_new_library_path`,
//! `path_change::prepare_library_dir` and `path_change::switch_library_root`
//! (called once the files have moved), and gets its own command here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use grid_core::config::Config;
use grid_core::library::path_change::{
    self, LibraryActivity, PathChangeRefusal, StartFreshPreview, StartFreshReport,
};
use grid_core::library::paths::library_root;
use grid_core::library::InstallService;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::{err, AppState};
use crate::config_write::modify_config;
use crate::relink_service::INSTALLED_CHANGED_EVENT;

/// [`check_library_path`]'s answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LibraryPathCheck {
    /// The folder can become the library root; `path` is how it will be
    /// stored.
    Ok { path: String },
    Refused {
        reason: PathChangeRefusal,
        message: String,
    },
}

/// What a Start fresh call did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StartFreshOutcome {
    /// `library_path` is the new root. `rows_removed` games left the
    /// library; `failures` lists what could not be removed (Delete only —
    /// those games stay in the library).
    Switched {
        library_path: String,
        rows_removed: usize,
        failures: Vec<String>,
    },
    /// Nothing changed.
    Refused {
        reason: PathChangeRefusal,
        message: String,
    },
    /// The library folder changed since the pane read it. Nothing changed.
    Stale { message: String },
}

const STALE_MESSAGE: &str =
    "The library folder changed since this window opened. Close it and try again.";

/// Start fresh's two answers to "Delete the old game files, or leave them
/// on disk?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OldFiles {
    Leave,
    Delete,
}

/// What is running right now: live download-queue entries (firmware rows
/// included), running games, cloud transfers.
pub(crate) fn library_activity(state: &AppState) -> LibraryActivity {
    LibraryActivity {
        installs: state
            .install
            .as_ref()
            .is_ok_and(|install| install.snapshot().has_live_entry()),
        running_games: state
            .launch
            .as_ref()
            .is_ok_and(|launch| !launch.snapshot().sessions.is_empty()),
        cloud_transfers: state.cloud.transfers_active(),
    }
}

fn refused(reason: PathChangeRefusal) -> LibraryPathCheck {
    LibraryPathCheck::Refused {
        reason,
        message: reason.message().to_string(),
    }
}

/// Checks `path` as the new library root against the current one and what
/// is running. Creates nothing.
#[tauri::command]
pub async fn check_library_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<LibraryPathCheck, String> {
    let activity = library_activity(&state);
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(
            match path_change::validate_new_library_path(&config.library_path, &path, activity) {
                Ok(target) => LibraryPathCheck::Ok {
                    path: target.to_string_lossy().into_owned(),
                },
                Err(reason) => refused(reason),
            },
        )
    })
    .await
    .map_err(|e| format!("check_library_path did not finish: {e}"))?
}

/// What Start fresh › Delete would remove from the current library: the
/// folders per game and the total size. Reads only. An unset library has
/// nothing to remove.
#[tauri::command]
pub async fn library_start_fresh_preview(
    state: State<'_, AppState>,
) -> Result<StartFreshPreview, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    tokio::task::spawn_blocking(move || {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        match library_root(&config) {
            Some(old_root) => install.start_fresh_preview(&old_root).map_err(err),
            None => Ok(StartFreshPreview {
                old_root: String::new(),
                games: Vec::new(),
                total_bytes: 0,
                left_outside: Vec::new(),
            }),
        }
    })
    .await
    .map_err(|e| format!("library_start_fresh_preview did not finish: {e}"))?
}

/// Start fresh › Leave: switch the library to `path` and take the old
/// library's games out of the registry. No file is touched.
#[tauri::command]
pub async fn library_start_fresh_keep(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    expected_old_root: String,
) -> Result<StartFreshOutcome, String> {
    start_fresh(&state, app, path, expected_old_root, OldFiles::Leave).await
}

/// Start fresh › Delete (after the confirmation): switch the library to
/// `path`, then uninstall the old library's games through the guarded
/// uninstall path.
#[tauri::command]
pub async fn library_start_fresh_delete(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
    expected_old_root: String,
) -> Result<StartFreshOutcome, String> {
    start_fresh(&state, app, path, expected_old_root, OldFiles::Delete).await
}

async fn start_fresh(
    state: &AppState,
    app: AppHandle,
    path: String,
    expected_old_root: String,
    old_files: OldFiles,
) -> Result<StartFreshOutcome, String> {
    let install = state.install.as_ref().map_err(Clone::clone)?.clone();
    let activity = library_activity(state);
    let config_path = Config::default_path();
    let blocking_install = install.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        run_start_fresh(
            &blocking_install,
            &config_path,
            &path,
            &expected_old_root,
            activity,
            old_files,
        )
    })
    .await
    .map_err(|e| format!("start fresh did not finish: {e}"))??;

    if let StartFreshOutcome::Switched { rows_removed, .. } = &outcome {
        let _ = app.emit(INSTALLED_CHANGED_EVENT, *rows_removed);
        state
            .updates
            .spawn_refresh(app.clone(), state.session.clone(), install);
    }
    Ok(outcome)
}

/// The blocking body of Start fresh, in the order that keeps every
/// failure safe: re-check the old root and the path, create the new
/// folder, switch the config (the old root becomes a protected former
/// root), then drop or uninstall the old rows. A failure before the switch
/// changes nothing; a failure after it leaves the affected games in the
/// library, where they still work from their old folders.
fn run_start_fresh(
    install: &Arc<InstallService>,
    config_path: &Path,
    path: &str,
    expected_old_root: &str,
    activity: LibraryActivity,
    old_files: OldFiles,
) -> Result<StartFreshOutcome, String> {
    let config = Config::load(config_path).map_err(err)?;
    let current = config.library_path.clone();
    if current.trim() != expected_old_root.trim() {
        return Ok(StartFreshOutcome::Stale {
            message: STALE_MESSAGE.to_string(),
        });
    }
    let target: PathBuf = match path_change::validate_new_library_path(&current, path, activity)
        .and_then(|target| path_change::prepare_library_dir(&target).map(|()| target))
    {
        Ok(target) => target,
        Err(reason) => {
            return Ok(StartFreshOutcome::Refused {
                reason,
                message: reason.message().to_string(),
            })
        }
    };
    let old_root = library_root(&config);

    let switched = modify_config(config_path, |config| {
        if config.library_path != current {
            return Ok(false);
        }
        path_change::switch_library_root(config, &target);
        Ok(true)
    })?;
    if !switched {
        return Ok(StartFreshOutcome::Stale {
            message: STALE_MESSAGE.to_string(),
        });
    }

    let report = match old_root {
        None => Ok(StartFreshReport::default()),
        Some(old_root) => match old_files {
            OldFiles::Leave => install.start_fresh_forget(&old_root),
            OldFiles::Delete => install.start_fresh_delete(&old_root),
        },
    };
    let (rows_removed, failures) = match report {
        Ok(report) => (report.rows_removed, report.failures),
        Err(e) => (0, vec![err(e)]),
    };
    tracing::info!(
        rows_removed,
        failures = failures.len(),
        delete = old_files == OldFiles::Delete,
        "library folder changed (start fresh)"
    );
    Ok(StartFreshOutcome::Switched {
        library_path: target.to_string_lossy().into_owned(),
        rows_removed,
        failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::library::registry::{InstalledGame, Registry};

    const IDLE: LibraryActivity = LibraryActivity {
        installs: false,
        running_games: false,
        cloud_transfers: false,
    };

    fn s(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    struct Setup {
        dir: tempfile::TempDir,
        config_path: PathBuf,
        registry: Arc<Registry>,
        install: Arc<InstallService>,
        old: PathBuf,
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        std::fs::create_dir_all(old.join("games").join("SNES").join("Mario")).unwrap();
        std::fs::write(
            old.join("games")
                .join("SNES")
                .join("Mario")
                .join("game.sfc"),
            b"rom",
        )
        .unwrap();
        let config_path = dir.path().join("config.toml");
        Config {
            library_path: s(&old),
            ..Default::default()
        }
        .save(&config_path)
        .unwrap();
        let registry = Arc::new(Registry::open(&dir.path().join("registry.db")).unwrap());
        registry
            .upsert(&InstalledGame {
                title: "Mario".into(),
                platform: "SNES".into(),
                rom_id: Some(1),
                extracted_dir: s(&old.join("games").join("SNES").join("Mario")),
                ..Default::default()
            })
            .unwrap();
        let install = InstallService::new(registry.clone(), config_path.clone());
        Setup {
            dir,
            config_path,
            registry,
            install,
            old,
        }
    }

    #[test]
    fn leave_switches_the_root_records_the_old_one_and_keeps_the_files() {
        let t = setup();
        let new = t.dir.path().join("new");
        let outcome = run_start_fresh(
            &t.install,
            &t.config_path,
            &s(&new),
            &s(&t.old),
            IDLE,
            OldFiles::Leave,
        )
        .unwrap();

        assert_eq!(
            outcome,
            StartFreshOutcome::Switched {
                library_path: s(&new),
                rows_removed: 1,
                failures: Vec::new(),
            }
        );
        assert!(new.is_dir());
        let config = Config::load(&t.config_path).unwrap();
        assert_eq!(config.library_path, s(&new));
        assert_eq!(config.former_library_paths, vec![s(&t.old)]);
        assert!(t.registry.all().unwrap().is_empty());
        assert!(t
            .old
            .join("games")
            .join("SNES")
            .join("Mario")
            .join("game.sfc")
            .is_file());
    }

    #[test]
    fn delete_switches_the_root_and_removes_the_old_game() {
        let t = setup();
        let new = t.dir.path().join("new");
        let outcome = run_start_fresh(
            &t.install,
            &t.config_path,
            &s(&new),
            &s(&t.old),
            IDLE,
            OldFiles::Delete,
        )
        .unwrap();

        assert!(matches!(
            outcome,
            StartFreshOutcome::Switched {
                rows_removed: 1,
                ..
            }
        ));
        assert!(!t.old.join("games").join("SNES").join("Mario").exists());
        assert!(t.old.join("games").join("SNES").is_dir());
    }

    #[test]
    fn a_refusal_or_a_stale_root_changes_nothing() {
        let t = setup();
        let inside = t.old.join("inner");
        let busy = LibraryActivity {
            running_games: true,
            ..IDLE
        };
        let cases = [
            (
                s(&inside),
                s(&t.old),
                IDLE,
                StartFreshOutcome::Refused {
                    reason: PathChangeRefusal::InsideCurrent,
                    message: PathChangeRefusal::InsideCurrent.message().to_string(),
                },
            ),
            (
                s(&t.dir.path().join("new")),
                s(&t.old),
                busy,
                StartFreshOutcome::Refused {
                    reason: PathChangeRefusal::GameRunning,
                    message: PathChangeRefusal::GameRunning.message().to_string(),
                },
            ),
            (
                s(&t.dir.path().join("new")),
                "/some/other/root".to_string(),
                IDLE,
                StartFreshOutcome::Stale {
                    message: STALE_MESSAGE.to_string(),
                },
            ),
        ];
        for (path, expected_old, activity, expected) in cases {
            for old_files in [OldFiles::Leave, OldFiles::Delete] {
                let outcome = run_start_fresh(
                    &t.install,
                    &t.config_path,
                    &path,
                    &expected_old,
                    activity,
                    old_files,
                )
                .unwrap();
                assert_eq!(outcome, expected, "{path}");
            }
        }
        let config = Config::load(&t.config_path).unwrap();
        assert_eq!(config.library_path, s(&t.old));
        assert!(config.former_library_paths.is_empty());
        assert_eq!(t.registry.all().unwrap().len(), 1);
        assert!(!t.dir.path().join("new").exists());
        assert!(t
            .old
            .join("games")
            .join("SNES")
            .join("Mario")
            .join("game.sfc")
            .is_file());
    }
}
