//! Backup planning: what a backup writes to a drive, what it skips and whether
//! it fits. The planner itself lands in Task 10; these are the types the IPC
//! contract (`plan_backup`) carries.

use serde::{Deserialize, Serialize};

use super::pack::PackOs;

/// A game named in a plan without being written (already on the drive, or too
/// big for the drive's file system).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanGameRef {
    pub rom_id: i64,
    pub name: String,
}

/// One emulator a backup carries, across every OS the pack is for.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanEmulator {
    pub name: String,
    pub os: Vec<PackOs>,
    /// `None` when the release size is not known before the download.
    pub bytes: Option<u64>,
    /// Already on the drive for every OS: nothing to write.
    pub already: bool,
}

/// The firmware of one platform a backup carries.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct PlanFirmware {
    pub platform_name: String,
    /// The number of firmware files.
    pub files: u32,
    pub bytes: u64,
    pub already: bool,
}

/// The answer to `plan_backup`: what the Backup dialog shows before it starts.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BackupPlan {
    /// The drive key: its mount path, or its drive letter on Windows.
    pub drive: String,
    pub games_to_write: Vec<i64>,
    pub skipped: Vec<PlanGameRef>,
    pub too_big: Vec<PlanGameRef>,
    pub emulators: Vec<PlanEmulator>,
    pub firmware: Vec<PlanFirmware>,
    /// Why the firmware checkbox is disabled, else empty.
    pub firmware_reason: String,
    pub bytes_to_write: u64,
    pub free_bytes: u64,
    pub fits: bool,
}
