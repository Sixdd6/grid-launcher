//! The `debug_prints` settings toggle (docs/porting/02-config-and-secrets.md).
//! The rule lives in [`crate::logging`]; these are thin config wrappers.

use grid_core::config::Config;

use crate::config_write::modify_config;
use crate::logging::apply_debug_prints;

use super::err;

#[tauri::command]
pub async fn get_debug_prints() -> Result<bool, String> {
    tokio::task::spawn_blocking(|| {
        let config = Config::load(&Config::default_path()).map_err(err)?;
        Ok(config.debug_prints)
    })
    .await
    .map_err(|e| format!("get_debug_prints did not finish: {e}"))?
}

#[tauri::command]
pub async fn set_debug_prints(enabled: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        modify_config(&Config::default_path(), |config| {
            config.debug_prints = enabled;
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("set_debug_prints did not finish: {e}"))??;
    apply_debug_prints(enabled);
    Ok(())
}
