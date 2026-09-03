//! App-layer server-update tracking (doc 10 "When checks run" / "How an
//! available update is surfaced"). grid-core decides whether ONE row has an
//! update (`library::update_detection`); this module decides WHEN the whole
//! library is re-checked and holds the transient result. Never persisted
//! (doc 10 invariant 5).
//!
//! Triggers (commands.rs / lib.rs): a session comes up (connect, restore,
//! retry), a game finalizes (any install mode), a game is uninstalled. A
//! disconnect clears the set. No timer, no polling.
//!
//! Token secrecy: nothing here logs a URL or header; fetch failures log the
//! rom id only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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
}

impl UpdateService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            available: Mutex::new(HashMap::new()),
            generation: AtomicU64::new(0),
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
        let available = self.available.lock().unwrap();
        let mut rows: Vec<UpdateRow> = installed
            .iter()
            .filter_map(|row| {
                let rom_id = row.rom_id?;
                let info = available.get(&rom_id)?;
                Some(UpdateRow {
                    rom_id,
                    label: Self::button_label_for(&row.rom_file_name, &info.server_rom_file_name),
                })
            })
            .collect();
        rows.sort_by_key(|r| r.rom_id);
        rows
    }

    /// Whether one rom carries an update. Not a command: the frontend reads
    /// the whole set through [`Self::rows`].
    #[allow(dead_code)] // kept as the single-row form of `rows`
    pub fn has_update(&self, rom_id: i64) -> bool {
        self.available.lock().unwrap().contains_key(&rom_id)
    }

    /// Drops every entry; emits the event only when something changed.
    pub fn clear(&self, app: &AppHandle) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let was_empty = {
            let mut available = self.available.lock().unwrap();
            let was_empty = available.is_empty();
            available.clear();
            was_empty
        };
        if !was_empty {
            let _ = app.emit(UPDATES_CHANGED_EVENT, Vec::<UpdateRow>::new());
        }
    }

    /// One full pass over the registry. Runs on Tauri's async runtime; a
    /// pass that is overtaken by a newer one discards its result.
    pub fn spawn_refresh(
        self: &Arc<Self>,
        app: AppHandle,
        session: Arc<SessionManager>,
        install: Arc<InstallService>,
    ) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            this.refresh(app, session, install).await;
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
        for row in rows {
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
        if self.generation.load(Ordering::SeqCst) != generation {
            return; // overtaken by a newer pass (or a clear)
        }
        *self.available.lock().unwrap() = next;
        let installed = install.registry().all().unwrap_or_default();
        let _ = app.emit(UPDATES_CHANGED_EVENT, self.rows(&installed));
    }
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
}
