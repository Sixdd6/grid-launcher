//! Emulated launch core. See `docs/porting/04-emulator-launch.md` for the
//! behavior this module tree ports from `grid_launcher/emulator/` and
//! `grid_launcher/ui/mixins/emulator_ui_mixin.py`.

pub mod profiles;
pub mod rom;
pub mod selection;
pub mod template;

/// Errors raised while resolving or running an emulated launch. Extended by
/// later tasks in this module tree (spawn, sessions); today it carries only
/// the validation case profile matching and argument-template checks need.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("{0}")]
    Validation(String),
}
