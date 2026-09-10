//! App-layer server-update tracking (doc 10 "When checks run" / "How an
//! available update is surfaced"). grid-core decides whether ONE row has an
//! update (`library::update_detection`); this module decides WHEN the whole
//! library is re-checked and holds the transient result. Never persisted
//! (doc 10 invariant 5).
//!
//! Triggers (commands.rs / lib.rs): a session comes up (connect, restore,
//! retry), a game finalizes — the base, update and native-update merge
//! paths all fire `InstallService`'s game-finalized hook — or a game is
//! uninstalled. A disconnect clears the set. No timer, no polling.
//!
//! Token secrecy: nothing here logs a URL or header; fetch failures log the
//! rom id only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use grid_core::library::registry::InstalledGame;
use grid_core::library::update_detection::{
    format_version_tag, game_has_server_update, has_newer_server_rom_version,
    is_emulators_platform, rom_file_name_version, ServerVersion,
};
use grid_core::library::InstallService;
use grid_core::session::SessionManager;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

pub const UPDATES_CHANGED_EVENT: &str = "updates-changed";
/// Verbatim (details_view_mixin.py:1818).
pub const UPDATE_GONE: &str = "A newer server version is no longer available for this game.";
const MAX_IN_FLIGHT: usize = 4;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub server_rom_file_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateRow {
    pub rom_id: i64,
    pub label: String,
}

pub struct UpdateService {
    available: Mutex<HashMap<i64, UpdateInfo>>,
    generation: AtomicU64,
    gate: PassGate,
}

/// Collapses overlapping refresh triggers into at most one pass in flight
/// plus at most one queued rerun.
///
/// Several triggers fire close together in practice — a connect that also
/// finalizes a queued install, a batch of installs completing — and each one
/// used to spawn a full pass over the whole registry. The generation guard
/// made the extra passes harmless but not free: every one of them re-fetched
/// a rom detail for every installed game. So a trigger that arrives while a
/// pass is running does not start one; it asks the running pass to go around
/// once more when it ends, and repeated triggers collapse into that single
/// rerun. The registry snapshot is taken at the START of a pass, which is
/// why a rerun is needed at all rather than trusting the pass in flight.
///
/// Both decisions have to be atomic with the flag they read, so they live
/// here on the flags rather than as free functions.
#[derive(Default)]
struct PassGate {
    /// A pass is running (or a rerun has just been claimed for one).
    in_flight: AtomicBool,
    /// A trigger arrived while a pass was running.
    rerun_requested: AtomicBool,
}

impl PassGate {
    /// A trigger arrives: `true` when the caller must start a pass, `false`
    /// when a pass is already running — in which case the request is
    /// recorded and [`Self::should_rerun`] will pick it up.
    ///
    /// The request is recorded BEFORE the claim so a pass ending
    /// concurrently either sees it or loses the claim to us; a claim we win
    /// consumes it, because the pass we are about to start IS that run.
    fn should_start(&self) -> bool {
        self.rerun_requested.store(true, Ordering::SeqCst);
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.rerun_requested.store(false, Ordering::SeqCst);
        true
    }

    /// A pass has just finished: `true` when it must run exactly once more,
    /// having re-claimed the in-flight slot for that rerun. The request is
    /// consumed either way, so two triggers during one pass buy one rerun,
    /// not two. `false` also covers a trigger that claimed the slot for
    /// itself in the gap — that pass is the rerun.
    fn should_rerun(&self) -> bool {
        self.in_flight.store(false, Ordering::SeqCst);
        if !self.rerun_requested.swap(false, Ordering::SeqCst) {
            return false;
        }
        !self.in_flight.swap(true, Ordering::SeqCst)
    }
}

impl UpdateService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            available: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(0),
            gate: PassGate::default(),
        })
    }

    /// `_details_update_button_text_for_game` (grid-launcher.py:3300-3325).
    pub fn button_label_for(installed_rom_file_name: &str, server_rom_file_name: &str) -> String {
        if !has_newer_server_rom_version(installed_rom_file_name, server_rom_file_name) {
            return "Update".to_string();
        }
        match rom_file_name_version(server_rom_file_name) {
            Some(tag) => format!("Update to {}", format_version_tag(&tag)),
            None => "Update".to_string(),
        }
    }

    /// The rows the frontend renders, in ascending rom id order.
    pub fn rows(&self, installed: &[InstalledGame]) -> Vec<UpdateRow> {
        rows_of(&self.available.lock().unwrap(), installed)
    }

    /// Whether one rom carries an update. Not a command: the frontend reads
    /// the whole set through [`Self::rows`].
    #[cfg(test)]
    pub fn has_update(&self, rom_id: i64) -> bool {
        self.available.lock().unwrap().contains_key(&rom_id)
    }

    /// The AppHandle-free half of [`Self::clear`]: bumps the generation so
    /// any pass in flight discards its result, drops every entry, and
    /// reports whether the set had anything in it.
    fn take_all(&self) -> bool {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let mut available = self.available.lock().unwrap();
        !std::mem::take(&mut *available).is_empty()
    }

    /// Drops every entry; emits the event only when something changed.
    pub fn clear(&self, app: &AppHandle) {
        if self.take_all() {
            let _ = app.emit(UPDATES_CHANGED_EVENT, Vec::<UpdateRow>::new());
        }
    }

    /// Commits one pass's result, and returns the rows to emit — or [`None`]
    /// when a newer pass (or a [`Self::clear`]) has already bumped the
    /// generation, in which case nothing is written.
    ///
    /// The generation is re-checked UNDER the `available` lock: checking it
    /// first would let a `clear` land in the gap, emit its empty set, and
    /// then have this pass silently repopulate the map behind it.
    fn store_if_current(
        &self,
        generation: u64,
        next: HashMap<i64, UpdateInfo>,
        installed: &[InstalledGame],
    ) -> Option<Vec<UpdateRow>> {
        let mut available = self.available.lock().unwrap();
        if self.generation.load(Ordering::SeqCst) != generation {
            return None;
        }
        *available = next;
        Some(rows_of(&available, installed))
    }

    /// One full pass over the registry. Runs on Tauri's async runtime; a
    /// pass that is overtaken by a newer one discards its result.
    ///
    /// Returns without spawning when a pass is already in flight ([`PassGate`]):
    /// that pass runs once more instead, so a burst of triggers costs two
    /// walks of the registry at most rather than one per trigger.
    pub fn spawn_refresh(
        self: &Arc<Self>,
        app: AppHandle,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
    ) {
        if !self.gate.should_start() {
            return;
        }
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut guard = PassGuard::new(this.clone());
            loop {
                this.clone()
                    .refresh(app.clone(), session.clone(), install.clone())
                    .await;
                if !this.gate.should_rerun() {
                    // `should_rerun` already settled the flag, one way or
                    // the other: nothing left for the guard to do.
                    guard.disarm();
                    break;
                }
            }
        });
    }

    async fn refresh(
        self: Arc<Self>,
        app: AppHandle,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
    ) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(client) = session.client() else {
            self.clear(&app);
            return;
        };
        let rows = match tokio::task::spawn_blocking({
            let install = install.clone();
            move || install.registry().all()
        })
        .await
        {
            Ok(Ok(rows)) => rows,
            _ => {
                tracing::warn!("update check skipped: registry read failed");
                return;
            }
        };
        let semaphore = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
        let mut tasks = Vec::new();
        // Cloned per row: `rows` itself is kept for the commit below, so the
        // emitted set is built from the snapshot this pass actually checked
        // rather than a second registry read that could fail or disagree.
        for row in rows.iter().cloned() {
            let Some(rom_id) = row.rom_id else { continue };
            if is_emulators_platform(&row.platform) {
                continue;
            }
            let client = client.clone();
            let semaphore = semaphore.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = semaphore.acquire_owned().await.ok()?;
                let detail = match client.rom_detail(rom_id).await {
                    Ok(detail) => detail,
                    Err(_) => {
                        tracing::debug!("update check: rom {rom_id} detail fetch failed");
                        return None;
                    }
                };
                let server = ServerVersion {
                    platform: &detail.platform_name,
                    rom_file_name: &detail.fs_name,
                    updated_at: &detail.server_updated_at,
                };
                game_has_server_update(&row, &server).then(|| {
                    (
                        rom_id,
                        UpdateInfo {
                            server_rom_file_name: detail.fs_name.clone(),
                        },
                    )
                })
            }));
        }
        let mut next = HashMap::new();
        for task in tasks {
            if let Ok(Some((rom_id, info))) = task.await {
                next.insert(rom_id, info);
            }
        }
        // `None` means overtaken by a newer pass (or a clear): write
        // nothing and emit nothing.
        let Some(emitted) = self.store_if_current(generation, next, &rows) else {
            return;
        };
        let _ = app.emit(UPDATES_CHANGED_EVENT, emitted);
    }
}

/// Releases [`PassGate`]'s in-flight flag when the pass task unwinds.
///
/// The normal path disarms it — by then [`PassGate::should_rerun`] has
/// either freed the flag or handed it to the trigger that claimed it, and
/// clearing it again would let a second pass start beside that one. Only a
/// panic leaves the flag set with no task behind it, which would otherwise
/// block every later update check for the life of the process.
struct PassGuard {
    service: Option<Arc<UpdateService>>,
}

impl PassGuard {
    fn new(service: Arc<UpdateService>) -> Self {
        Self {
            service: Some(service),
        }
    }

    fn disarm(&mut self) {
        self.service = None;
    }
}

impl Drop for PassGuard {
    fn drop(&mut self) {
        if let Some(service) = self.service.take() {
            service.gate.in_flight.store(false, Ordering::SeqCst);
        }
    }
}

/// [`UpdateService::rows`]'s body, over an already-locked map: the commit
/// step needs the rows computed under the same lock it writes with.
fn rows_of(available: &HashMap<i64, UpdateInfo>, installed: &[InstalledGame]) -> Vec<UpdateRow> {
    let mut rows: Vec<UpdateRow> = installed
        .iter()
        .filter_map(|row| {
            let rom_id = row.rom_id?;
            let info = available.get(&rom_id)?;
            Some(UpdateRow {
                rom_id,
                label: UpdateService::button_label_for(
                    &row.rom_file_name,
                    &info.server_rom_file_name,
                ),
            })
        })
        .collect();
    rows.sort_by_key(|r| r.rom_id);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_label_names_the_target_only_when_the_server_tag_is_newer() {
        assert_eq!(
            UpdateService::button_label_for("g (v1.0.0).zip", "g (v1.1.0).zip"),
            "Update to v1.1.0"
        );
        assert_eq!(
            UpdateService::button_label_for("g (v00009).zip", "g (v00010).zip"),
            "Update to v00010"
        );
        assert_eq!(
            UpdateService::button_label_for("g (v1.1.0).zip", "g (v1.0.0).zip"),
            "Update"
        );
        assert_eq!(
            UpdateService::button_label_for("g.zip", "g (v1.0.0).zip"),
            "Update"
        );
        assert_eq!(
            UpdateService::button_label_for("g (v01234).zip", "g (v1.0.0).zip"),
            "Update"
        );
    }

    #[test]
    fn rows_only_lists_installed_rows_with_an_entry_and_a_rom_id() {
        let service = UpdateService::new();
        service.available.lock().unwrap().insert(
            2,
            UpdateInfo {
                server_rom_file_name: "b (v2.0).zip".into(),
            },
        );
        service.available.lock().unwrap().insert(
            9,
            UpdateInfo {
                server_rom_file_name: "gone.zip".into(),
            },
        );
        let installed = vec![
            InstalledGame {
                rom_id: Some(1),
                rom_file_name: "a.zip".into(),
                ..Default::default()
            },
            InstalledGame {
                rom_id: Some(2),
                rom_file_name: "b (v1.0).zip".into(),
                ..Default::default()
            },
            InstalledGame {
                rom_id: None,
                ..Default::default()
            },
        ];
        assert_eq!(
            service.rows(&installed),
            vec![UpdateRow {
                rom_id: 2,
                label: "Update to v2.0".into()
            }]
        );
        assert!(service.has_update(2));
        assert!(!service.has_update(1));
    }

    fn info(name: &str) -> UpdateInfo {
        UpdateInfo {
            server_rom_file_name: name.to_string(),
        }
    }

    fn installed(rom_id: i64, rom_file_name: &str) -> InstalledGame {
        InstalledGame {
            rom_id: Some(rom_id),
            rom_file_name: rom_file_name.to_string(),
            ..Default::default()
        }
    }

    fn one(rom_id: i64, name: &str) -> HashMap<i64, UpdateInfo> {
        HashMap::from([(rom_id, info(name))])
    }

    /// The registry order the rows arrive in is `title_key`, not rom id, so
    /// the sort is load-bearing.
    #[test]
    fn rows_are_ordered_by_rom_id() {
        let service = UpdateService::new();
        *service.available.lock().unwrap() =
            HashMap::from([(7, info("c.zip")), (2, info("a.zip")), (5, info("b.zip"))]);
        let rows = service.rows(&[
            installed(7, "c.zip"),
            installed(2, "a.zip"),
            installed(5, "b.zip"),
        ]);
        assert_eq!(
            rows.iter().map(|r| r.rom_id).collect::<Vec<_>>(),
            vec![2, 5, 7]
        );
    }

    #[test]
    fn a_stale_pass_writes_nothing_and_emits_nothing() {
        let service = UpdateService::new();
        let generation = service.generation.fetch_add(1, Ordering::SeqCst) + 1;
        // A newer pass starts and finishes first.
        service.generation.fetch_add(1, Ordering::SeqCst);
        assert!(service
            .store_if_current(generation, one(2, "b (v2.0).zip"), &[installed(2, "b.zip")])
            .is_none());
        assert!(!service.has_update(2));
    }

    /// The race the lock ordering exists for: a disconnect lands after the
    /// pass has collected its results but before it commits. The commit must
    /// be dropped, or the cleared set would silently repopulate.
    #[test]
    fn a_clear_mid_pass_discards_that_pass() {
        let service = UpdateService::new();
        let generation = service.generation.fetch_add(1, Ordering::SeqCst) + 1;
        service.take_all();
        assert!(service
            .store_if_current(generation, one(2, "b (v2.0).zip"), &[installed(2, "b.zip")])
            .is_none());
        assert!(!service.has_update(2));
    }

    #[test]
    fn a_current_pass_stores_and_returns_its_rows() {
        let service = UpdateService::new();
        let generation = service.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let emitted = service
            .store_if_current(
                generation,
                one(2, "b (v2.0).zip"),
                &[installed(2, "b (v1.0).zip")],
            )
            .expect("nothing bumped the generation");
        assert_eq!(
            emitted,
            vec![UpdateRow {
                rom_id: 2,
                label: "Update to v2.0".into()
            }]
        );
        assert!(service.has_update(2));
    }

    /// However many triggers land while a pass is running, the pass goes
    /// around exactly once more — not once per trigger.
    #[test]
    fn triggers_during_a_pass_collapse_into_one_rerun() {
        let gate = PassGate::default();
        assert!(gate.should_start(), "nothing was running");
        assert!(!gate.should_start(), "a pass is in flight");
        assert!(!gate.should_start());
        assert!(gate.should_rerun(), "the triggers earn one rerun");
        assert!(!gate.should_rerun(), "...and only one");
    }

    #[test]
    fn a_pass_with_no_trigger_does_not_rerun() {
        let gate = PassGate::default();
        assert!(gate.should_start());
        assert!(!gate.should_rerun());
        // The slot is free again, so the next trigger runs immediately.
        assert!(gate.should_start());
    }

    /// A trigger that arrives after the pass released the slot is a fresh
    /// pass, not a queued rerun.
    #[test]
    fn a_trigger_after_a_pass_ends_starts_its_own_pass() {
        let gate = PassGate::default();
        assert!(gate.should_start());
        assert!(!gate.should_rerun());
        assert!(gate.should_start());
        assert!(!gate.should_rerun(), "that trigger was not queued twice");
    }

    /// The panic path: the guard frees the slot so later triggers still run.
    #[test]
    fn an_abandoned_pass_guard_frees_the_slot() {
        let service = UpdateService::new();
        assert!(service.gate.should_start());
        drop(PassGuard::new(service.clone()));
        assert!(service.gate.should_start());
        // ...and a disarmed guard leaves the flag exactly as it found it.
        let mut guard = PassGuard::new(service.clone());
        guard.disarm();
        drop(guard);
        assert!(
            !service.gate.should_start(),
            "the pass still holds the slot"
        );
    }

    /// `clear` emits only on the transition, so `take_all` reports the set as
    /// non-empty exactly once.
    #[test]
    fn take_all_reports_non_empty_once() {
        let service = UpdateService::new();
        assert!(!service.take_all(), "an empty set has nothing to announce");
        *service.available.lock().unwrap() = one(2, "b (v2.0).zip");
        assert!(service.take_all());
        assert!(!service.take_all());
    }
}
