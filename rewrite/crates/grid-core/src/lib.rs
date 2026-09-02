//! GRID Launcher core library: config, secrets, RomM client, covers, session.
//! UI-agnostic — this crate must never depend on Tauri.

pub mod autoconfig;
pub mod config;
pub mod covers;
pub mod launch;
pub mod library;
pub mod romm;
pub mod secrets;
pub mod session;
#[cfg(test)]
pub(crate) mod test_env;
