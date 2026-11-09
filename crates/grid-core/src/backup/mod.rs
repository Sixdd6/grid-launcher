//! Game backups to removable drives: the gridpack format, drive detection, the
//! write engine and backup planning. See
//! `docs/design/system/components/GameBackups/README.md`.
//!
//! A drive is identified by its mount path (Linux, macOS) or its drive letter
//! (Windows). A pack has no id of its own.

pub mod archive;
pub mod drives;
pub mod local;
pub mod pack;
pub mod plan;
pub mod steam_runtime;
pub mod write;
