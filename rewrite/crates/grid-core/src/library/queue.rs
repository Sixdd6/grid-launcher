//! Install queue state machine. Pure data and transitions — no I/O, no
//! async. The async service (a later task) drives this under a mutex. See
//! `docs/superpowers/specs/2026-08-31-install-pipeline-core-design.md`
//! ("Queue rules", doc 03 §1, ported) for the behavior this implements.

use std::collections::VecDeque;

use super::LibraryError;

/// Lifecycle state of one [`DownloadEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Installing,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

/// One tracked install, from admission through a terminal status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadEntry {
    pub id: u64,
    pub rom_id: i64,
    pub title: String,
    pub platform: String,
    pub status: DownloadStatus,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: f64,
    pub install_processed_bytes: u64,
    pub install_total_bytes: u64,
    pub error: String,
}

/// The full entry list, newest first (reverse of insertion order).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadsSnapshot {
    pub entries: Vec<DownloadEntry>,
}

/// Result of [`QueueState::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Both slots were free: the entry was created `Downloading` and the
    /// caller should start the download task for this id.
    Start(u64),
    /// A slot was busy: the entry was created `Queued` and appended to
    /// `waiting`.
    Queued(u64),
    /// `rom_id` was already active or waiting; nothing was created.
    Duplicate,
}

/// Result of [`QueueState::request_cancel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelAction {
    /// The entry was the active download: status is now `Cancelling`: the
    /// caller must flip the cooperative cancellation flag.
    ActiveDownload,
    /// The entry was queued: it was removed from `waiting` and marked
    /// `Cancelled`.
    RemovedFromQueue,
    /// The entry does not exist, or is not cancellable in its current
    /// status (e.g. already finalizing or terminal).
    Ignored,
}

/// The install queue: at most one active download and one active finalize,
/// with the rest FIFO in `waiting`.
#[derive(Default)]
pub struct QueueState {
    entries: Vec<DownloadEntry>,
    next_id: u64,
    download_active: Option<u64>,
    finalize_active: Option<u64>,
    waiting: VecDeque<u64>,
}

impl QueueState {
    /// Admits a new install for `rom_id`. Duplicate check (same `rom_id`
    /// already active in either slot's entry, or waiting) happens first and
    /// creates nothing. Otherwise creates the entry: `Downloading` and
    /// takes the download slot when both slots are free, else `Queued` and
    /// appended to `waiting`.
    pub fn admit(&mut self, rom_id: i64, title: &str, platform: &str) -> Admission {
        if self.has_pending(rom_id) {
            return Admission::Duplicate;
        }

        let id = self.alloc_id();
        let both_free = self.download_active.is_none() && self.finalize_active.is_none();
        let status = if both_free {
            DownloadStatus::Downloading
        } else {
            DownloadStatus::Queued
        };
        self.entries.push(DownloadEntry {
            id,
            rom_id,
            title: title.to_string(),
            platform: platform.to_string(),
            status,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_bps: 0.0,
            install_processed_bytes: 0,
            install_total_bytes: 0,
            error: String::new(),
        });

        if both_free {
            self.download_active = Some(id);
            Admission::Start(id)
        } else {
            self.waiting.push_back(id);
            Admission::Queued(id)
        }
    }

    /// Updates download progress for `id`. No-op when `id` is unknown.
    /// `speed` clamps to `>= 0.0`.
    pub fn set_progress(&mut self, id: u64, downloaded: u64, total: u64, speed: f64) {
        if let Some(entry) = self.entry_mut(id) {
            entry.downloaded_bytes = downloaded;
            entry.total_bytes = total;
            entry.speed_bps = speed.max(0.0);
        }
    }

    /// Updates install (extraction/finalize) progress for `id`. No-op when
    /// `id` is unknown.
    pub fn set_install_progress(&mut self, id: u64, processed: u64, total: u64) {
        if let Some(entry) = self.entry_mut(id) {
            entry.install_processed_bytes = processed;
            entry.install_total_bytes = total;
        }
    }

    /// Download task ended. No-op unless `id` currently owns the download
    /// slot (guards against a stale/duplicate completion call arriving
    /// after the slot has already moved on). Frees the download slot.
    /// `Ok(())` with `skip_finalize` false → `Installing`, `finalize_active
    /// = Some(id)`. `Ok(())` with `skip_finalize` true (already installed)
    /// → `Completed` directly. `Err(LibraryError::Cancelled)` → `Cancelled`.
    /// Any other `Err(e)` → `Failed` with `e`'s `Display` text as `error`.
    pub fn download_finished(
        &mut self,
        id: u64,
        result: Result<(), LibraryError>,
        skip_finalize: bool,
    ) {
        if self.download_active != Some(id) {
            return;
        }
        self.download_active = None;

        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        entry.speed_bps = 0.0;
        match result {
            Ok(()) if skip_finalize => {
                entry.status = DownloadStatus::Completed;
            }
            Ok(()) => {
                entry.status = DownloadStatus::Installing;
                self.finalize_active = Some(id);
            }
            Err(LibraryError::Cancelled) => {
                entry.status = DownloadStatus::Cancelled;
            }
            Err(e) => {
                entry.status = DownloadStatus::Failed;
                entry.error = e.to_string();
            }
        }
    }

    /// Finalize task ended. No-op unless `id` currently owns the finalize
    /// slot (guards against a stale/duplicate completion call arriving
    /// after the slot has already moved on). Frees the finalize slot.
    /// `Ok(())` → `Completed`; when `warning` is non-empty it is stored in
    /// `error` even though the entry completed. `Err(e)` → `Failed` with
    /// `e`'s `Display` text as `error`, with `warning` appended after a
    /// newline when non-empty.
    pub fn finalize_finished(&mut self, id: u64, result: Result<(), LibraryError>, warning: &str) {
        if self.finalize_active != Some(id) {
            return;
        }
        self.finalize_active = None;

        let Some(entry) = self.entry_mut(id) else {
            return;
        };
        entry.speed_bps = 0.0;
        match result {
            Ok(()) => {
                entry.status = DownloadStatus::Completed;
                entry.error = warning.to_string();
            }
            Err(e) => {
                entry.status = DownloadStatus::Failed;
                entry.error = if warning.is_empty() {
                    e.to_string()
                } else {
                    format!("{e}\n{warning}")
                };
            }
        }
    }

    /// When both slots are free and `waiting` is non-empty, pops the front
    /// id, marks its entry `Downloading`, takes the download slot, and
    /// returns it. Otherwise returns `None`.
    pub fn next_ready(&mut self) -> Option<u64> {
        if self.download_active.is_some() || self.finalize_active.is_some() {
            return None;
        }
        let id = self.waiting.pop_front()?;
        if let Some(entry) = self.entry_mut(id) {
            entry.status = DownloadStatus::Downloading;
        }
        self.download_active = Some(id);
        Some(id)
    }

    /// Requests cancellation of `id`. See [`CancelAction`] for the three
    /// outcomes.
    pub fn request_cancel(&mut self, id: u64) -> CancelAction {
        if self.download_active == Some(id) {
            if let Some(entry) = self.entry_mut(id) {
                entry.status = DownloadStatus::Cancelling;
            }
            return CancelAction::ActiveDownload;
        }

        if let Some(pos) = self.waiting.iter().position(|&waiting_id| waiting_id == id) {
            self.waiting.remove(pos);
            if let Some(entry) = self.entry_mut(id) {
                entry.status = DownloadStatus::Cancelled;
                entry.error = "Cancelled while queued".to_string();
                entry.speed_bps = 0.0;
            }
            return CancelAction::RemovedFromQueue;
        }

        CancelAction::Ignored
    }

    /// Removes the entry for `id` from the list (and from `waiting` if
    /// still present, so a dismissed queued entry is never started later).
    /// Refuses (returns `false`, leaves everything intact) when `id`
    /// currently owns the download or finalize slot: dismissing it would
    /// leave that slot pointing at a dead id, breaking duplicate detection
    /// and wedging the queue. Callers only offer dismiss for
    /// terminal entries, but this method is the trust boundary — it does
    /// not rely on that convention.
    pub fn dismiss(&mut self, id: u64) -> bool {
        if self.download_active == Some(id) || self.finalize_active == Some(id) {
            return false;
        }
        if let Some(pos) = self.waiting.iter().position(|&waiting_id| waiting_id == id) {
            self.waiting.remove(pos);
        }
        if let Some(pos) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// `Some(rom_id)` when `id`'s entry is `Failed` or `Cancelled`; `None`
    /// otherwise (including when `id` is unknown).
    pub fn retryable(&self, id: u64) -> Option<i64> {
        let entry = self.entry(id)?;
        match entry.status {
            DownloadStatus::Failed | DownloadStatus::Cancelled => Some(entry.rom_id),
            _ => None,
        }
    }

    /// The full entry list, newest first.
    pub fn snapshot(&self) -> DownloadsSnapshot {
        let mut entries = self.entries.clone();
        entries.reverse();
        DownloadsSnapshot { entries }
    }

    /// Looks up an entry by id.
    pub fn entry(&self, id: u64) -> Option<&DownloadEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn entry_mut(&mut self, id: u64) -> Option<&mut DownloadEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// Whether `rom_id` is already active (download or finalize slot) or
    /// sitting in `waiting`.
    fn has_pending(&self, rom_id: i64) -> bool {
        let active = [self.download_active, self.finalize_active]
            .into_iter()
            .flatten()
            .any(|id| self.entry(id).is_some_and(|entry| entry.rom_id == rom_id));
        if active {
            return true;
        }
        self.waiting
            .iter()
            .any(|&id| self.entry(id).is_some_and(|entry| entry.rom_id == rom_id))
    }

    fn alloc_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admit_idle(state: &mut QueueState, rom_id: i64) -> u64 {
        match state.admit(rom_id, "Title", "Platform") {
            Admission::Start(id) => id,
            other => panic!("expected Start, got {other:?}"),
        }
    }

    // --- admit --------------------------------------------------------

    #[test]
    fn admit_on_idle_starts_downloading() {
        let mut state = QueueState::default();
        let admission = state.admit(1, "Game", "SNES");
        let Admission::Start(id) = admission else {
            panic!("expected Start, got {admission:?}");
        };
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(entry.rom_id, 1);
        assert_eq!(entry.title, "Game");
        assert_eq!(entry.platform, "SNES");
    }

    #[test]
    fn admit_while_busy_queues() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        let admission = state.admit(2, "Other", "SNES");
        let Admission::Queued(id) = admission else {
            panic!("expected Queued, got {admission:?}");
        };
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Queued);
    }

    #[test]
    fn duplicate_rom_id_active_is_rejected() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        assert_eq!(state.admit(1, "Game", "SNES"), Admission::Duplicate);
    }

    #[test]
    fn duplicate_rom_id_queued_is_rejected() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        state.admit(2, "Other", "SNES");
        assert_eq!(state.admit(2, "Other", "SNES"), Admission::Duplicate);
    }

    #[test]
    fn duplicate_rom_id_in_finalize_active_is_rejected() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);
        assert_eq!(state.admit(1, "Game", "SNES"), Admission::Duplicate);
    }

    // --- download_finished ---------------------------------------------

    #[test]
    fn download_ok_moves_to_installing_and_busies_finalize_slot() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Installing);
        assert_eq!(state.next_ready(), None); // finalize slot busy, download slot free but waiting is empty anyway
    }

    #[test]
    fn download_ok_frees_download_slot_so_next_ready_can_start_a_waiter() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(2, "Other", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        state.download_finished(first, Ok(()), false);
        // finalize_active is busy with `first`, so next_ready must not fire yet.
        assert_eq!(state.next_ready(), None);
        assert_eq!(state.entry(second).unwrap().status, DownloadStatus::Queued);
    }

    #[test]
    fn download_err_cancelled_sets_cancelled_status_with_no_error_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_progress(id, 100, 200, 5.0);
        state.download_finished(id, Err(LibraryError::Cancelled), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "");
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn download_err_other_sets_failed_with_display_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Err(LibraryError::NoLaunchFile), false);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, LibraryError::NoLaunchFile.to_string());
    }

    #[test]
    fn download_ok_skip_finalize_completes_directly_and_frees_both_slots() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        let waiter = match state.admit(2, "Other", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        state.download_finished(id, Ok(()), true);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Completed);
        // Both slots are free now (finalize was never taken), so the
        // waiter should be pulled in.
        assert_eq!(state.next_ready(), Some(waiter));
        assert_eq!(
            state.entry(waiter).unwrap().status,
            DownloadStatus::Downloading
        );
    }

    #[test]
    fn stale_download_finished_on_a_completed_entry_is_a_no_op() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        state.download_finished(first, Ok(()), false);
        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );

        // The download slot has moved on to a different entry.
        let second = admit_idle(&mut state, 2);
        assert_eq!(state.download_active, Some(second));

        // A stale completion for the old id must not resurrect it, steal
        // the finalize slot, or disturb the entry that now owns the
        // download slot.
        state.download_finished(first, Ok(()), false);
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.finalize_active, None);
        assert_eq!(state.download_active, Some(second));
    }

    // --- finalize_finished ------------------------------------------------

    #[test]
    fn finalize_ok_completes_and_next_ready_pops_fifo() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(2, "Second", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        let third = match state.admit(3, "Third", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };

        state.download_finished(first, Ok(()), false);
        // finalize slot busy: next_ready must not fire.
        assert_eq!(state.next_ready(), None);

        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");

        // Both slots free now: FIFO order pulls `second` before `third`.
        assert_eq!(state.next_ready(), Some(second));
        assert_eq!(
            state.entry(second).unwrap().status,
            DownloadStatus::Downloading
        );
        assert_eq!(state.entry(third).unwrap().status, DownloadStatus::Queued);
    }

    #[test]
    fn finalize_ok_with_warning_stores_it_on_the_completed_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Ok(()), "could not delete archive");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Completed);
        assert_eq!(entry.error, "could not delete archive");
    }

    #[test]
    fn finalize_err_sets_failed_with_display_text() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Err(LibraryError::NoLaunchFile), "");
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Failed);
        assert_eq!(entry.error, LibraryError::NoLaunchFile.to_string());
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn finalize_err_appends_warning_after_newline() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        state.finalize_finished(id, Err(LibraryError::NoLaunchFile), "cleanup also failed");
        let entry = state.entry(id).unwrap();
        assert_eq!(
            entry.error,
            format!("{}\ncleanup also failed", LibraryError::NoLaunchFile)
        );
    }

    #[test]
    fn stale_finalize_finished_on_a_completed_entry_is_a_no_op() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        state.download_finished(first, Ok(()), false);
        state.finalize_finished(first, Ok(()), "");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");

        // A different entry now owns the finalize slot.
        let second = admit_idle(&mut state, 2);
        state.download_finished(second, Ok(()), false);
        assert_eq!(state.finalize_active, Some(second));

        // A stale finalize completion for the old id must not touch the
        // already-completed entry or the slot now owned by `second`.
        state.finalize_finished(first, Err(LibraryError::NoLaunchFile), "stale warning");
        assert_eq!(
            state.entry(first).unwrap().status,
            DownloadStatus::Completed
        );
        assert_eq!(state.entry(first).unwrap().error, "");
        assert_eq!(state.finalize_active, Some(second));
    }

    // --- request_cancel -----------------------------------------------

    #[test]
    fn cancel_queued_removes_from_waiting_with_exact_error() {
        let mut state = QueueState::default();
        admit_idle(&mut state, 1);
        let waiter = match state.admit(2, "Other", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(state.request_cancel(waiter), CancelAction::RemovedFromQueue);
        let entry = state.entry(waiter).unwrap();
        assert_eq!(entry.status, DownloadStatus::Cancelled);
        assert_eq!(entry.error, "Cancelled while queued");
    }

    #[test]
    fn cancel_active_download_marks_cancelling() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        assert_eq!(state.request_cancel(id), CancelAction::ActiveDownload);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Cancelling);
    }

    #[test]
    fn cancel_unknown_id_is_ignored() {
        let mut state = QueueState::default();
        assert_eq!(state.request_cancel(999), CancelAction::Ignored);
    }

    #[test]
    fn cancel_installing_entry_is_ignored() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert_eq!(state.request_cancel(id), CancelAction::Ignored);
        assert_eq!(state.entry(id).unwrap().status, DownloadStatus::Installing);
    }

    // --- dismiss --------------------------------------------------------

    #[test]
    fn dismiss_removes_the_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Err(LibraryError::Cancelled), false);
        assert!(state.dismiss(id));
        assert!(state.entry(id).is_none());
    }

    #[test]
    fn dismiss_unknown_id_returns_false() {
        let mut state = QueueState::default();
        assert!(!state.dismiss(999));
    }

    #[test]
    fn dismiss_queued_entry_removes_it_from_waiting_so_it_never_starts() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let waiter = match state.admit(2, "Other", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert!(state.dismiss(waiter));
        state.download_finished(first, Ok(()), true);
        // The dismissed id must never be picked up by next_ready.
        assert_eq!(state.next_ready(), None);
    }

    #[test]
    fn dismiss_refuses_the_active_download_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        assert!(!state.dismiss(id));
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Downloading);
        assert_eq!(state.download_active, Some(id));
    }

    #[test]
    fn dismiss_refuses_the_finalize_active_entry() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.download_finished(id, Ok(()), false);
        assert!(!state.dismiss(id));
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.status, DownloadStatus::Installing);
        assert_eq!(state.finalize_active, Some(id));
    }

    // --- retryable --------------------------------------------------------

    #[test]
    fn retryable_only_for_failed_or_cancelled() {
        let mut state = QueueState::default();
        let failed = admit_idle(&mut state, 1);
        state.download_finished(failed, Err(LibraryError::NoLaunchFile), false);
        assert_eq!(state.retryable(failed), Some(1));

        let mut state2 = QueueState::default();
        let cancelled = admit_idle(&mut state2, 2);
        state2.download_finished(cancelled, Err(LibraryError::Cancelled), false);
        assert_eq!(state2.retryable(cancelled), Some(2));

        let mut state3 = QueueState::default();
        let active = admit_idle(&mut state3, 3);
        assert_eq!(state3.retryable(active), None);
    }

    #[test]
    fn retryable_unknown_id_is_none() {
        let state = QueueState::default();
        assert_eq!(state.retryable(999), None);
    }

    // --- snapshot --------------------------------------------------------

    #[test]
    fn snapshot_lists_entries_newest_first() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        let second = match state.admit(2, "Second", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        let third = match state.admit(3, "Third", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };

        let ids: Vec<u64> = state.snapshot().entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![third, second, first]);
    }

    // --- progress setters -------------------------------------------------

    #[test]
    fn set_progress_updates_fields_and_clamps_negative_speed() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_progress(id, 50, 100, -3.0);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.downloaded_bytes, 50);
        assert_eq!(entry.total_bytes, 100);
        assert_eq!(entry.speed_bps, 0.0);
    }

    #[test]
    fn set_progress_on_unknown_id_is_a_no_op() {
        let mut state = QueueState::default();
        state.set_progress(999, 1, 2, 3.0); // must not panic
    }

    #[test]
    fn set_install_progress_updates_fields() {
        let mut state = QueueState::default();
        let id = admit_idle(&mut state, 1);
        state.set_install_progress(id, 10, 20);
        let entry = state.entry(id).unwrap();
        assert_eq!(entry.install_processed_bytes, 10);
        assert_eq!(entry.install_total_bytes, 20);
    }

    #[test]
    fn set_install_progress_on_unknown_id_is_a_no_op() {
        let mut state = QueueState::default();
        state.set_install_progress(999, 1, 2); // must not panic
    }

    // --- ids ----------------------------------------------------------

    #[test]
    fn ids_start_at_one_and_increment_never_reused() {
        let mut state = QueueState::default();
        let first = admit_idle(&mut state, 1);
        assert_eq!(first, 1);
        let second = match state.admit(2, "Other", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(second, 2);
        state.dismiss(second);
        let third = match state.admit(3, "Third", "SNES") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(third, 3);
    }
}
