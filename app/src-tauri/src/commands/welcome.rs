//! The first-run welcome (romm-storefront-plan 0.5, 0.6): whether it shows,
//! and marking it done. `setup.welcomed` in the config; RomM's state never
//! decides it.

use super::err;
use crate::config_write::modify_config;
use grid_core::config::Config;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WelcomeStatus {
    /// Show the welcome: it was never finished or skipped.
    pub needed: bool,
}

fn welcome_status_of(config: &Config) -> WelcomeStatus {
    WelcomeStatus {
        needed: !config.setup.welcomed,
    }
}

#[tauri::command]
pub fn welcome_status() -> Result<WelcomeStatus, String> {
    let config = Config::load(&Config::default_path()).map_err(err)?;
    Ok(welcome_status_of(&config))
}

/// The welcome was finished (Continue) or skipped (Skip for now).
#[tauri::command]
pub fn finish_welcome() -> Result<(), String> {
    modify_config(&Config::default_path(), |c| {
        c.setup.welcomed = true;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_welcome_shows_until_it_is_finished() {
        let mut config = Config::default();
        assert!(welcome_status_of(&config).needed);
        config.setup.welcomed = true;
        assert!(!welcome_status_of(&config).needed);
    }
}
