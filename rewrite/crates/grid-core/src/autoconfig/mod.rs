//! Emulator autoconfiguration: the `ensure_*` writers that seed an
//! emulator's own settings files so a launched game finds its saves,
//! firmware, controller profile and RetroAchievements login where GRID
//! expects them.
//!
//! Ports `grid_launcher/emulator/*.py`'s `ensure_*` functions. See
//! `docs/porting/05-emulator-autoconfig.md` for the behavior contract; the
//! shared section-writer families live in [`writers`] and the path helpers
//! every module's candidate list is built from live in [`paths`].

pub mod azahar;
pub mod cemu;
pub mod cores;
pub mod dolphin;
pub mod duckstation;
pub mod eden;
pub mod paths;
pub mod pcsx2;
pub mod ppsspp;
pub mod redream;
pub mod retroarch;
pub mod rpcs3;
pub mod writers;
pub mod xemu;

use std::collections::BTreeMap;
use std::path::PathBuf;

use secrecy::{ExposeSecret, SecretString};

/// RetroAchievements credentials as GRID holds them for the `ensure_*`
/// writers that log RetroArch (and, later, other cores) into RetroAchievements.
///
/// Reaches its final form in a later milestone task; this task defines the
/// shape the writers need: a plain username and a redacted token.
/// `token()` is the ONLY `expose_secret()` call site outside
/// `secrets.rs`/`romm/mod.rs` — `scripts/check_secret_hygiene.sh` allowlists
/// this file for exactly that reason. `Debug` is derived rather than
/// hand-written: `SecretString`'s own `Debug` impl already redacts, so the
/// derive never leaks the token.
#[derive(Debug, Clone)]
pub struct RaCredentials {
    username: String,
    token: SecretString,
}

impl RaCredentials {
    pub fn new(username: impl Into<String>, token: impl Into<SecretString>) -> Self {
        Self {
            username: username.into(),
            token: token.into(),
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// The token in the clear. Every call site must be a write straight to
    /// disk (retroarch.cfg's `cheevos_token` line) or an equally narrow,
    /// audited sink — never a log, an error, or an IPC payload.
    pub fn token(&self) -> &str {
        self.token.expose_secret()
    }
}

/// Every `ensure_*` writer's return value.
///
/// Spec deviation D8: Python returns a `str`, a `Path`, or a `dict`
/// depending on the module — a dynamic-typing artifact, not a behavior. One
/// struct carries all three shapes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnsureResult {
    /// True when this call wrote at least one file.
    pub changed: bool,
    /// The primary file the writer targeted. `None` when the writer bailed
    /// out (blank path, missing executable, unreadable or unwritable file).
    pub config_path: Option<PathBuf>,
    /// Secondary files a writer also owns. Documented keys, and only these:
    ///   dolphin -> "gfx_ini_path", "gcpad_ini_path"
    ///   rpcs3   -> "gui_config_path", "current_settings_path", "vfs_path"
    ///   cemu    -> "profile_path"
    ///   ppsspp  -> "ra_token_path"
    pub extras: BTreeMap<String, PathBuf>,
}

impl EnsureResult {
    /// The bail-out result: nothing written, no path to report.
    pub fn unchanged() -> Self {
        Self::default()
    }

    /// A result naming the primary config file the writer targeted.
    pub fn at(path: impl Into<PathBuf>, changed: bool) -> Self {
        Self {
            changed,
            config_path: Some(path.into()),
            extras: BTreeMap::new(),
        }
    }

    /// Record a secondary file this writer owns. Chainable.
    pub fn with_extra(mut self, key: &str, path: impl Into<PathBuf>) -> Self {
        self.extras.insert(key.to_string(), path.into());
        self
    }

    /// Fold another write's outcome in: `self.changed |= other`.
    pub fn merge_changed(&mut self, other: bool) {
        self.changed |= other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_result_unchanged_is_all_default() {
        let result = EnsureResult::unchanged();
        assert!(!result.changed);
        assert_eq!(result.config_path, None);
        assert!(result.extras.is_empty());
    }

    #[test]
    fn ensure_result_at_records_path_and_extras() {
        let result =
            EnsureResult::at("/tmp/PCSX2.ini", true).with_extra("gfx_ini_path", "/tmp/GFX.ini");
        assert!(result.changed);
        assert_eq!(result.config_path, Some(PathBuf::from("/tmp/PCSX2.ini")));
        assert_eq!(
            result.extras.get("gfx_ini_path"),
            Some(&PathBuf::from("/tmp/GFX.ini"))
        );
    }

    /// The writers are consumed from sibling modules (one per emulator), so
    /// `desired!` must resolve through the crate root, not only inside
    /// `writers.rs` where `macro_rules!` puts it in textual scope.
    #[test]
    fn desired_macro_is_usable_from_another_module() {
        let want = crate::desired![("Key", "value"), ("Other", "2"),];
        assert_eq!(
            want,
            vec![
                ("Key".to_string(), "value".to_string()),
                ("Other".to_string(), "2".to_string()),
            ]
        );
        let empty: writers::Desired = crate::desired![];
        assert!(empty.is_empty());
    }

    #[test]
    fn ra_credentials_debug_redacts_the_token() {
        let ra = RaCredentials::new("sixdd6", "FAKE-TEST-TOKEN-not-real");
        let debug = format!("{ra:?}");
        assert!(!debug.contains("FAKE-TEST-TOKEN-not-real"), "leak: {debug}");
        assert!(debug.contains("sixdd6"), "username should still print");
    }

    #[test]
    fn ra_credentials_accessors_round_trip() {
        let ra = RaCredentials::new("sixdd6", "FAKE-TEST-TOKEN-not-real");
        assert_eq!(ra.username(), "sixdd6");
        assert_eq!(ra.token(), "FAKE-TEST-TOKEN-not-real");
    }

    #[test]
    fn ensure_result_merge_changed_is_sticky() {
        let mut result = EnsureResult::at("/tmp/x.ini", false);
        result.merge_changed(false);
        assert!(!result.changed);
        result.merge_changed(true);
        assert!(result.changed);
        result.merge_changed(false);
        assert!(result.changed, "merge_changed must never clear a set flag");
    }
}
