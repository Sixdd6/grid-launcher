//! GRID Launcher core library: config, secrets, RomM client, images, session.
//! UI-agnostic — this crate must never depend on Tauri.

pub mod autoconfig;
pub mod cloud;
pub mod config;
pub mod fatx;
pub mod firmware;
pub mod images;
pub mod import_python;
pub mod launch;
pub mod library;
pub mod pcgw;
pub mod platform;
pub mod retroachievements;
pub mod romm;
pub mod secrets;
pub mod session;
#[cfg(test)]
pub(crate) mod test_env;

// unrar_sys 0.5.8 calls the registry API (GetRarDataPath) but its build
// script never links advapi32, so any binary without another advapi32 user
// (grid-core's own test binaries) fails to link on Windows.
#[cfg(windows)]
#[link(name = "advapi32")]
extern "system" {}
