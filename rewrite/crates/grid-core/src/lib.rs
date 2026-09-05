//! GRID Launcher core library: config, secrets, RomM client, images, session.
//! UI-agnostic — this crate must never depend on Tauri.

pub mod autoconfig;
pub mod cloud;
pub mod config;
pub mod fatx;
pub mod firmware;
pub mod images;
pub mod launch;
pub mod library;
pub mod pcgw;
pub mod retroachievements;
pub mod romm;
pub mod secrets;
pub mod session;
#[cfg(test)]
pub(crate) mod test_env;
